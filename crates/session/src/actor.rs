//! Single-writer control authority over one ordered input dispatcher.

mod boundary;
mod quiescence;
mod rate;
mod wire_output;
pub use boundary::{
    APPLICATION_HEARTBEAT, Availability, ConnectionLease, ControlRole, ControlSnapshot,
    CredentialAuthority, GlobalStop, HeartbeatPolicy, SafetySignal, SessionError, SessionOutput,
    SessionSnapshot, TransportGeneration, VerifiedConnection,
};
pub use quiescence::{QuiescenceError, QuiescenceMode, QuiescenceTicket};
pub use wire_output::SessionOutputEncoder;

use std::collections::VecDeque;
use std::mem::size_of;
use std::num::NonZeroU64;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tether_input_core::{
    BackendStatus, DispatchOutcome, DispatchReceipt, DispatcherMode, EmergencyReason, InputAction,
    InputBackend, InputDispatcher, PermissionState, ReleaseTicket,
};
use tether_protocol::validation::{
    ClientMessage, Limits, Operation, TimedOperation, ValidationError, capability_snapshot,
    validate_client_hello,
};

use crate::admission::{AdmissionCursor, AdmissionError, AdmissionFacts, BarrierKind};
use boundary::{ConnectionId, LeaseState};
use quiescence::Observation;

const MAX_CONNECTIONS: usize = 8;
const MAX_OUTPUTS: usize = 8;
const MAX_ACTIONS: usize = 256;
const MAX_RETAINED_BYTES: usize = 128 * 1024;
const HELLO_TIMEOUT: Duration = Duration::from_secs(2);
const RELEASE_TIMEOUT: Duration = Duration::from_secs(2);
static NEXT_EPOCH: AtomicU64 = AtomicU64::new(1);

const fn wire_rejection(error: ValidationError) -> SessionError {
    match error {
        ValidationError::IncompatibleVersion
        | ValidationError::CapabilityUnavailable
        | ValidationError::BackendUnavailable
        | ValidationError::MessageTooLarge => SessionError::Protocol(error),
        _ => SessionError::InvalidMessage,
    }
}

fn allocate_epoch(counter: &AtomicU64) -> Option<u64> {
    counter
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .ok()
}

struct Connection<D> {
    id: ConnectionId,
    lease: Arc<LeaseState>,
    authority: VerifiedConnection<D>,
    deadline: Duration,
    cursor: Option<AdmissionCursor>,
    output: VecDeque<SessionOutput>,
    ingress: rate::Rate,
}

struct Release {
    ticket: ReleaseTicket,
    deadline: Duration,
    reason: EmergencyReason,
    next: Availability,
    grant: Option<ConnectionId>,
}

struct PendingAction {
    receipt: DispatchReceipt,
    bytes: usize,
}

/// The reactor owns this value on one thread and calls `poll` even without
/// traffic. The composition root retains the dispatcher and its worker join.
/// The transport delivers bounded messages one at a time through `receive`.
pub struct SessionActor<'d, B: InputBackend + 'static, D: Copy + Eq> {
    dispatcher: &'d mut InputDispatcher<B>,
    safety: SafetySignal,
    now: Duration,
    availability: Availability,
    generation: Option<TransportGeneration>,
    transport_retired: bool,
    epoch: u64,
    next_connection: Option<NonZeroU64>,
    backend: BackendStatus,
    limits: Limits,
    connections: Vec<Connection<D>>,
    owner: Option<ConnectionId>,
    confirmed_neutral: bool,
    release: Option<Release>,
    quiescence: Option<Observation>,
    pending: VecDeque<PendingAction>,
    pending_bytes: usize,
    maximum_actions: usize,
    ingress: rate::Rate,
}

impl<'d, B: InputBackend + 'static, D: Copy + Eq> SessionActor<'d, B, D> {
    /// Starts with an independent neutralization barrier and no input owner.
    #[must_use]
    pub fn new(
        dispatcher: &'d mut InputDispatcher<B>,
        generation: TransportGeneration,
        backend: BackendStatus,
        limits: Limits,
        now: Duration,
    ) -> Self {
        Self::start(dispatcher, Some(generation), backend, limits, now)
    }

    /// Starts a neutral actor before a listener generation has been published.
    #[must_use]
    pub fn without_transport(
        dispatcher: &'d mut InputDispatcher<B>,
        backend: BackendStatus,
        limits: Limits,
        now: Duration,
    ) -> Self {
        Self::start(dispatcher, None, backend, limits, now)
    }

    fn start(
        dispatcher: &'d mut InputDispatcher<B>,
        generation: Option<TransportGeneration>,
        backend: BackendStatus,
        limits: Limits,
        now: Duration,
    ) -> Self {
        let maximum_actions = MAX_ACTIONS.min(dispatcher.queue_capacity());
        let mut actor = Self {
            dispatcher,
            safety: SafetySignal::default(),
            now,
            availability: Availability::Releasing,
            generation,
            transport_retired: generation.is_none(),
            epoch: 0,
            next_connection: Some(NonZeroU64::MIN),
            backend,
            limits,
            connections: Vec::with_capacity(MAX_CONNECTIONS),
            owner: None,
            confirmed_neutral: false,
            release: None,
            quiescence: None,
            pending: VecDeque::new(),
            pending_bytes: 0,
            maximum_actions,
            ingress: rate::Rate::shared(now),
        };
        if actor.backend.fault.is_some() {
            actor.stop(GlobalStop::Fault);
        } else {
            let next = if !actor.transport_retired && actor.backend_ready() {
                Availability::Ready
            } else {
                Availability::Suspended
            };
            actor.begin_release(EmergencyReason::ExplicitStop, next, None);
        }
        actor
    }

    #[must_use]
    pub fn safety_signal(&self) -> SafetySignal {
        self.safety.clone()
    }

    #[must_use]
    pub fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            availability: self.availability,
            epoch: self.epoch,
            has_owner: self.owner.is_some(),
            connections: self.connections.len(),
            pending_actions: self.pending.len(),
            confirmed_neutral: self.confirmed_neutral,
        }
    }

    /// Next actor deadline independent of transport traffic. The reactor also
    /// polls for reserved signals and native receipts on its bounded cadence.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Duration> {
        self.connections
            .iter()
            .map(|connection| connection.deadline.min(connection.authority.expires_at))
            .chain(self.release.iter().map(|release| release.deadline))
            .min()
    }

    /// Handles reserved signals, deadlines and receipts before normal work.
    pub fn poll(&mut self, now: Duration) {
        if self.availability == Availability::Stopped {
            self.safety.close_quiescence();
            return;
        }
        if now < self.now {
            self.stop(GlobalStop::Fault);
            return;
        }
        self.now = now;
        if let Some(reason) = [self.safety.take(), self.observe_quiescence()]
            .into_iter()
            .flatten()
            .max_by_key(|reason| *reason as u8)
        {
            self.stop(reason);
        }
        for index in (0..self.connections.len()).rev() {
            let Some(connection) = self.connections.get(index) else {
                continue;
            };
            if connection.lease.requested.load(Ordering::Acquire)
                || !connection.authority.credential.is_current()
                || connection.authority.expires_at <= now
                || connection.deadline <= now
            {
                self.remove(index);
            }
        }
        if self.release.is_some() {
            self.poll_release();
        } else if !matches!(
            self.availability,
            Availability::Faulted | Availability::Stopped
        ) {
            self.poll_actions();
        }
        self.settle_quiescence();
    }

    /// Reserves a fresh lease for a verified connection before the upgrade.
    ///
    /// # Errors
    /// Rejects retired generations, expired credentials, duplicate devices,
    /// exhausted capacity or a terminal actor. Input authority requires hello.
    pub fn register(
        &mut self,
        now: Duration,
        authority: VerifiedConnection<D>,
    ) -> Result<ConnectionLease, SessionError> {
        self.poll(now);
        if self.transport_retired
            || matches!(
                self.availability,
                Availability::Faulted | Availability::Stopped
            )
        {
            return Err(SessionError::Unavailable);
        }
        if Some(authority.generation) != self.generation
            || authority.expires_at <= now
            || !authority.credential.is_current()
        {
            return Err(SessionError::StaleAuthority);
        }
        if self.connections.len() == MAX_CONNECTIONS
            || self
                .connections
                .iter()
                .any(|connection| connection.authority.device == authority.device)
        {
            return Err(SessionError::Capacity);
        }
        let Some((id, deadline)) = self.next_connection.zip(now.checked_add(HELLO_TIMEOUT)) else {
            self.stop(GlobalStop::Fault);
            return Err(SessionError::InternalFault);
        };
        self.next_connection = id.checked_add(1);
        let id = ConnectionId(id);
        let state = Arc::new(LeaseState::default());
        self.connections.push(Connection {
            id,
            lease: Arc::clone(&state),
            authority,
            deadline,
            cursor: None,
            output: VecDeque::new(),
            ingress: rate::Rate::connection(now),
        });
        Ok(ConnectionLease { id, state })
    }

    /// Supplies one bounded binary message from the verified connection.
    ///
    /// # Errors
    /// Fails closed on stale leases, malformed and unauthorized input or
    /// exhausted queue capacity. Owner rejection triggers reserved release.
    pub fn receive(
        &mut self,
        now: Duration,
        lease: &ConnectionLease,
        bytes: &[u8],
    ) -> Result<(), SessionError> {
        self.poll(now);
        let index = self.locate(lease).ok_or(SessionError::Closed)?;
        let result = if self.connections[index].cursor.is_some() {
            self.receive_runtime(index, bytes)
        } else {
            self.negotiate(index, bytes)
        };
        if result == Err(SessionError::InternalFault) {
            self.stop(GlobalStop::Fault);
        } else if result.is_err()
            && let Some(index) = self.locate(lease)
        {
            self.remove(index);
        }
        result
    }

    /// Takes at most one output. The socket writer owns its own IO deadline.
    pub fn next_output(&mut self, now: Duration, lease: &ConnectionLease) -> Option<SessionOutput> {
        self.poll(now);
        let index = self.locate(lease)?;
        self.connections[index].output.pop_front()
    }

    /// Closes all observed leases of this device without releasing another
    /// device's input. The credential store must already forbid re-admission.
    pub fn revoke_device(&mut self, now: Duration, device: D) {
        // Mark before polling so the revoked connection cannot win a pending grant.
        for connection in &self.connections {
            if connection.authority.device == device {
                connection.lease.requested.store(true, Ordering::Release);
            }
        }
        self.poll(now);
    }

    /// Publishes the core's cached backend status. A change while input can be
    /// admitted invalidates the grant or pending grant and closes the epoch.
    pub fn update_backend(&mut self, now: Duration, status: BackendStatus) {
        let changed = status != self.backend;
        self.backend = status;
        if changed && self.backend.fault.is_some() {
            self.safety.request(GlobalStop::Fault);
        } else if changed
            && (self.owner.is_some()
                || !self.backend_ready()
                || self
                    .release
                    .as_ref()
                    .is_some_and(|release| release.grant.is_some()))
        {
            self.safety.request(GlobalStop::Lifecycle);
        }
        self.poll(now);
    }

    /// Explicit core recovery permits new requests after verified neutralization.
    /// A retired transport is replaced only with a strictly newer generation.
    ///
    /// # Errors
    /// Rejects missing neutrality, unsafe backend health, and stale generations.
    pub fn recover(
        &mut self,
        now: Duration,
        generation: TransportGeneration,
    ) -> Result<(), SessionError> {
        self.poll(now);
        if self.availability != Availability::Suspended
            || !self.confirmed_neutral
            || !self.backend_ready()
        {
            return Err(SessionError::Unavailable);
        }
        if (self.transport_retired && self.generation.is_some_and(|old| generation.0 <= old.0))
            || (!self.transport_retired && Some(generation) != self.generation)
        {
            return Err(SessionError::StaleAuthority);
        }
        self.generation = Some(generation);
        self.transport_retired = false;
        self.availability = Availability::Ready;
        self.publish_control();
        Ok(())
    }

    fn locate(&self, lease: &ConnectionLease) -> Option<usize> {
        self.connections
            .iter()
            .position(|value| value.id == lease.id && Arc::ptr_eq(&value.lease, &lease.state))
    }

    fn control(&self, connection: ConnectionId) -> ControlSnapshot {
        let role = match (self.availability, self.owner) {
            (Availability::Ready, Some(owner)) if owner == connection => ControlRole::Controlling,
            (Availability::Ready, Some(_)) => ControlRole::Standby,
            (Availability::Ready | Availability::Releasing, None) => ControlRole::NoController,
            _ => ControlRole::Suspended,
        };
        ControlSnapshot {
            epoch: self.epoch,
            role,
        }
    }

    fn publish_control(&mut self) {
        for index in (0..self.connections.len()).rev() {
            let Some(connection) = self.connections.get(index) else {
                continue;
            };
            let control = self.control(connection.id);
            let connection = &mut self.connections[index];
            if connection.cursor.is_some() {
                if connection.output.len() == MAX_OUTPUTS {
                    self.remove(index);
                } else {
                    connection.output.push_back(SessionOutput::Control(control));
                }
            }
        }
    }

    fn negotiate(&mut self, index: usize, bytes: &[u8]) -> Result<(), SessionError> {
        let hello = validate_client_hello(bytes, true, self.limits).map_err(wire_rejection)?;
        let Ok(cursor) = AdmissionCursor::new(self.epoch, hello.limits()) else {
            self.stop(GlobalStop::Fault);
            return Err(SessionError::InternalFault);
        };
        let control = self.control(self.connections[index].id);
        let capabilities = capability_snapshot(&self.backend, hello.limits())
            .map_err(|_| SessionError::InternalFault)?;
        let deadline = self
            .now
            .checked_add(APPLICATION_HEARTBEAT.timeout)
            .ok_or(SessionError::InternalFault)?;
        let connection = &mut self.connections[index];
        connection.output.push_back(SessionOutput::Hello {
            negotiation: hello,
            capabilities: Box::new(capabilities),
            control,
            heartbeat: APPLICATION_HEARTBEAT,
        });
        connection.deadline = deadline;
        connection.cursor = Some(cursor);
        Ok(())
    }

    fn receive_runtime(&mut self, index: usize, bytes: &[u8]) -> Result<(), SessionError> {
        let connection = &mut self.connections[index];
        let id = connection.id;
        let cursor = connection
            .cursor
            .as_mut()
            .ok_or(SessionError::InternalFault)?;
        let facts = AdmissionFacts {
            authenticated: !connection.lease.requested.load(Ordering::Acquire)
                && connection.authority.credential.is_current(),
            controls_input: self.owner == Some(id) && self.availability == Availability::Ready,
            authorization_revision: connection.authority.authority_revision.get(),
            epoch: self.epoch,
            backend: &self.backend,
        };
        let prepared = cursor.prepare(bytes, &facts).map_err(|error| match error {
            AdmissionError::Validation(error) => wire_rejection(error),
            _ => SessionError::InvalidMessage,
        })?;
        let barrier = prepared.barrier();
        if !matches!(
            barrier,
            Some(BarrierKind::RelinquishControl | BarrierKind::ReleaseAll)
        ) {
            if !connection
                .ingress
                .admit(&mut self.ingress, self.now, prepared.operations())
            {
                return Err(SessionError::RateLimited);
            }
            let actions = prepared.operations() - usize::from(barrier.is_some());
            let retained = actions * (size_of::<InputAction>() + size_of::<PendingAction>())
                + prepared.text_bytes();
            if self.pending.len() + actions > self.maximum_actions
                || self.pending_bytes + retained > MAX_RETAINED_BYTES
            {
                return Err(SessionError::Capacity);
            }
        }
        let message = cursor
            .commit(prepared, &facts)
            .map_err(|_| SessionError::InternalFault)?;
        if cursor.snapshot().next_sequence.is_none() {
            return Err(SessionError::Closed);
        }
        match message.into_message() {
            ClientMessage::Batch(operations) => {
                self.apply_batch(operations, barrier, id)?;
            }
            ClientMessage::Heartbeat(identifier) => {
                if connection.output.len() == MAX_OUTPUTS {
                    return Err(SessionError::Capacity);
                }
                connection.output.push_back(SessionOutput::Pong {
                    epoch: self.epoch,
                    identifier,
                });
                connection.deadline = self
                    .now
                    .checked_add(APPLICATION_HEARTBEAT.timeout)
                    .ok_or(SessionError::InternalFault)?;
            }
            ClientMessage::Leaving(_) => self.remove(index),
        }
        Ok(())
    }

    fn apply_batch(
        &mut self,
        operations: Vec<TimedOperation>,
        barrier: Option<BarrierKind>,
        id: ConnectionId,
    ) -> Result<(), SessionError> {
        if matches!(
            barrier,
            Some(BarrierKind::RelinquishControl | BarrierKind::ReleaseAll)
        ) {
            let (reason, grant) = if barrier == Some(BarrierKind::ReleaseAll) {
                (EmergencyReason::ExplicitStop, Some(id))
            } else {
                (EmergencyReason::OwnershipTransfer, None)
            };
            self.begin_release(reason, Availability::Ready, grant);
            return Ok(());
        }
        let actions: Vec<_> = operations
            .into_iter()
            .map(TimedOperation::into_operation)
            .filter_map(|op| {
                if let Operation::Input(action) = op {
                    Some(action)
                } else {
                    None
                }
            })
            .collect();
        let sizes: Vec<_> = actions
            .iter()
            .map(|action| {
                size_of::<InputAction>()
                    + size_of::<PendingAction>()
                    + if let InputAction::TextInsert(text) = action {
                        text.byte_len()
                    } else {
                        0
                    }
            })
            .collect();
        let bytes = sizes.iter().sum::<usize>();
        if self.safety.has_pending() {
            self.poll(self.now);
            return Err(SessionError::Unavailable);
        }
        if self.connections.iter().any(|value| {
            value.id == id
                && (value.lease.requested.load(Ordering::Acquire)
                    || !value.authority.credential.is_current())
        }) {
            return Err(SessionError::Closed);
        }
        let receipts = self
            .dispatcher
            .submit_batch(actions)
            .map_err(|_| SessionError::Unavailable)?;
        self.pending_bytes += bytes;
        self.pending.extend(
            receipts
                .into_iter()
                .zip(sizes)
                .map(|(receipt, bytes)| PendingAction { receipt, bytes }),
        );
        if barrier == Some(BarrierKind::RequestControl) {
            self.request_control(id)?;
        }
        Ok(())
    }

    fn request_control(&mut self, id: ConnectionId) -> Result<(), SessionError> {
        if self.availability == Availability::Ready
            && self.owner != Some(id)
            && self.backend_ready()
        {
            self.begin_release(
                EmergencyReason::OwnershipTransfer,
                Availability::Ready,
                Some(id),
            );
        } else {
            let control = self.control(id);
            let connection = self
                .connections
                .iter_mut()
                .find(|value| value.id == id)
                .ok_or(SessionError::Closed)?;
            let cursor = connection
                .cursor
                .as_mut()
                .ok_or(SessionError::InternalFault)?;
            cursor
                .resolve_request_without_transfer()
                .map_err(|_| SessionError::InternalFault)?;
            if connection.output.len() == MAX_OUTPUTS {
                return Err(SessionError::Capacity);
            }
            connection.output.push_back(SessionOutput::Control(control));
        }
        Ok(())
    }

    fn remove(&mut self, index: usize) {
        let removed = self.connections.swap_remove(index);
        removed.lease.closed.store(true, Ordering::Release);
        if let Some(release) = &mut self.release
            && release.grant == Some(removed.id)
        {
            release.grant = None;
        }
        if self.owner == Some(removed.id) {
            self.begin_release(EmergencyReason::ConnectionLost, Availability::Ready, None);
        }
    }

    fn close_all(&mut self) {
        for connection in self.connections.drain(..) {
            connection.lease.closed.store(true, Ordering::Release);
        }
    }

    fn stop(&mut self, reason: GlobalStop) {
        if self.availability == Availability::Stopped
            || (self.availability == Availability::Faulted && reason != GlobalStop::Shutdown)
        {
            return;
        }
        if self.release.as_ref().is_some_and(|release| {
            release.next == Availability::Stopped
                || (release.next == Availability::Faulted && reason != GlobalStop::Shutdown)
        }) {
            return;
        }
        let (emergency, next) = match reason {
            GlobalStop::RevokeDevices => {
                let suspended = self.availability == Availability::Suspended
                    || self
                        .release
                        .as_ref()
                        .is_some_and(|release| release.next == Availability::Suspended);
                self.close_all();
                let next = if suspended {
                    Availability::Suspended
                } else {
                    Availability::Ready
                };
                (EmergencyReason::ConnectionLost, next)
            }
            GlobalStop::StopControl => (EmergencyReason::ExplicitStop, Availability::Suspended),
            GlobalStop::Lifecycle | GlobalStop::RetireTransport => {
                (EmergencyReason::Lifecycle, Availability::Suspended)
            }
            GlobalStop::Fault => (EmergencyReason::BackendFault, Availability::Faulted),
            GlobalStop::Shutdown => (EmergencyReason::Shutdown, Availability::Stopped),
        };
        if matches!(
            reason,
            GlobalStop::RetireTransport | GlobalStop::Fault | GlobalStop::Shutdown
        ) {
            self.transport_retired = true;
            self.close_all();
        }
        self.begin_release(emergency, next, None);
    }

    fn begin_release(
        &mut self,
        reason: EmergencyReason,
        next: Availability,
        grant: Option<ConnectionId>,
    ) {
        if self.release.as_ref().is_some_and(|current| {
            current.reason == reason && current.next == next && current.grant == grant
        }) {
            return;
        }
        self.owner = None;
        self.availability = Availability::Releasing;
        self.confirmed_neutral = false;
        self.pending.clear();
        self.pending_bytes = 0;
        let requested = self.dispatcher.request_release(reason);
        let deadline = self
            .release
            .as_ref()
            .map(|release| release.deadline)
            .or_else(|| self.now.checked_add(RELEASE_TIMEOUT));
        if let (Ok(ticket), Some(deadline)) = (requested, deadline) {
            self.release = Some(Release {
                ticket,
                deadline,
                reason,
                next,
                grant,
            });
        } else {
            self.fail_without_proof();
        }
    }

    fn fail_without_proof(&mut self) {
        self.owner = None;
        self.confirmed_neutral = false;
        self.release = None;
        self.availability = Availability::Faulted;
        self.close_all();
        let _best_effort = self
            .dispatcher
            .request_release(EmergencyReason::BackendFault);
    }

    fn poll_release(&mut self) {
        let Some(release) = self.release.as_ref() else {
            return;
        };
        if self.now >= release.deadline {
            self.fail_without_proof();
            return;
        }
        let Some(outcome) = release.ticket.try_outcome() else {
            return;
        };
        let next = release.next;
        let grant = release.grant;
        self.release = None;
        let backend = self.dispatcher.snapshot();
        let expected_mode = match next {
            Availability::Stopped => DispatcherMode::Stopped,
            Availability::Faulted => DispatcherMode::Faulted,
            _ => DispatcherMode::Suspended,
        };
        if !outcome.report.is_confirmed_neutral()
            || backend.mode != expected_mode
            || backend.held_buttons != 0
            || backend.held_keys != 0
            || backend.queued_actions != 0
            || (expected_mode == DispatcherMode::Suspended
                && (!backend.worker_alive || backend.terminal_fault))
        {
            self.fail_without_proof();
            return;
        }
        let Some(epoch) = allocate_epoch(&NEXT_EPOCH) else {
            self.fail_without_proof();
            return;
        };
        for connection in &mut self.connections {
            if let Some(cursor) = &mut connection.cursor
                && cursor.advance_epoch(epoch).is_err()
            {
                self.fail_without_proof();
                return;
            }
        }
        self.epoch = epoch;
        self.confirmed_neutral = true;
        self.availability = next;
        if next == Availability::Ready
            && self.backend_ready()
            && let Some(id) = grant
            && self.can_grant(id)
        {
            if self.dispatcher.resume().is_err() {
                self.fail_without_proof();
                return;
            }
            self.owner = Some(id);
            self.confirmed_neutral = false;
        }
        self.publish_control();
    }

    fn can_grant(&self, id: ConnectionId) -> bool {
        !self.transport_retired
            && !self.safety.has_pending()
            && self.connections.iter().any(|connection| {
                connection.id == id
                    && connection.cursor.is_some()
                    && Some(connection.authority.generation) == self.generation
                    && connection.authority.expires_at > self.now
                    && connection.authority.credential.is_current()
                    && connection.output.len() < MAX_OUTPUTS
                    && !connection.lease.requested.load(Ordering::Acquire)
            })
    }

    fn poll_actions(&mut self) {
        let status = self.dispatcher.snapshot();
        if status.terminal_fault || !status.worker_alive {
            self.stop(GlobalStop::Fault);
            return;
        }
        if self.owner.is_some() && status.mode != DispatcherMode::Running {
            self.stop(GlobalStop::Lifecycle);
            return;
        }
        while let Some(front) = self.pending.front_mut() {
            match front.receipt.try_outcome() {
                Ok(None) => break,
                Ok(Some(DispatchOutcome::Executed { .. })) => {
                    self.pending_bytes -= front.bytes;
                    self.pending.pop_front();
                }
                Ok(Some(DispatchOutcome::BackendRejected { error, .. }))
                    if !error.requires_emergency_release() =>
                {
                    self.stop(GlobalStop::Lifecycle);
                    break;
                }
                _ => {
                    self.stop(GlobalStop::Fault);
                    break;
                }
            }
        }
    }

    fn backend_ready(&self) -> bool {
        self.backend.initialized
            && self.backend.available
            && self.backend.fault.is_none()
            && matches!(
                self.backend.permission,
                PermissionState::Granted | PermissionState::NotRequired
            )
    }
}

impl<B: InputBackend + 'static, D: Copy + Eq> Drop for SessionActor<'_, B, D> {
    fn drop(&mut self) {
        self.safety.close_quiescence();
        self.close_all();
        if self.availability != Availability::Stopped {
            let reason = if std::thread::panicking() {
                EmergencyReason::WorkerPanic
            } else {
                EmergencyReason::Shutdown
            };
            let _best_effort = self.dispatcher.request_release(reason);
        }
    }
}

#[cfg(test)]
mod tests;
