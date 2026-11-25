//! Connects supervisor quiescence to the session actor's reserved release path.

use std::num::NonZeroU64;
use std::task::Poll;
use tether_network::supervisor::{NeutralityTicket, ReleaseCause, SafetyCoordinator, SafetyError};
use tether_session::actor::{QuiescenceError, QuiescenceMode, QuiescenceTicket, SafetySignal};

pub struct ListenerSessionSafety(SafetySignal);

impl ListenerSessionSafety {
    #[must_use]
    pub const fn new(signal: SafetySignal) -> Self {
        Self(signal)
    }
}

pub struct SessionNeutrality(QuiescenceTicket);

impl NeutralityTicket for SessionNeutrality {
    fn poll_neutrality(&mut self) -> Poll<Result<(), SafetyError>> {
        self.0.poll().map(|result| result.map_err(safety_error))
    }
}

impl SafetyCoordinator for ListenerSessionSafety {
    type Ticket = SessionNeutrality;

    fn request_release(
        &mut self,
        cause: ReleaseCause,
        _obsolete: Option<NonZeroU64>,
    ) -> Result<Self::Ticket, SafetyError> {
        // A listener transition fences every session, including startup before
        // the first transport generation exists. Cleanup is never scoped away.
        let mode = match cause {
            ReleaseCause::Shutdown => QuiescenceMode::Shutdown,
            ReleaseCause::StartOrRebind
            | ReleaseCause::Stop
            | ReleaseCause::InterfaceChanged
            | ReleaseCause::TransportFault => QuiescenceMode::RetireTransport,
        };
        self.0
            .request_quiescence(mode)
            .map(SessionNeutrality)
            .map_err(safety_error)
    }
}

const fn safety_error(error: QuiescenceError) -> SafetyError {
    match error {
        QuiescenceError::Unavailable => SafetyError::Unavailable,
        QuiescenceError::NotNeutral => SafetyError::NotNeutral,
    }
}
