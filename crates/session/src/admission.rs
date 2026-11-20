//! Stateful, side-effect-free runtime admission for the future serialized actor.
//! This module has no dispatcher, transport, credential verifier or native handle.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use tether_input_core::{BackendStatus, ButtonAction, InputAction, KeyAction};
use tether_protocol::validation::{
    ClientContext, ClientMessage, HeldState, Limits, Operation, ValidatedClient, ValidationError,
    next_sequence, validate_client,
};

static NEXT_CURSOR: AtomicU64 = AtomicU64::new(1);

/// Trusted current facts from the serialized actor/verifier, never a wire claim.
pub struct AdmissionFacts<'a> {
    pub authenticated: bool,
    pub controls_input: bool,
    pub authorization_revision: u64,
    pub epoch: u64,
    pub backend: &'a BackendStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionError {
    Validation(ValidationError),
    InvalidFacts,
    PendingBarrier,
    StalePreparation,
    CounterExhausted,
    InvalidTransition,
}
impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "session admission: {self:?}")
    }
}
impl std::error::Error for AdmissionError {}
impl From<ValidationError> for AdmissionError {
    fn from(value: ValidationError) -> Self {
        Self::Validation(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BarrierKind {
    RequestControl,
    RelinquishControl,
    ReleaseAll,
}

#[derive(Clone, Eq, PartialEq)]
struct FactSnapshot {
    authenticated: bool,
    controls_input: bool,
    authorization_revision: u64,
    epoch: u64,
    backend: BackendStatus,
}
impl From<&AdmissionFacts<'_>> for FactSnapshot {
    fn from(value: &AdmissionFacts<'_>) -> Self {
        Self {
            authenticated: value.authenticated,
            controls_input: value.controls_input,
            authorization_revision: value.authorization_revision,
            epoch: value.epoch,
            backend: value.backend.clone(),
        }
    }
}

/// Owned preparation; dropping it changes nothing and never executes input.
pub struct PreparedEnvelope {
    cursor: u64,
    revision: u64,
    next_revision: u64,
    facts: FactSnapshot,
    message: ValidatedClient,
    projection: Option<HeldState>,
    barrier: Option<BarrierKind>,
    operations: usize,
}
impl PreparedEnvelope {
    /// Expanded logical operation count for reserving bounded actor-local work.
    #[must_use]
    pub const fn operations(&self) -> usize {
        self.operations
    }
    #[must_use]
    pub const fn barrier(&self) -> Option<BarrierKind> {
        self.barrier
    }
}

/// Expected ordered state, explicitly not a native held-state/neutrality report.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct AdmissionSnapshot {
    pub epoch: u64,
    pub next_sequence: Option<u64>,
    pub last_timestamp_micros: u64,
    pub last_heartbeat_identifier: Option<u64>,
    pub projected_buttons: usize,
    pub projected_keys: usize,
    pub pending_barrier: Option<BarrierKind>,
}

/// Connection-local ordered admission. The actor still owns all real authority.
pub struct AdmissionCursor {
    nonce: u64,
    revision: u64,
    epoch: u64,
    next_sequence: Option<u64>,
    last_timestamp: u64,
    last_heartbeat: Option<u64>,
    limits: Limits,
    projection: HeldState,
    pending: Option<BarrierKind>,
}

macro_rules! redacted {
    ($($name:ident),+ $(,)?) => {$(
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($name), "(<redacted>)"))
            }
        }
    )+};
}
redacted!(AdmissionCursor, AdmissionSnapshot, PreparedEnvelope);
impl fmt::Debug for AdmissionFacts<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AdmissionFacts(<redacted>)")
    }
}

fn nonce(counter: &AtomicU64) -> Result<u64, AdmissionError> {
    counter
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| AdmissionError::CounterExhausted)
}

impl AdmissionCursor {
    /// Creates an empty post-negotiation cursor, never restoring held state.
    /// Its private process-local nonce is not an installation or authentication ID.
    ///
    /// # Errors
    /// Fails closed when private instance-counter space is exhausted.
    pub fn new(epoch: u64, limits: Limits) -> Result<Self, AdmissionError> {
        Ok(Self {
            nonce: nonce(&NEXT_CURSOR)?,
            revision: 0,
            epoch,
            next_sequence: Some(1),
            last_timestamp: 0,
            last_heartbeat: None,
            limits,
            projection: HeldState::default(),
            pending: None,
        })
    }

    #[must_use]
    pub fn snapshot(&self) -> AdmissionSnapshot {
        AdmissionSnapshot {
            epoch: self.epoch,
            next_sequence: self.next_sequence,
            last_timestamp_micros: self.last_timestamp,
            last_heartbeat_identifier: self.last_heartbeat,
            projected_buttons: self.projection.buttons.len(),
            projected_keys: self.projection.keys.len(),
            pending_barrier: self.pending,
        }
    }

    /// Validates the entire envelope against the current ordered projection.
    /// No cursor mutation, queue admission, input execution or authority grant.
    ///
    /// # Errors
    /// Rejects invalid wire/context, counter exhaustion or input behind a barrier.
    pub fn prepare(
        &self,
        bytes: &[u8],
        facts: &AdmissionFacts<'_>,
    ) -> Result<PreparedEnvelope, AdmissionError> {
        if facts.epoch != self.epoch || facts.authorization_revision == 0 {
            return Err(AdmissionError::InvalidFacts);
        }
        let next_revision = self
            .revision
            .checked_add(1)
            .ok_or(AdmissionError::CounterExhausted)?;
        let message = validate_client(
            bytes,
            &ClientContext {
                authenticated: facts.authenticated,
                controls_input: facts.controls_input,
                epoch: facts.epoch,
                next_sequence: self.next_sequence.ok_or(AdmissionError::CounterExhausted)?,
                last_timestamp_micros: self.last_timestamp,
                last_heartbeat_identifier: self.last_heartbeat,
                limits: self.limits,
                backend: facts.backend,
                held: &self.projection,
            },
        )?;
        let mut projection = None;
        let mut barrier = None;
        let mut operations = 0;
        if let ClientMessage::Batch(values) = message.message() {
            if self.pending.is_some() {
                return Err(AdmissionError::PendingBarrier);
            }
            operations = values.len();
            let mut after = self.projection.clone();
            for value in values {
                match value.operation() {
                    Operation::Input(action) => project(&mut after, action),
                    Operation::RequestControl => barrier = Some(BarrierKind::RequestControl),
                    Operation::RelinquishControl => barrier = Some(BarrierKind::RelinquishControl),
                    Operation::ReleaseAll => barrier = Some(BarrierKind::ReleaseAll),
                }
            }
            projection = Some(after);
        }
        Ok(PreparedEnvelope {
            cursor: self.nonce,
            revision: self.revision,
            next_revision,
            facts: FactSnapshot::from(facts),
            message,
            projection,
            barrier,
            operations,
        })
    }

    /// Commits alongside a reserved actor-local queue slot, never after injection.
    /// Reserve capacity first; do not submit native work before this succeeds.
    /// The serialized actor must discard its reservation if context became stale.
    ///
    /// # Errors
    /// Rejects cross-cursor/obsolete preparation or changed current facts without
    /// changing sequence, projected held state, timestamps or barrier state.
    pub fn commit(
        &mut self,
        value: PreparedEnvelope,
        facts: &AdmissionFacts<'_>,
    ) -> Result<ValidatedClient, AdmissionError> {
        if value.cursor != self.nonce
            || value.revision != self.revision
            || value.facts != FactSnapshot::from(facts)
        {
            return Err(AdmissionError::StalePreparation);
        }
        self.revision = value.next_revision;
        self.next_sequence = next_sequence(value.message.sequence());
        self.last_timestamp = value.message.timestamp_micros();
        if let ClientMessage::Heartbeat(identifier) = value.message.message() {
            self.last_heartbeat = Some(*identifier);
        }
        if let Some(after) = value.projection {
            self.projection = after;
        }
        if value.barrier.is_some() {
            self.pending = value.barrier;
        }
        Ok(value.message)
    }

    /// Installs a strictly newer actor-authorized epoch after its safety barrier.
    /// This is not itself evidence of release or permission to resume a dispatcher.
    /// Connection sequence/timestamps/heartbeat IDs are deliberately retained.
    ///
    /// # Errors
    /// Rejects epoch reuse/regression or revision overflow without partial reset.
    pub fn advance_epoch(&mut self, epoch: u64) -> Result<(), AdmissionError> {
        if epoch <= self.epoch {
            return Err(AdmissionError::InvalidTransition);
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(AdmissionError::CounterExhausted)?;
        self.revision = revision;
        self.epoch = epoch;
        self.projection = HeldState::default();
        self.pending = None;
        Ok(())
    }

    /// Completes a denied/idempotent ownership request, never a release barrier.
    ///
    /// # Errors
    /// Rejects missing/non-request barriers or revision overflow with no mutation.
    pub fn resolve_request_without_transfer(&mut self) -> Result<(), AdmissionError> {
        if self.pending != Some(BarrierKind::RequestControl) {
            return Err(AdmissionError::InvalidTransition);
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(AdmissionError::CounterExhausted)?;
        self.revision = revision;
        self.pending = None;
        Ok(())
    }
}

fn project(held: &mut HeldState, action: &InputAction) {
    match action {
        InputAction::MouseButton(ButtonAction::Down(button)) => {
            held.buttons.insert(*button);
        }
        InputAction::MouseButton(ButtonAction::Up(button)) => {
            held.buttons.remove(button);
        }
        InputAction::PhysicalKey(KeyAction::Down(key)) => {
            held.keys.insert(*key);
        }
        InputAction::PhysicalKey(KeyAction::Up(key)) => {
            held.keys.remove(key);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
