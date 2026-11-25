//! Connects authenticated sockets to the single-writer session actor.

use futures_util::future::{Either, select};
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::time::{Duration, Instant};
use tether_config::DeviceId;
use tether_input_core::InputBackend;
use tether_network::http1::{ControlAuthorization, ControlPayload, ControlSession, ControlSocket};
use tether_session::actor::{
    ConnectionLease, CredentialAuthority, SessionActor, SessionError, SessionOutputEncoder,
    TransportGeneration, VerifiedConnection,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{Notify, mpsc, oneshot};
use tokio::time::timeout;

const MAX_CONNECTIONS: usize = 8;
const MAX_INBOUND_BYTES: usize = 128 * 1024;
const ACTOR_DEADLINE: Duration = Duration::from_secs(2);

type Reservation = ControlAuthorization<ActorSocket>;
type Output = oneshot::Sender<Vec<u8>>;

struct Register {
    proof: VerifiedConnection<DeviceId>,
    response: oneshot::Sender<Reservation>,
}

struct Input {
    payload: ControlPayload,
    receipt: oneshot::Sender<Result<(), SessionError>>,
    _budget: RetainedBytes,
}

struct RetainedBytes {
    total: Arc<AtomicUsize>,
    bytes: usize,
}

impl RetainedBytes {
    fn reserve(total: &Arc<AtomicUsize>, bytes: usize) -> Option<Self> {
        total
            .try_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(bytes)
                    .filter(|sum| *sum <= MAX_INBOUND_BYTES)
            })
            .ok()?;
        Some(Self {
            total: Arc::clone(total),
            bytes,
        })
    }
}

impl Drop for RetainedBytes {
    fn drop(&mut self) {
        self.total.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

#[derive(Clone)]
pub(crate) struct ControlBridge {
    registrations: SyncSender<Register>,
    activity: Arc<Notify>,
}

/// Owned by the same thread as the session actor. Drive it even when no
/// sockets are active so expiry and emergency cleanup continue to progress.
pub struct ControlInbox {
    origin: Instant,
    registrations: Receiver<Register>,
    active: Vec<Active>,
    retained: Arc<AtomicUsize>,
    activity: Arc<Notify>,
}

struct Active {
    lease: Arc<ConnectionLease>,
    input: mpsc::Receiver<Input>,
    demand: mpsc::Receiver<Output>,
    pending: Option<Output>,
    encoder: SessionOutputEncoder,
}

impl Drop for Active {
    fn drop(&mut self) {
        self.lease.disconnect();
    }
}

/// An exclusive reserved actor lease and its bounded network-side mailboxes.
pub struct ActorSocket {
    lease: Arc<ConnectionLease>,
    credential: CredentialAuthority,
    input: mpsc::Sender<Input>,
    demand: mpsc::Sender<Output>,
    retained: Arc<AtomicUsize>,
    activity: Arc<Notify>,
}

impl Drop for ActorSocket {
    fn drop(&mut self) {
        // Actor state may still share the lease while this socket goes away.
        self.lease.disconnect();
        self.activity.notify_one();
    }
}

pub(crate) fn channel(origin: Instant) -> (ControlBridge, ControlInbox) {
    let (sender, registrations) = sync_channel(MAX_CONNECTIONS);
    let activity = Arc::new(Notify::new());
    (
        ControlBridge {
            registrations: sender,
            activity: Arc::clone(&activity),
        },
        ControlInbox {
            origin,
            registrations,
            active: Vec::with_capacity(MAX_CONNECTIONS),
            retained: Arc::new(AtomicUsize::new(0)),
            activity,
        },
    )
}

impl ControlBridge {
    pub(crate) async fn reserve(&self, proof: VerifiedConnection<DeviceId>) -> Reservation {
        let (response, reply) = oneshot::channel();
        match self.registrations.try_send(Register { proof, response }) {
            Ok(()) => {
                self.activity.notify_one();
                reply.await.unwrap_or(ControlAuthorization::Unavailable)
            }
            Err(TrySendError::Full(_)) => ControlAuthorization::RateLimited,
            Err(TrySendError::Disconnected(_)) => ControlAuthorization::Unavailable,
        }
    }
}

impl ControlInbox {
    /// Waits for control work or the next periodic safety and network poll.
    pub async fn wait_for_activity(&self, poll_interval: Duration) {
        let _ = select(
            pin!(self.activity.notified()),
            pin!(tokio::time::sleep(poll_interval)),
        )
        .await;
    }

    /// Activates a listener generation using the same clock as mailbox polling.
    ///
    /// # Errors
    /// Rejects a stale generation or an actor that has not confirmed neutrality.
    pub fn recover_transport<B: InputBackend + 'static>(
        &self,
        actor: &mut SessionActor<'_, B, DeviceId>,
        generation: TransportGeneration,
    ) -> Result<(), SessionError> {
        actor.recover(self.origin.elapsed(), generation)
    }

    /// Polls the actor and advances at most one registration, message and
    /// output demand per relevant mailbox. It never waits on a socket.
    pub fn drive<B: InputBackend + 'static>(&mut self, actor: &mut SessionActor<'_, B, DeviceId>) {
        let now = self.origin.elapsed();
        actor.poll(now);
        self.active.retain(|session| !session.lease.is_closed());
        if let Ok(Register { proof, response }) = self.registrations.try_recv()
            && !response.is_closed()
        {
            let reservation = self.register(actor, now, proof);
            let _cancelled = response.send(reservation);
        }
        for session in &mut self.active {
            session.drive(actor, now);
        }
        self.active.retain(|session| !session.lease.is_closed());
    }

    fn register<B: InputBackend + 'static>(
        &mut self,
        actor: &mut SessionActor<'_, B, DeviceId>,
        now: Duration,
        proof: VerifiedConnection<DeviceId>,
    ) -> Reservation {
        let mut identifier = [0; 16];
        if getrandom::fill(&mut identifier).is_err() {
            return ControlAuthorization::Unavailable;
        }
        let credential = proof.credential.clone();
        let lease = match actor.register(now, proof) {
            Ok(lease) => Arc::new(lease),
            Err(SessionError::StaleAuthority) => return ControlAuthorization::Rejected,
            Err(SessionError::Capacity) => return ControlAuthorization::RateLimited,
            Err(_) => return ControlAuthorization::Unavailable,
        };
        let (input, receive) = mpsc::channel(1);
        let (demand, read) = mpsc::channel(1);
        self.active.push(Active {
            lease: Arc::clone(&lease),
            input: receive,
            demand: read,
            pending: None,
            encoder: SessionOutputEncoder::new(identifier),
        });
        ControlAuthorization::Verified(ActorSocket {
            lease,
            credential,
            input,
            demand,
            retained: Arc::clone(&self.retained),
            activity: Arc::clone(&self.activity),
        })
    }
}

impl Active {
    fn drive<B: InputBackend + 'static>(
        &mut self,
        actor: &mut SessionActor<'_, B, DeviceId>,
        now: Duration,
    ) {
        if let Ok(input) = self.input.try_recv() {
            if input.receipt.is_closed() {
                self.lease.disconnect();
                return;
            }
            let admission = actor.receive(now, &self.lease, input.payload.as_ref());
            if input.receipt.send(admission).is_err() || admission.is_err() {
                self.lease.disconnect();
                return;
            }
        }
        if self.pending.is_none() {
            self.pending = self.demand.try_recv().ok();
        }
        if let Some(demand) = &self.pending {
            if demand.is_closed() {
                self.lease.disconnect();
            } else if let Some(output) = actor.next_output(now, &self.lease) {
                let demand = self.pending.take().expect("pending output demand");
                let delivered = self
                    .encoder
                    .encode(output)
                    .is_ok_and(|bytes| demand.send(bytes).is_ok());
                if !delivered {
                    self.lease.disconnect();
                }
            }
        }
    }
}

impl ActorSocket {
    fn expect_output(&self) -> Option<oneshot::Receiver<Vec<u8>>> {
        let (reply, result) = oneshot::channel();
        self.demand.try_send(reply).ok()?;
        self.activity.notify_one();
        Some(result)
    }

    async fn accept_input(&self, payload: ControlPayload) -> Result<(), SessionError> {
        let budget = RetainedBytes::reserve(&self.retained, payload.as_ref().len())
            .ok_or(SessionError::Capacity)?;
        let (receipt, result) = oneshot::channel();
        self.input
            .try_send(Input {
                payload,
                receipt,
                _budget: budget,
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => SessionError::Capacity,
                mpsc::error::TrySendError::Closed(_) => SessionError::Closed,
            })?;
        self.activity.notify_one();
        timeout(ACTOR_DEADLINE, result)
            .await
            .map_err(|_| SessionError::Unavailable)?
            .map_err(|_| SessionError::Closed)?
    }
}

impl ControlSession for ActorSocket {
    fn is_current(&self) -> bool {
        !self.lease.is_closed() && self.credential.is_current()
    }

    async fn run<I: AsyncRead + AsyncWrite + Unpin + Send>(
        &mut self,
        socket: &mut ControlSocket<'_, I>,
    ) {
        let Some(mut output) = self.expect_output() else {
            return;
        };
        while self.is_current() {
            // Dropping a pending read retains its binary-message deadline.
            let event = match select(&mut output, pin!(socket.receive())).await {
                Either::Left((ready, _)) => Either::Left(ready),
                Either::Right((ready, _)) => Either::Right(ready),
            };
            match event {
                Either::Left(Ok(bytes)) if self.is_current() => {
                    if socket.send(bytes).await.is_err() {
                        break;
                    }
                    let Some(next) = self.expect_output() else {
                        break;
                    };
                    output = next;
                }
                Either::Right(Ok(payload)) if self.is_current() => {
                    if let Err(error) = self.accept_input(payload).await {
                        self.lease.disconnect();
                        socket.set_close_code(error.close_code());
                        break;
                    }
                }
                _ => break,
            }
        }
    }
}
