//! Core-driven listener lifecycle. No HTTP or input is authorized here.

use crate::interfaces::{ApprovedSelection, BindingPlan, InterfaceError, InterfaceInventory};
use std::fmt;
use std::num::NonZeroU64;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::task::Poll;
use std::time::{Duration, Instant};

const RELEASE_TIMEOUT: Duration = Duration::from_secs(2);
const INVENTORY_INTERVAL: Duration = Duration::from_millis(500);
const MAX_TRANSPORTS: usize = 64;
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);

/// Requests always apply to the entire input dispatcher. A supplied network
/// generation helps retire its sessions, but cannot limit dispatcher cleanup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseCause {
    StartOrRebind,
    Stop,
    InterfaceChanged,
    TransportFault,
    Shutdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafetyError {
    Unavailable,
    NotNeutral,
}

/// The core issues this ticket after closing session admission and enqueuing
/// reserved cleanup. Dropping the ticket must never cancel that cleanup.
pub trait NeutralityTicket {
    fn poll_neutrality(&mut self) -> Poll<Result<(), SafetyError>>;
}

pub trait SafetyCoordinator {
    type Ticket: NeutralityTicket;

    /// Use a nonblocking reserved path. Success from the ticket requires
    /// session teardown and confirmed neutral input.
    ///
    /// # Errors
    /// Returns an error if cleanup could not be submitted.
    fn request_release(
        &mut self,
        cause: ReleaseCause,
        obsolete: Option<NonZeroU64>,
    ) -> Result<Self::Ticket, SafetyError>;
}

pub struct AcceptedTransport<T, H> {
    pub transport: T,
    pub shutdown: H,
}

pub type AcceptResult<D> = Result<
    Option<AcceptedTransport<<D as ListenerDriver>::Transport, <D as ListenerDriver>::Shutdown>>,
    <D as ListenerDriver>::Error,
>;

/// Native work must have a bounded, non-waiting poll. A cancelled inventory
/// request must never publish a result after cancellation.
pub trait ListenerDriver: Sized {
    type Set;
    type Transport;
    type Shutdown;
    type Error;

    fn inventory(&self) -> &InterfaceInventory;
    /// # Errors
    /// Reports inability to start a bounded native refresh.
    fn begin_refresh(&mut self, now: Instant) -> Result<(), InterfaceError>;
    fn cancel_refresh(&mut self);
    fn poll_refresh(&mut self, now: Instant) -> Poll<Result<(), InterfaceError>>;
    /// # Errors
    /// On error, the driver must close the entire attempted socket set.
    fn bind(&mut self, plan: BindingPlan, generation: NonZeroU64)
    -> Result<Self::Set, Self::Error>;
    /// # Errors
    /// Reports errors that compromise the listener. Reject individual ineligible peers.
    fn accept(&mut self, listeners: &mut Self::Set, generation: NonZeroU64) -> AcceptResult<Self>;

    /// Terminates a retained transport through its independent shutdown handle.
    ///
    /// # Errors
    /// Reports inability to confirm transport shutdown.
    fn close(shutdown: &Self::Shutdown) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListenerStatus {
    Stopped,
    Draining,
    Starting,
    Listening { generation: NonZeroU64 },
    Faulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SupervisorError<E> {
    Busy,
    Faulted,
    Interface(InterfaceError),
    Socket(E),
    Safety(SafetyError),
    ReleaseTimedOut,
    CounterExhausted,
    CleanupFailed,
    Panicked,
}

struct Generation {
    id: NonZeroU64,
    active: AtomicBool,
}

/// Keep this guard with the transport until the HTTP or WebSocket layer
/// finishes. The same authority is revoked for every socket in a generation.
#[must_use]
pub struct ConnectionAuthority<H> {
    generation: Arc<Generation>,
    _shutdown: Arc<H>,
}

/// Revocable generation evidence for work passing through a bounded service
/// queue. It keeps no socket or connection permit alive.
#[derive(Clone)]
pub struct ConnectionAdmission(Arc<Generation>);

impl ConnectionAdmission {
    #[must_use]
    pub fn generation(&self) -> NonZeroU64 {
        self.0.id
    }

    #[must_use]
    pub fn is_current(&self) -> bool {
        self.0.active.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) fn test_only(generation: NonZeroU64) -> Self {
        Self(Arc::new(Generation {
            id: generation,
            active: AtomicBool::new(true),
        }))
    }

    #[cfg(test)]
    pub(crate) fn retire_for_test(&self) {
        self.0.active.store(false, Ordering::Release);
    }
}

impl<H> ConnectionAuthority<H> {
    #[must_use]
    pub fn admission(&self) -> ConnectionAdmission {
        ConnectionAdmission(Arc::clone(&self.generation))
    }
    #[must_use]
    pub fn generation(&self) -> NonZeroU64 {
        self.generation.id
    }

    #[must_use]
    pub fn is_current(&self) -> bool {
        self.generation.active.load(Ordering::Acquire)
    }
}

impl<H> fmt::Debug for ConnectionAuthority<H> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ConnectionAuthority(<redacted>)")
    }
}

struct LiveSet<D: ListenerDriver> {
    listeners: D::Set,
    revision: u64,
    generation: Arc<Generation>,
    connections: Vec<Weak<D::Shutdown>>,
}

struct Drain<T> {
    ticket: T,
    deadline: Instant,
    start: Option<ApprovedSelection>,
}

enum State<T> {
    Stopped {
        refreshing: bool,
    },
    Draining(Drain<T>),
    BeforeBind(ApprovedSelection),
    AfterBind,
    Listening {
        observed: Instant,
        refreshing: bool,
        accepted: bool,
    },
    Faulted,
}

impl<T> State<T> {
    const STOPPED: Self = Self::Stopped { refreshing: false };
}

type Admitted<D> = (
    <D as ListenerDriver>::Transport,
    ConnectionAuthority<<D as ListenerDriver>::Shutdown>,
);
type Outcome<D, T> = Result<T, SupervisorError<<D as ListenerDriver>::Error>>;

/// Only the core drives this supervisor. It must poll transitions while idle
/// and deliver stop/lifecycle commands ahead of routine accept work.
pub struct ListenerSupervisor<D: ListenerDriver, S: SafetyCoordinator> {
    driver: D,
    safety: S,
    state: State<S::Ticket>,
    live: Option<LiveSet<D>>,
    last_generation: Option<NonZeroU64>,
    generation_counter: &'static AtomicU64,
}

impl<D: ListenerDriver, S: SafetyCoordinator> ListenerSupervisor<D, S> {
    pub const fn new(driver: D, safety: S) -> Self {
        Self {
            driver,
            safety,
            state: State::STOPPED,
            live: None,
            last_generation: None,
            generation_counter: &NEXT_GENERATION,
        }
    }

    #[must_use]
    pub fn inventory(&self) -> &InterfaceInventory {
        self.driver.inventory()
    }

    #[must_use]
    pub fn status(&self) -> ListenerStatus {
        match self.state {
            State::Stopped { .. } => ListenerStatus::Stopped,
            State::Draining(_) => ListenerStatus::Draining,
            State::BeforeBind(_) | State::AfterBind => ListenerStatus::Starting,
            State::Listening { .. } => self
                .last_generation
                .map_or(ListenerStatus::Faulted, |generation| {
                    ListenerStatus::Listening { generation }
                }),
            State::Faulted => ListenerStatus::Faulted,
        }
    }

    /// Obtains the inventory for foreground approval while stopped.
    ///
    /// # Errors
    /// Rejects requests outside the stopped state and reports provider failure.
    pub fn request_inventory(&mut self, now: Instant) -> Outcome<D, ()> {
        self.protect(|this| match this.state {
            State::Stopped { refreshing: true } => Ok(()),
            State::Stopped { refreshing: false } => {
                this.driver
                    .begin_refresh(now)
                    .map_err(SupervisorError::Interface)?;
                this.state = State::Stopped { refreshing: true };
                Ok(())
            }
            State::Faulted => Err(SupervisorError::Faulted),
            _ => Err(SupervisorError::Busy),
        })
    }

    /// Begins a start or rebind after the core has durably committed approval.
    ///
    /// # Errors
    /// Rejects a conflicting transition or unavailable reserved cleanup.
    pub fn start(&mut self, approval: ApprovedSelection, now: Instant) -> Outcome<D, ()> {
        self.protect(|this| match this.state {
            State::Stopped { .. } | State::Listening { .. } => {
                this.drain(Some(approval), ReleaseCause::StartOrRebind, now)
            }
            State::Faulted => Err(SupervisorError::Faulted),
            _ => Err(SupervisorError::Busy),
        })
    }

    /// Immediately closes authority. If cleanup is pending, reuse its deadline
    /// and cancel any pending start. Poll transitions to finish closing sockets.
    ///
    /// # Errors
    /// Reports unavailable reserved cleanup or a permanently faulted supervisor.
    pub fn stop(&mut self, now: Instant) -> Outcome<D, ()> {
        self.protect(|this| match &mut this.state {
            State::Draining(drain) => {
                drain.start = None;
                Ok(())
            }
            State::Faulted => Err(SupervisorError::Faulted),
            _ => this.drain(None, ReleaseCause::Stop, now),
        })
    }

    /// Drives one cleanup, binding or idle-validation step.
    ///
    /// # Errors
    /// Reports typed binding and provider failures, or a latched safety fault.
    pub fn poll_transition(&mut self, now: Instant) -> Outcome<D, ()> {
        self.protect(|this| this.advance(now))
    }

    /// Returns at most one on-link transport and its revocation guard. Callers
    /// must check the guard at request and session boundaries.
    ///
    /// # Errors
    /// A native failure closes admission and starts reserved cleanup.
    pub fn poll_accept(&mut self, now: Instant) -> Outcome<D, Option<Admitted<D>>> {
        self.protect(|this| this.accept(now))
    }

    fn drain(
        &mut self,
        start: Option<ApprovedSelection>,
        cause: ReleaseCause,
        now: Instant,
    ) -> Outcome<D, ()> {
        self.invalidate();
        let ticket = match self.safety.request_release(cause, self.last_generation) {
            Ok(ticket) => ticket,
            Err(error) => return self.fail(SupervisorError::Safety(error)),
        };
        let Some(deadline) = now.checked_add(RELEASE_TIMEOUT) else {
            return self.fail(SupervisorError::CounterExhausted);
        };
        self.driver.cancel_refresh();
        self.state = State::Draining(Drain {
            ticket,
            deadline,
            start,
        });
        Ok(())
    }

    fn advance(&mut self, now: Instant) -> Outcome<D, ()> {
        match std::mem::replace(&mut self.state, State::STOPPED) {
            State::Stopped { refreshing: false } => Ok(()),
            State::Stopped { refreshing: true } => match self.driver.poll_refresh(now) {
                Poll::Pending => {
                    self.state = State::Stopped { refreshing: true };
                    Ok(())
                }
                Poll::Ready(result) => result.map_err(SupervisorError::Interface),
            },
            State::Faulted => {
                self.state = State::Faulted;
                Err(SupervisorError::Faulted)
            }
            State::Draining(mut drain) => {
                if now >= drain.deadline {
                    return self.fail(SupervisorError::ReleaseTimedOut);
                }
                match drain.ticket.poll_neutrality() {
                    Poll::Pending => self.state = State::Draining(drain),
                    Poll::Ready(Err(error)) => return self.fail(SupervisorError::Safety(error)),
                    Poll::Ready(Ok(())) => {
                        if !self.close_live() {
                            return self.fail(SupervisorError::CleanupFailed);
                        }
                        if let Some(approval) = drain.start {
                            self.driver
                                .begin_refresh(now)
                                .map_err(SupervisorError::Interface)?;
                            self.state = State::BeforeBind(approval);
                        }
                    }
                }
                Ok(())
            }
            State::BeforeBind(approval) => match self.driver.poll_refresh(now) {
                Poll::Pending => {
                    self.state = State::BeforeBind(approval);
                    Ok(())
                }
                Poll::Ready(result) => {
                    result.map_err(SupervisorError::Interface)?;
                    self.bind_candidate(&approval, now)
                }
            },
            State::AfterBind => match self.driver.poll_refresh(now) {
                Poll::Pending => {
                    self.state = State::AfterBind;
                    Ok(())
                }
                Poll::Ready(result) => self.publish_candidate(result, now),
            },
            State::Listening {
                observed,
                refreshing,
                accepted,
            } => self.revalidate(observed, refreshing, accepted, now),
        }
    }

    fn bind_candidate(&mut self, approval: &ApprovedSelection, now: Instant) -> Outcome<D, ()> {
        let Some(generation) = self
            .generation_counter
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .ok()
            .and_then(NonZeroU64::new)
        else {
            return self.fail(SupervisorError::CounterExhausted);
        };
        let snapshot = self
            .driver
            .inventory()
            .current()
            .ok_or(SupervisorError::Interface(InterfaceError::StaleInventory))?;
        let plan =
            BindingPlan::new(snapshot, approval, generation).map_err(SupervisorError::Interface)?;
        let revision = snapshot.revision();
        self.last_generation = Some(generation);
        let listeners = self
            .driver
            .bind(plan, generation)
            .map_err(SupervisorError::Socket)?;
        self.driver
            .begin_refresh(now)
            .map_err(SupervisorError::Interface)?;
        self.live = Some(LiveSet {
            listeners,
            revision,
            generation: Arc::new(Generation {
                id: generation,
                active: AtomicBool::new(false),
            }),
            connections: Vec::new(),
        });
        self.state = State::AfterBind;
        Ok(())
    }

    fn publish_candidate(
        &mut self,
        result: Result<(), InterfaceError>,
        now: Instant,
    ) -> Outcome<D, ()> {
        if let Err(error) = result.and_then(|()| self.check_inventory()) {
            if !self.close_live() {
                return self.fail(SupervisorError::CleanupFailed);
            }
            return Err(SupervisorError::Interface(error));
        }
        let Some(live) = &self.live else {
            return self.fail(SupervisorError::CleanupFailed);
        };
        live.generation.active.store(true, Ordering::Release);
        self.state = State::Listening {
            observed: now,
            refreshing: false,
            accepted: false,
        };
        Ok(())
    }

    fn revalidate(
        &mut self,
        observed: Instant,
        refreshing: bool,
        accepted: bool,
        now: Instant,
    ) -> Outcome<D, ()> {
        if now < observed {
            return self.inventory_lost(InterfaceError::ProviderChanged, now);
        }
        if refreshing {
            match self.driver.poll_refresh(now) {
                Poll::Pending => {}
                Poll::Ready(result) => {
                    if let Err(error) = result.and_then(|()| self.check_inventory()) {
                        return self.inventory_lost(error, now);
                    }
                    self.state = State::Listening {
                        observed: now,
                        refreshing: false,
                        accepted: false,
                    };
                    return Ok(());
                }
            }
        } else if accepted || now.saturating_duration_since(observed) >= INVENTORY_INTERVAL {
            return self.begin_revalidation(observed, now);
        }
        self.state = State::Listening {
            observed,
            refreshing,
            accepted,
        };
        Ok(())
    }

    fn begin_revalidation(&mut self, observed: Instant, now: Instant) -> Outcome<D, ()> {
        if let Err(error) = self.driver.begin_refresh(now) {
            return self.inventory_lost(error, now);
        }
        self.state = State::Listening {
            observed,
            refreshing: true,
            accepted: false,
        };
        Ok(())
    }

    fn check_inventory(&self) -> Result<(), InterfaceError> {
        let (Some(live), Some(snapshot)) = (&self.live, self.driver.inventory().current()) else {
            return Err(InterfaceError::StaleInventory);
        };
        if live.revision != snapshot.revision() {
            return Err(InterfaceError::StaleInventory);
        }
        Ok(())
    }

    fn inventory_lost(&mut self, error: InterfaceError, now: Instant) -> Outcome<D, ()> {
        self.drain(None, ReleaseCause::InterfaceChanged, now)?;
        Err(SupervisorError::Interface(error))
    }

    fn accept(&mut self, now: Instant) -> Outcome<D, Option<Admitted<D>>> {
        let State::Listening {
            observed,
            refreshing,
            accepted,
        } = self.state
        else {
            return Ok(None);
        };
        if now < observed {
            self.inventory_lost(InterfaceError::ProviderChanged, now)?;
            return Ok(None);
        }
        if refreshing {
            return Ok(None);
        }
        if accepted || now.saturating_duration_since(observed) >= INVENTORY_INTERVAL {
            self.begin_revalidation(observed, now)?;
            return Ok(None);
        }
        if let Err(error) = self.check_inventory() {
            self.inventory_lost(error, now)?;
            return Ok(None);
        }
        let Some(live) = &mut self.live else {
            return self.fail(SupervisorError::CleanupFailed);
        };
        live.connections.retain(|handle| handle.strong_count() != 0);
        if live.connections.len() >= MAX_TRANSPORTS {
            return Ok(None);
        }
        let candidate = self.driver.accept(&mut live.listeners, live.generation.id);
        match candidate {
            Ok(None) => Ok(None),
            Ok(Some(AcceptedTransport {
                transport,
                shutdown,
            })) => {
                let shutdown = Arc::new(shutdown);
                live.connections.push(Arc::downgrade(&shutdown));
                let authority = ConnectionAuthority {
                    generation: Arc::clone(&live.generation),
                    _shutdown: shutdown,
                };
                self.state = State::Listening {
                    observed,
                    refreshing: false,
                    accepted: true,
                };
                Ok(Some((transport, authority)))
            }
            Err(error) => {
                self.drain(None, ReleaseCause::TransportFault, now)?;
                Err(SupervisorError::Socket(error))
            }
        }
    }

    fn invalidate(&self) {
        if let Some(live) = &self.live {
            live.generation.active.store(false, Ordering::Release);
        }
    }

    fn close_live(&mut self) -> bool {
        let Some(live) = self.live.take() else {
            return true;
        };
        live.generation.active.store(false, Ordering::Release);
        let mut clean = true;
        for handle in live.connections.iter().filter_map(Weak::upgrade) {
            clean &= matches!(
                catch_unwind(AssertUnwindSafe(move || D::close(&handle))),
                Ok(Ok(()))
            );
        }
        clean & catch_unwind(AssertUnwindSafe(|| drop(live))).is_ok()
    }

    fn fail<T>(&mut self, reason: SupervisorError<D::Error>) -> Outcome<D, T> {
        self.state = State::Faulted;
        self.close_live();
        Err(reason)
    }

    fn protect<T>(&mut self, run: impl FnOnce(&mut Self) -> Outcome<D, T>) -> Outcome<D, T> {
        if let Ok(result) = catch_unwind(AssertUnwindSafe(|| run(self))) {
            result
        } else {
            self.best_effort_shutdown();
            self.fail(SupervisorError::Panicked)
        }
    }

    fn best_effort_shutdown(&mut self) {
        self.invalidate();
        let _ = catch_unwind(AssertUnwindSafe(|| {
            self.safety
                .request_release(ReleaseCause::Shutdown, self.last_generation)
        }));
        let _ = catch_unwind(AssertUnwindSafe(|| self.driver.cancel_refresh()));
    }
}

impl<D: ListenerDriver, S: SafetyCoordinator> Drop for ListenerSupervisor<D, S> {
    fn drop(&mut self) {
        if matches!(self.state, State::Stopped { .. } | State::Draining(_)) {
            let _ = catch_unwind(AssertUnwindSafe(|| self.driver.cancel_refresh()));
        } else {
            self.best_effort_shutdown();
        }
        self.close_live();
    }
}

#[cfg(test)]
mod tests;
