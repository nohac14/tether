use std::num::NonZeroU64;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::Duration;

use super::quiescence::Mailbox;
use tether_protocol::validation::ValidatedHello;

/// One admitted transport generation. This is distinct from an ownership epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportGeneration(pub NonZeroU64);

/// Shared revocation state for one credential from the pairing coordinator.
#[derive(Clone)]
pub struct CredentialAuthority(Arc<AtomicBool>);

impl CredentialAuthority {
    #[must_use]
    pub fn active() -> Self {
        Self(Arc::new(AtomicBool::new(true)))
    }

    #[must_use]
    pub fn is_current(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    pub fn revoke(&self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Facts supplied by the trusted, bounded upgrade verifier after peer, Host,
/// Origin and credential checks. Browser messages never construct these facts.
pub struct VerifiedConnection<D> {
    pub device: D,
    pub authority_revision: NonZeroU64,
    pub generation: TransportGeneration,
    pub expires_at: Duration,
    pub credential: CredentialAuthority,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct ConnectionId(pub(super) NonZeroU64);

#[derive(Default)]
pub(super) struct LeaseState {
    pub(super) requested: AtomicBool,
    pub(super) closed: AtomicBool,
}

/// Connection-specific reserved teardown. The transport can share this handle
/// via an `Arc`. Its final drop requests closure independently of normal work.
pub struct ConnectionLease {
    pub(super) id: ConnectionId,
    pub(super) state: Arc<LeaseState>,
}

impl std::fmt::Debug for ConnectionLease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ConnectionLease(<redacted>)")
    }
}

impl ConnectionLease {
    pub fn disconnect(&self) {
        self.state.requested.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.state.closed.load(Ordering::Acquire)
    }
}

impl Drop for ConnectionLease {
    fn drop(&mut self) {
        self.disconnect();
    }
}

/// Global commands coalesce by severity. They never need normal queue space.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum GlobalStop {
    RevokeDevices = 1,
    StopControl = 2,
    Lifecycle = 3,
    RetireTransport = 4,
    Fault = 5,
    Shutdown = 6,
}

/// Cloneable reserved delivery, shared with infrastructure outside the actor.
#[derive(Clone, Default)]
pub struct SafetySignal {
    stop: Arc<AtomicU8>,
    pub(super) quiescence: Arc<Mailbox>,
}

impl SafetySignal {
    pub fn request(&self, reason: GlobalStop) {
        self.stop.fetch_max(reason as u8, Ordering::AcqRel);
    }

    pub(super) fn has_pending(&self) -> bool {
        self.stop.load(Ordering::Acquire) != 0 || self.quiescence.has_pending()
    }

    pub(super) fn take(&self) -> Option<GlobalStop> {
        match self.stop.swap(0, Ordering::AcqRel) {
            1 => Some(GlobalStop::RevokeDevices),
            2 => Some(GlobalStop::StopControl),
            3 => Some(GlobalStop::Lifecycle),
            4 => Some(GlobalStop::RetireTransport),
            5 => Some(GlobalStop::Fault),
            6 => Some(GlobalStop::Shutdown),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Availability {
    Ready,
    Releasing,
    Suspended,
    Faulted,
    Stopped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlRole {
    NoController,
    Standby,
    Controlling,
    Suspended,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlSnapshot {
    pub epoch: u64,
    pub role: ControlRole,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeartbeatPolicy {
    pub interval: Duration,
    pub timeout: Duration,
}

pub const APPLICATION_HEARTBEAT: HeartbeatPolicy = HeartbeatPolicy {
    interval: Duration::from_secs(1),
    timeout: Duration::from_secs(4),
};

/// Mandatory bounded output for the connection writer. No socket writes occur
/// on the actor. Hello diagnostics are redacted by the protocol layer.
#[derive(Debug)]
pub enum SessionOutput {
    Hello {
        negotiation: ValidatedHello,
        capabilities: Box<tether_protocol::wire::CapabilitySnapshot>,
        control: ControlSnapshot,
        heartbeat: HeartbeatPolicy,
    },
    Control(ControlSnapshot),
    Pong {
        epoch: u64,
        identifier: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionError {
    Unavailable,
    Capacity,
    StaleAuthority,
    Closed,
    InvalidMessage,
    Protocol(tether_protocol::validation::ValidationError),
    RateLimited,
    InternalFault,
}

impl SessionError {
    /// Closed category for the WebSocket peer. The actor releases first.
    #[must_use]
    pub const fn close_code(self) -> u16 {
        match self {
            Self::Protocol(error) => error.close_code(),
            Self::StaleAuthority => 4001,
            Self::InvalidMessage | Self::RateLimited => 1008,
            Self::Capacity => 1013,
            Self::Unavailable | Self::InternalFault => 1011,
            Self::Closed => 1000,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionSnapshot {
    pub availability: Availability,
    pub epoch: u64,
    pub has_owner: bool,
    pub connections: usize,
    pub pending_actions: usize,
    pub confirmed_neutral: bool,
}
