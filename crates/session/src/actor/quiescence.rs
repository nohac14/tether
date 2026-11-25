use super::{Availability, GlobalStop, SafetySignal, SessionActor};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::task::Poll;
use tether_input_core::InputBackend;

/// The stronger shutdown request supersedes a network retirement in flight.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum QuiescenceMode {
    RetireTransport,
    Shutdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuiescenceError {
    Unavailable,
    NotNeutral,
}

/// One observed release result. Dropping the ticket does not withdraw the request.
pub struct QuiescenceTicket {
    mode: QuiescenceMode,
    result: Arc<AtomicU8>,
}

const RETIRED: u8 = 1;
const STOPPED: u8 = 2;
const NOT_NEUTRAL: u8 = 3;
const UNAVAILABLE: u8 = 4;

impl QuiescenceTicket {
    pub fn poll(&self) -> Poll<Result<(), QuiescenceError>> {
        match self.result.load(Ordering::Acquire) {
            RETIRED if self.mode == QuiescenceMode::RetireTransport => Poll::Ready(Ok(())),
            STOPPED if self.mode == QuiescenceMode::Shutdown => Poll::Ready(Ok(())),
            NOT_NEUTRAL => Poll::Ready(Err(QuiescenceError::NotNeutral)),
            RETIRED | STOPPED | UNAVAILABLE => Poll::Ready(Err(QuiescenceError::Unavailable)),
            _ => Poll::Pending,
        }
    }
}

#[derive(Clone)]
struct Request {
    mode: QuiescenceMode,
    result: Arc<AtomicU8>,
}

impl Request {
    fn matches(&self, other: &Self) -> bool {
        self.mode == other.mode && Arc::ptr_eq(&self.result, &other.result)
    }
}

#[derive(Default)]
struct Slot {
    request: Option<Request>,
    closed: bool,
}

#[derive(Default)]
pub(super) struct Mailbox(Mutex<Slot>);

impl Mailbox {
    fn slot(&self) -> MutexGuard<'_, Slot> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn has_pending(&self) -> bool {
        self.slot().request.is_some()
    }
}

impl SafetySignal {
    /// Reserves the sole cross-service cleanup slot. A harder stop reuses its
    /// ticket and keeps normal input fenced until the stronger stop completes.
    ///
    /// # Errors
    /// Returns `Unavailable` if the receiving actor has closed.
    pub fn request_quiescence(
        &self,
        mode: QuiescenceMode,
    ) -> Result<QuiescenceTicket, QuiescenceError> {
        let mut slot = self.quiescence.slot();
        if slot.closed {
            return Err(QuiescenceError::Unavailable);
        }
        let request = slot.request.get_or_insert_with(|| Request {
            mode,
            result: Arc::new(AtomicU8::new(0)),
        });
        request.mode = request.mode.max(mode);
        Ok(QuiescenceTicket {
            mode,
            result: Arc::clone(&request.result),
        })
    }

    pub(super) fn close_quiescence(&self) {
        let mut slot = self.quiescence.slot();
        slot.closed = true;
        if let Some(request) = slot.request.take() {
            request.result.store(UNAVAILABLE, Ordering::Release);
        }
    }
}

pub(super) struct Observation {
    request: Request,
    previous_epoch: u64,
}

impl<B: InputBackend + 'static, D: Copy + Eq> SessionActor<'_, B, D> {
    pub(super) fn observe_quiescence(&mut self) -> Option<GlobalStop> {
        let request = self.safety.quiescence.slot().request.clone()?;
        if self
            .quiescence
            .as_ref()
            .is_some_and(|prior| prior.request.matches(&request))
        {
            return None;
        }
        let reason = match request.mode {
            QuiescenceMode::RetireTransport => GlobalStop::RetireTransport,
            QuiescenceMode::Shutdown => GlobalStop::Shutdown,
        };
        self.quiescence = Some(Observation {
            request,
            previous_epoch: self.epoch,
        });
        Some(reason)
    }

    pub(super) fn settle_quiescence(&mut self) {
        if let Some(observed) = &self.quiescence {
            let stopped = self.availability == Availability::Stopped;
            let suspended = self.availability == Availability::Suspended;
            let result = if self.availability == Availability::Faulted {
                NOT_NEUTRAL
            } else if (stopped || suspended)
                && self.transport_retired
                && self.confirmed_neutral
                && self.epoch > observed.previous_epoch
                && self.owner.is_none()
                && self.connections.is_empty()
                && self.pending.is_empty()
                && (stopped || observed.request.mode == QuiescenceMode::RetireTransport)
            {
                if stopped { STOPPED } else { RETIRED }
            } else {
                return;
            };
            let mut slot = self.safety.quiescence.slot();
            if slot
                .request
                .as_ref()
                .is_some_and(|request| request.matches(&observed.request))
            {
                observed.request.result.store(result, Ordering::Release);
                slot.request = None;
                self.quiescence = None;
            }
        }
        if self.availability == Availability::Stopped {
            self.safety.close_quiescence();
        }
    }
}
