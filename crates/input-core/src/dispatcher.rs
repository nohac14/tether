use crate::{
    BackendError, BackendErrorCategory, ButtonAction, InputAction, InputBackend, KeyAction,
    MouseButton, PhysicalKey, PointerDelta, ReleaseFailure, ReleaseReport, RetryScope, ScrollDelta,
};
use std::collections::VecDeque;
use std::fmt;
use std::io;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Default bound on accepted logical actions waiting for native execution.
pub const DEFAULT_QUEUE_CAPACITY: usize = 256;

/// Largest queue bound accepted without an architectural review.
pub const MAX_QUEUE_CAPACITY: usize = 4_096;

/// Monotonic position assigned to accepted actions and release barriers.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DispatchPosition(u64);

impl DispatchPosition {
    /// Returns the safe numeric position.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Validated dispatcher resource bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DispatcherConfig {
    queue_capacity: usize,
}

impl DispatcherConfig {
    /// Creates a dispatcher configuration with a bounded non-zero queue.
    ///
    /// # Errors
    ///
    /// Returns [`DispatcherConfigError`] when the capacity is zero or exceeds
    /// [`MAX_QUEUE_CAPACITY`].
    pub const fn new(queue_capacity: usize) -> Result<Self, DispatcherConfigError> {
        if queue_capacity == 0 || queue_capacity > MAX_QUEUE_CAPACITY {
            Err(DispatcherConfigError)
        } else {
            Ok(Self { queue_capacity })
        }
    }

    /// Returns the maximum number of waiting logical actions.
    #[must_use]
    pub const fn queue_capacity(self) -> usize {
        self.queue_capacity
    }
}

impl Default for DispatcherConfig {
    fn default() -> Self {
        Self {
            queue_capacity: DEFAULT_QUEUE_CAPACITY,
        }
    }
}

/// Invalid dispatcher configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DispatcherConfigError;

impl fmt::Display for DispatcherConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("dispatcher queue capacity is outside the allowed range")
    }
}

impl std::error::Error for DispatcherConfigError {}

/// Current admission/lifecycle state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatcherMode {
    Running,
    Releasing,
    Suspended,
    Faulted,
    Stopped,
}

/// Cause routed through the reserved release mailbox.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmergencyReason {
    ExplicitStop,
    ConnectionLost,
    OwnershipTransfer,
    Lifecycle,
    QueueSaturated,
    PositionExhausted,
    InvalidTransition,
    BackendFault,
    WorkerPanic,
    Shutdown,
}

impl EmergencyReason {
    const fn priority(self) -> u8 {
        match self {
            Self::ExplicitStop => 0,
            Self::ConnectionLost | Self::OwnershipTransfer | Self::Lifecycle => 1,
            Self::QueueSaturated | Self::PositionExhausted | Self::InvalidTransition => 2,
            Self::BackendFault => 3,
            Self::WorkerPanic => 4,
            Self::Shutdown => 5,
        }
    }

    fn mode_after_release(self, neutral: bool) -> DispatcherMode {
        if self == Self::Shutdown {
            DispatcherMode::Stopped
        } else if !neutral
            || matches!(
                self,
                Self::QueueSaturated
                    | Self::PositionExhausted
                    | Self::InvalidTransition
                    | Self::BackendFault
                    | Self::WorkerPanic
            )
        {
            DispatcherMode::Faulted
        } else {
            DispatcherMode::Suspended
        }
    }

    const fn establishes_terminal_fault(self) -> bool {
        matches!(
            self,
            Self::QueueSaturated
                | Self::PositionExhausted
                | Self::InvalidTransition
                | Self::BackendFault
                | Self::WorkerPanic
        )
    }
}

/// Why an action was rejected before native execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RejectionReason {
    DuplicateDown,
    UnmatchedUp,
    ButtonAlreadyHeld,
}

/// Why accepted but not-yet-executed work was cancelled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancellationReason {
    Emergency(EmergencyReason),
    WorkerStopped,
}

/// Completion of one accepted logical action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchOutcome {
    Executed {
        position: DispatchPosition,
    },
    BackendRejected {
        position: DispatchPosition,
        error: BackendError,
    },
    Rejected {
        position: DispatchPosition,
        reason: RejectionReason,
    },
    Cancelled {
        position: DispatchPosition,
        reason: CancellationReason,
    },
}

impl DispatchOutcome {
    /// Returns the position assigned at admission.
    #[must_use]
    pub const fn position(self) -> DispatchPosition {
        match self {
            Self::Executed { position }
            | Self::BackendRejected { position, .. }
            | Self::Rejected { position, .. }
            | Self::Cancelled { position, .. } => position,
        }
    }
}

/// Failure before an ordinary action can enter the bounded queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionError {
    NotAdmitting(DispatcherMode),
    QueueSaturated { release_position: DispatchPosition },
    PositionExhausted,
    WorkerStopped,
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotAdmitting(_) => "dispatcher is not admitting ordinary input",
            Self::QueueSaturated { .. } => "dispatcher queue saturated; release was requested",
            Self::PositionExhausted => "dispatcher position space exhausted",
            Self::WorkerStopped => "dispatcher worker is stopped",
        })
    }
}

impl std::error::Error for AdmissionError {}

/// Completion-wait failure that contains no input payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TicketError {
    PositionExhausted,
    TimedOut,
    WorkerStopped,
}

impl fmt::Display for TicketError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::PositionExhausted => "dispatcher position space exhausted",
            Self::TimedOut => "dispatcher completion wait timed out",
            Self::WorkerStopped => "dispatcher worker stopped before completion",
        })
    }
}

impl std::error::Error for TicketError {}

/// Receipt proving an ordinary action was admitted.
pub struct DispatchReceipt {
    position: DispatchPosition,
    receiver: mpsc::Receiver<DispatchOutcome>,
    observed: Option<DispatchOutcome>,
}

impl DispatchReceipt {
    /// Returns the assigned position without waiting.
    #[must_use]
    pub const fn position(&self) -> DispatchPosition {
        self.position
    }

    /// Inspects completion without waiting for native execution.
    ///
    /// Pending inspection does not consume or cancel the receipt. Once observed,
    /// the same outcome remains available to repeated inspection or a final wait.
    /// This does not grant input authority or change dispatcher/backend state.
    ///
    /// # Errors
    /// Returns [`TicketError::WorkerStopped`] if the producer disappears without
    /// publishing an outcome; worker loss must not be mistaken for pending work.
    pub fn try_outcome(&mut self) -> Result<Option<DispatchOutcome>, TicketError> {
        if self.observed.is_none() {
            self.observed = match self.receiver.try_recv() {
                Ok(outcome) => Some(outcome),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => return Err(TicketError::WorkerStopped),
            };
        }
        Ok(self.observed)
    }

    /// Waits until execution, rejection, cancellation, or worker loss.
    ///
    /// # Errors
    ///
    /// Returns [`TicketError::WorkerStopped`] if the worker disappears before
    /// publishing an outcome.
    pub fn wait(self) -> Result<DispatchOutcome, TicketError> {
        if let Some(outcome) = self.observed {
            return Ok(outcome);
        }
        self.receiver.recv().map_err(|_| TicketError::WorkerStopped)
    }

    /// Waits for at most `timeout`.
    ///
    /// # Errors
    ///
    /// Returns [`TicketError::TimedOut`] on deadline expiry or
    /// [`TicketError::WorkerStopped`] on channel disconnection.
    pub fn wait_timeout(self, timeout: Duration) -> Result<DispatchOutcome, TicketError> {
        if let Some(outcome) = self.observed {
            return Ok(outcome);
        }
        self.receiver
            .recv_timeout(timeout)
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => TicketError::TimedOut,
                mpsc::RecvTimeoutError::Disconnected => TicketError::WorkerStopped,
            })
    }
}

impl fmt::Debug for DispatchReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DispatchReceipt")
            .field("position", &self.position)
            .finish_non_exhaustive()
    }
}

/// Result of one reserved-path release operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseOutcome {
    pub position: DispatchPosition,
    pub reason: EmergencyReason,
    pub report: ReleaseReport,
}

#[derive(Debug, Default)]
struct ReleaseSlot {
    outcome: Mutex<Option<ReleaseOutcome>>,
    ready: Condvar,
}

impl ReleaseSlot {
    fn complete(&self, outcome: ReleaseOutcome) {
        *lock_recover(&self.outcome) = Some(outcome);
        self.ready.notify_all();
    }
}

/// Cloneable wait handle for one coalesced emergency release.
#[derive(Clone)]
pub struct ReleaseTicket {
    position: DispatchPosition,
    slot: Arc<ReleaseSlot>,
}

impl ReleaseTicket {
    /// Returns the release barrier's assigned position.
    #[must_use]
    pub const fn position(&self) -> DispatchPosition {
        self.position
    }

    /// Reads the shared completion slot without waiting for native cleanup.
    ///
    /// `None` is pending, not proof of neutrality or a live worker. The owner
    /// must independently enforce its deadline and inspect dispatcher health.
    /// Repeated reads and cloned tickets share the same bounded outcome slot.
    /// This uses the existing short slot mutex; it is not a lock-free promise.
    #[must_use]
    pub fn try_outcome(&self) -> Option<ReleaseOutcome> {
        lock_recover(&self.slot.outcome).clone()
    }

    /// Waits until native cleanup finishes.
    ///
    /// # Errors
    ///
    /// This API currently has no runtime error after a ticket is issued; the
    /// result type remains aligned with timed waits and future worker adapters.
    pub fn wait(self) -> Result<ReleaseOutcome, TicketError> {
        let mut outcome = lock_recover(&self.slot.outcome);
        while outcome.is_none() {
            outcome = self
                .slot
                .ready
                .wait(outcome)
                .unwrap_or_else(PoisonError::into_inner);
        }
        match outcome.clone() {
            Some(outcome) => Ok(outcome),
            None => unreachable!("release outcome checked above"),
        }
    }

    /// Waits for at most `timeout`.
    ///
    /// # Errors
    ///
    /// Returns [`TicketError::TimedOut`] if cleanup has not completed.
    pub fn wait_timeout(self, timeout: Duration) -> Result<ReleaseOutcome, TicketError> {
        let deadline = Instant::now().checked_add(timeout);
        let mut outcome = lock_recover(&self.slot.outcome);
        loop {
            if let Some(outcome) = outcome.clone() {
                return Ok(outcome);
            }
            let remaining = deadline
                .and_then(|deadline| deadline.checked_duration_since(Instant::now()))
                .ok_or(TicketError::TimedOut)?;
            let (next, timed_out) = self
                .slot
                .ready
                .wait_timeout(outcome, remaining)
                .unwrap_or_else(PoisonError::into_inner);
            outcome = next;
            if timed_out.timed_out() && outcome.is_none() {
                return Err(TicketError::TimedOut);
            }
        }
    }
}

impl fmt::Debug for ReleaseTicket {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReleaseTicket")
            .field("position", &self.position)
            .finish_non_exhaustive()
    }
}

/// Safe dispatcher metrics and state snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DispatcherSnapshot {
    pub mode: DispatcherMode,
    /// True once this dispatcher instance has suffered an unrecoverable fault.
    /// The latch remains set through cleanup and can only end in `Stopped`.
    pub terminal_fault: bool,
    pub queued_actions: usize,
    pub held_buttons: usize,
    pub held_keys: usize,
    pub last_completed_position: Option<DispatchPosition>,
    pub worker_alive: bool,
}

/// Failure to resume ordinary admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResumeError {
    NotSuspended(DispatcherMode),
    WorkerStopped,
}

impl fmt::Display for ResumeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotSuspended(_) => "dispatcher is not safely suspended",
            Self::WorkerStopped => "dispatcher worker is stopped",
        })
    }
}

impl std::error::Error for ResumeError {}

struct Completion {
    position: DispatchPosition,
    sender: mpsc::Sender<DispatchOutcome>,
}

struct QueueEntry {
    action: InputAction,
    completions: Vec<Completion>,
}

#[derive(Clone)]
struct PendingEmergency {
    position: DispatchPosition,
    reason: EmergencyReason,
    slot: Arc<ReleaseSlot>,
}

struct SharedState {
    mode: DispatcherMode,
    terminal_fault: bool,
    queue: VecDeque<QueueEntry>,
    queued_actions: usize,
    queue_capacity: usize,
    next_position: Option<u64>,
    emergency: Option<PendingEmergency>,
    active_release: Option<PendingEmergency>,
    held_buttons: usize,
    held_keys: usize,
    last_completed_position: Option<DispatchPosition>,
    worker_alive: bool,
}

struct Shared {
    state: Mutex<SharedState>,
    wake_worker: Condvar,
}

struct WorkerExit<B> {
    backend: B,
    panicked: bool,
}

/// Bounded single-threaded owner of one [`InputBackend`].
pub struct InputDispatcher<B: InputBackend + 'static> {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<WorkerExit<B>>>,
}

impl<B: InputBackend + 'static> InputDispatcher<B> {
    /// Starts the dedicated backend execution thread.
    ///
    /// # Errors
    ///
    /// Returns the operating-system thread creation error. No action has been
    /// admitted when this occurs.
    pub fn start(backend: B, config: DispatcherConfig) -> io::Result<Self> {
        let shared = Arc::new(Shared {
            state: Mutex::new(SharedState {
                mode: DispatcherMode::Running,
                terminal_fault: false,
                queue: VecDeque::with_capacity(config.queue_capacity()),
                queued_actions: 0,
                queue_capacity: config.queue_capacity(),
                next_position: Some(1),
                emergency: None,
                active_release: None,
                held_buttons: 0,
                held_keys: 0,
                last_completed_position: None,
                worker_alive: true,
            }),
            wake_worker: Condvar::new(),
        });
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("tether-input-dispatch".to_owned())
            .spawn(move || worker_entry(backend, &worker_shared))?;
        Ok(Self {
            shared,
            worker: Some(worker),
        })
    }

    /// Admits one action or fails closed when the bounded queue is unavailable.
    ///
    /// Compatible adjacent motion is coalesced only while every logical action
    /// remains bounded and receives its own monotonic completion position.
    ///
    /// # Errors
    ///
    /// Returns [`AdmissionError`] if admission is closed, the worker stopped,
    /// positions are exhausted, or saturation initiated emergency cleanup.
    pub fn submit(&self, action: InputAction) -> Result<DispatchReceipt, AdmissionError> {
        let (sender, receiver) = mpsc::channel();
        let mut state = lock_recover(&self.shared.state);
        if !state.worker_alive {
            return Err(AdmissionError::WorkerStopped);
        }
        if state.mode != DispatcherMode::Running {
            return Err(AdmissionError::NotAdmitting(state.mode));
        }
        if state.queued_actions == state.queue_capacity {
            let ticket = begin_emergency_locked(&mut state, EmergencyReason::QueueSaturated)
                .map_err(|()| AdmissionError::PositionExhausted)?;
            self.shared.wake_worker.notify_one();
            return Err(AdmissionError::QueueSaturated {
                release_position: ticket.position,
            });
        }
        let Some(position) = allocate_action_position(&mut state) else {
            let _release = begin_emergency_locked(&mut state, EmergencyReason::PositionExhausted);
            self.shared.wake_worker.notify_one();
            return Err(AdmissionError::PositionExhausted);
        };
        let completion = Completion { position, sender };
        state.queued_actions += 1;
        if let Some(tail) = state.queue.back_mut()
            && try_coalesce(&mut tail.action, &action)
        {
            tail.completions.push(completion);
        } else {
            state.queue.push_back(QueueEntry {
                action,
                completions: vec![completion],
            });
        }
        self.shared.wake_worker.notify_one();
        Ok(DispatchReceipt {
            position,
            receiver,
            observed: None,
        })
    }

    /// Stops ordinary admission and schedules priority native cleanup.
    ///
    /// Concurrent requests share one bounded mailbox slot. A higher-priority
    /// reason upgrades the pending request without enqueuing another release.
    ///
    /// # Errors
    ///
    /// Returns [`TicketError::WorkerStopped`] when cleanup cannot run or
    /// [`TicketError::PositionExhausted`] when no barrier position remains.
    pub fn request_release(&self, reason: EmergencyReason) -> Result<ReleaseTicket, TicketError> {
        let mut state = lock_recover(&self.shared.state);
        if !state.worker_alive {
            return Err(TicketError::WorkerStopped);
        }
        let ticket = begin_emergency_locked(&mut state, reason)
            .map_err(|()| TicketError::PositionExhausted)?;
        self.shared.wake_worker.notify_one();
        Ok(ticket)
    }

    /// Reopens admission only after a neutral recoverable release.
    ///
    /// # Errors
    ///
    /// Returns [`ResumeError`] for faulted, active, releasing, or stopped state.
    pub fn resume(&self) -> Result<(), ResumeError> {
        let mut state = lock_recover(&self.shared.state);
        if !state.worker_alive {
            return Err(ResumeError::WorkerStopped);
        }
        if state.mode != DispatcherMode::Suspended || state.terminal_fault {
            return Err(ResumeError::NotSuspended(state.mode));
        }
        state.mode = DispatcherMode::Running;
        Ok(())
    }

    /// Returns only bounded, non-sensitive state.
    #[must_use]
    pub fn snapshot(&self) -> DispatcherSnapshot {
        let state = lock_recover(&self.shared.state);
        DispatcherSnapshot {
            mode: state.mode,
            terminal_fault: state.terminal_fault,
            queued_actions: state.queued_actions,
            held_buttons: state.held_buttons,
            held_keys: state.held_keys,
            last_completed_position: state.last_completed_position,
            worker_alive: state.worker_alive,
        }
    }

    /// Performs priority release, joins the worker, and returns the backend for
    /// deterministic inspection or ownership transfer during shutdown.
    #[must_use]
    pub fn shutdown(mut self) -> ShutdownResult<B> {
        let release = self
            .request_release(EmergencyReason::Shutdown)
            .and_then(ReleaseTicket::wait);
        let exit = self.take_worker_exit();
        ShutdownResult {
            backend: exit.backend,
            release,
            worker_panicked: exit.panicked,
        }
    }

    fn take_worker_exit(&mut self) -> WorkerExit<B> {
        self.worker
            .take()
            .expect("dispatcher worker exists until shutdown")
            .join()
            .unwrap_or_else(|_| panic!("dispatcher outer panic containment failed"))
    }
}

impl<B: InputBackend + 'static> Drop for InputDispatcher<B> {
    fn drop(&mut self) {
        if self.worker.is_none() {
            return;
        }
        if let Ok(ticket) = self.request_release(EmergencyReason::Shutdown) {
            let _release_result = ticket.wait();
        }
        let _worker_exit = self.take_worker_exit();
    }
}

/// Owned result of orderly dispatcher shutdown.
pub struct ShutdownResult<B> {
    pub backend: B,
    pub release: Result<ReleaseOutcome, TicketError>,
    pub worker_panicked: bool,
}

fn lock_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn allocate_action_position(state: &mut SharedState) -> Option<DispatchPosition> {
    let current = state.next_position?;
    if current == u64::MAX {
        return None;
    }
    state.next_position = current.checked_add(1);
    Some(DispatchPosition(current))
}

fn allocate_release_position(state: &mut SharedState) -> Option<DispatchPosition> {
    let current = state.next_position?;
    state.next_position = current.checked_add(1);
    Some(DispatchPosition(current))
}

fn begin_emergency_locked(
    state: &mut SharedState,
    reason: EmergencyReason,
) -> Result<ReleaseTicket, ()> {
    state.terminal_fault |= reason.establishes_terminal_fault();

    if let Some(pending) = &mut state.emergency {
        if reason.priority() > pending.reason.priority() {
            pending.reason = reason;
        }
        return Ok(ReleaseTicket {
            position: pending.position,
            slot: Arc::clone(&pending.slot),
        });
    }

    let position = allocate_release_position(state).ok_or(())?;
    let slot = Arc::new(ReleaseSlot::default());
    let pending = PendingEmergency {
        position,
        reason,
        slot: Arc::clone(&slot),
    };
    state.mode = DispatcherMode::Releasing;
    cancel_queue_locked(state, CancellationReason::Emergency(reason));
    state.emergency = Some(pending);
    Ok(ReleaseTicket { position, slot })
}

fn cancel_queue_locked(state: &mut SharedState, reason: CancellationReason) {
    let queue = std::mem::take(&mut state.queue);
    for entry in queue {
        for completion in entry.completions {
            mark_completed(state, completion.position);
            let _receiver_may_be_gone = completion.sender.send(DispatchOutcome::Cancelled {
                position: completion.position,
                reason,
            });
        }
    }
    state.queued_actions = 0;
}

fn mark_completed(state: &mut SharedState, position: DispatchPosition) {
    state.last_completed_position = Some(
        state
            .last_completed_position
            .map_or(position, |current| current.max(position)),
    );
}

fn try_coalesce(existing: &mut InputAction, incoming: &InputAction) -> bool {
    match (existing, incoming) {
        (InputAction::PointerMove(existing), InputAction::PointerMove(incoming)) => {
            let x = i64::from(existing.x()) + i64::from(incoming.x());
            let y = i64::from(existing.y()) + i64::from(incoming.y());
            let Ok(x) = i32::try_from(x) else {
                return false;
            };
            let Ok(y) = i32::try_from(y) else {
                return false;
            };
            let Ok(combined) = PointerDelta::new(x, y) else {
                return false;
            };
            *existing = combined;
            true
        }
        (InputAction::Scroll(existing), InputAction::Scroll(incoming))
            if existing.axis() == incoming.axis() =>
        {
            let amount = i64::from(existing.amount()) + i64::from(incoming.amount());
            let Ok(amount) = i32::try_from(amount) else {
                return false;
            };
            let Ok(combined) = ScrollDelta::new(existing.axis(), amount) else {
                return false;
            };
            *existing = combined;
            true
        }
        _ => false,
    }
}

enum Work {
    Action(QueueEntry),
    Release(PendingEmergency),
    Stop,
}

fn worker_entry<B: InputBackend + 'static>(mut backend: B, shared: &Shared) -> WorkerExit<B> {
    let run_result = panic::catch_unwind(AssertUnwindSafe(|| run_worker(&mut backend, shared)));
    let panicked = match run_result {
        Ok(panicked) => panicked,
        Err(_payload) => {
            contain_unexpected_worker_panic(&mut backend, shared);
            true
        }
    };
    WorkerExit { backend, panicked }
}

fn run_worker(backend: &mut impl InputBackend, shared: &Shared) -> bool {
    let mut ledger = HeldLedger::default();
    loop {
        let work = next_work(shared);
        match work {
            Work::Action(entry) => {
                if process_action(backend, shared, &mut ledger, entry) {
                    return true;
                }
            }
            Work::Release(pending) => {
                let reason = pending.reason;
                let release = panic::catch_unwind(AssertUnwindSafe(|| backend.release_all()));
                let (report, release_panicked) = match release {
                    Ok(report) => (report, false),
                    Err(_payload) => (uncertain_release_report(), true),
                };
                if report.is_confirmed_neutral() {
                    ledger.clear();
                }
                complete_release(shared, &ledger, &pending, report, release_panicked);
                if release_panicked || reason == EmergencyReason::WorkerPanic {
                    return true;
                }
                let state = lock_recover(&shared.state);
                if !state.worker_alive {
                    return false;
                }
            }
            Work::Stop => return false,
        }
    }
}

fn next_work(shared: &Shared) -> Work {
    let mut state = lock_recover(&shared.state);
    loop {
        if !state.worker_alive {
            return Work::Stop;
        }
        if let Some(emergency) = state.emergency.take() {
            state.active_release = Some(emergency.clone());
            return Work::Release(emergency);
        }
        if let Some(entry) = state.queue.pop_front() {
            state.queued_actions = state.queued_actions.saturating_sub(entry.completions.len());
            return Work::Action(entry);
        }
        state = shared
            .wake_worker
            .wait(state)
            .unwrap_or_else(PoisonError::into_inner);
    }
}

fn process_action(
    backend: &mut impl InputBackend,
    shared: &Shared,
    ledger: &mut HeldLedger,
    entry: QueueEntry,
) -> bool {
    if let Err(reason) = ledger.validate(&entry.action) {
        schedule_internal_release(shared, EmergencyReason::InvalidTransition);
        complete_action(shared, entry.completions, |position| {
            DispatchOutcome::Rejected { position, reason }
        });
        return false;
    }

    let operation = entry.action.class();
    let execution = panic::catch_unwind(AssertUnwindSafe(|| backend.execute(&entry.action)));
    match execution {
        Ok(Ok(())) => {
            ledger.apply(&entry.action);
            publish_ledger(shared, ledger);
            complete_action(shared, entry.completions, |position| {
                DispatchOutcome::Executed { position }
            });
            false
        }
        Ok(Err(error)) => {
            if error.requires_emergency_release() {
                schedule_internal_release(shared, EmergencyReason::BackendFault);
            }
            complete_action(shared, entry.completions, |position| {
                DispatchOutcome::BackendRejected { position, error }
            });
            false
        }
        Err(_payload) => {
            let error = BackendError::new(
                BackendErrorCategory::Fatal,
                operation,
                RetryScope::AfterSessionRestart,
            );
            schedule_internal_release(shared, EmergencyReason::WorkerPanic);
            complete_action(shared, entry.completions, |position| {
                DispatchOutcome::BackendRejected { position, error }
            });
            false
        }
    }
}

fn complete_action(
    shared: &Shared,
    completions: Vec<Completion>,
    outcome: impl Fn(DispatchPosition) -> DispatchOutcome,
) {
    let mut state = lock_recover(&shared.state);
    for completion in completions {
        mark_completed(&mut state, completion.position);
        let _receiver_may_be_gone = completion.sender.send(outcome(completion.position));
    }
}

fn schedule_internal_release(shared: &Shared, reason: EmergencyReason) {
    let mut state = lock_recover(&shared.state);
    if begin_emergency_locked(&mut state, reason).is_err() {
        state.terminal_fault = true;
        state.mode = DispatcherMode::Faulted;
        cancel_queue_locked(&mut state, CancellationReason::WorkerStopped);
        state.worker_alive = false;
    }
    shared.wake_worker.notify_one();
}

fn complete_release(
    shared: &Shared,
    ledger: &HeldLedger,
    pending: &PendingEmergency,
    report: ReleaseReport,
    release_panicked: bool,
) {
    let outcome = ReleaseOutcome {
        position: pending.position,
        reason: pending.reason,
        report,
    };
    {
        let mut state = lock_recover(&shared.state);
        mark_completed(&mut state, pending.position);
        state.active_release = None;
        state.held_buttons = ledger.buttons.len();
        state.held_keys = ledger.keys.len();
        state.terminal_fault |= release_panicked || !outcome.report.is_confirmed_neutral();
        state.mode = if pending.reason == EmergencyReason::Shutdown {
            DispatcherMode::Stopped
        } else if state.terminal_fault {
            DispatcherMode::Faulted
        } else {
            pending
                .reason
                .mode_after_release(outcome.report.is_confirmed_neutral())
        };
        if state.mode == DispatcherMode::Stopped
            || pending.reason == EmergencyReason::PositionExhausted
            || pending.reason == EmergencyReason::WorkerPanic
            || release_panicked
        {
            state.worker_alive = false;
            cancel_queue_locked(&mut state, CancellationReason::WorkerStopped);
        }
    }
    pending.slot.complete(outcome);
    shared.wake_worker.notify_all();
}

fn contain_unexpected_worker_panic(backend: &mut impl InputBackend, shared: &Shared) {
    let report = panic::catch_unwind(AssertUnwindSafe(|| backend.release_all()))
        .unwrap_or_else(|_| uncertain_release_report());
    let pending = {
        let mut state = lock_recover(&shared.state);
        state.terminal_fault = true;
        state.mode = DispatcherMode::Faulted;
        state.worker_alive = false;
        state.held_buttons = if report.is_confirmed_neutral() {
            0
        } else {
            state.held_buttons
        };
        state.held_keys = if report.is_confirmed_neutral() {
            0
        } else {
            state.held_keys
        };
        cancel_queue_locked(&mut state, CancellationReason::WorkerStopped);
        let mut pending = Vec::with_capacity(2);
        if let Some(active) = state.active_release.take() {
            mark_completed(&mut state, active.position);
            pending.push(active);
        }
        if let Some(queued) = state.emergency.take() {
            mark_completed(&mut state, queued.position);
            pending.push(queued);
        }
        pending
    };
    for pending in pending {
        pending.slot.complete(ReleaseOutcome {
            position: pending.position,
            reason: EmergencyReason::WorkerPanic,
            report: report.clone(),
        });
    }
    shared.wake_worker.notify_all();
}

fn uncertain_release_report() -> ReleaseReport {
    ReleaseReport {
        attempted: 0,
        released: 0,
        failures: vec![ReleaseFailure::BackendLocalState],
        neutral: false,
    }
}

fn publish_ledger(shared: &Shared, ledger: &HeldLedger) {
    let mut state = lock_recover(&shared.state);
    state.held_buttons = ledger.buttons.len();
    state.held_keys = ledger.keys.len();
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HeldItem {
    Button(MouseButton),
    Key(PhysicalKey),
}

#[derive(Default)]
struct HeldLedger {
    buttons: Vec<MouseButton>,
    keys: Vec<PhysicalKey>,
}

impl HeldLedger {
    fn validate(&self, action: &InputAction) -> Result<(), RejectionReason> {
        match action {
            InputAction::MouseButton(ButtonAction::Down(button)) => {
                if self.buttons.contains(button) {
                    Err(RejectionReason::DuplicateDown)
                } else {
                    Ok(())
                }
            }
            InputAction::MouseButton(ButtonAction::Up(button)) => {
                if self.buttons.contains(button) {
                    Ok(())
                } else {
                    Err(RejectionReason::UnmatchedUp)
                }
            }
            InputAction::MouseButton(ButtonAction::Click { button, .. }) => {
                if self.buttons.contains(button) {
                    Err(RejectionReason::ButtonAlreadyHeld)
                } else {
                    Ok(())
                }
            }
            InputAction::PhysicalKey(KeyAction::Down(key)) => {
                if self.keys.contains(key) {
                    Err(RejectionReason::DuplicateDown)
                } else {
                    Ok(())
                }
            }
            InputAction::PhysicalKey(KeyAction::Up(key)) => {
                if self.keys.contains(key) {
                    Ok(())
                } else {
                    Err(RejectionReason::UnmatchedUp)
                }
            }
            _ => Ok(()),
        }
    }

    fn apply(&mut self, action: &InputAction) {
        match action {
            InputAction::MouseButton(ButtonAction::Down(button)) => self.buttons.push(*button),
            InputAction::MouseButton(ButtonAction::Up(button)) => {
                self.buttons.retain(|held| held != button);
            }
            InputAction::PhysicalKey(KeyAction::Down(key)) => self.keys.push(*key),
            InputAction::PhysicalKey(KeyAction::Up(key)) => {
                self.keys.retain(|held| held != key);
            }
            _ => {}
        }
    }

    fn clear(&mut self) {
        self.buttons.clear();
        self.keys.clear();
    }

    #[cfg(test)]
    fn release_plan(&self) -> Vec<HeldItem> {
        self.buttons
            .iter()
            .rev()
            .copied()
            .map(HeldItem::Button)
            .chain(
                self.keys
                    .iter()
                    .rev()
                    .filter(|key| !key.is_modifier())
                    .copied()
                    .map(HeldItem::Key),
            )
            .chain(
                self.keys
                    .iter()
                    .rev()
                    .filter(|key| key.is_modifier())
                    .copied()
                    .map(HeldItem::Key),
            )
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BackendKind, BackendStatus, MAX_ABSOLUTE_FIXED_DELTA, MockInputBackend, MockOutcome,
        OperationClass, ScrollAxis, StatusRefreshReason,
    };
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    const TEST_TIMEOUT: Duration = Duration::from_secs(2);

    fn key(usage: u16) -> PhysicalKey {
        PhysicalKey::new(usage).expect("test HID usage is valid")
    }

    fn pointer(x: i32, y: i32) -> InputAction {
        InputAction::PointerMove(PointerDelta::new(x, y).expect("test delta is bounded"))
    }

    fn scroll(axis: ScrollAxis, amount: i32) -> InputAction {
        InputAction::Scroll(ScrollDelta::new(axis, amount).expect("test delta is bounded"))
    }

    fn wait(receipt: DispatchReceipt) -> DispatchOutcome {
        receipt
            .wait_timeout(TEST_TIMEOUT)
            .expect("dispatcher completes test action")
    }

    fn wait_until_worker_stops<B: InputBackend + 'static>(dispatcher: &InputDispatcher<B>) {
        let deadline = Instant::now() + TEST_TIMEOUT;
        while dispatcher.snapshot().worker_alive && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        assert!(!dispatcher.snapshot().worker_alive, "worker did not stop");
    }

    struct BlockingBackend {
        mock: MockInputBackend,
        entered: Arc<(Mutex<bool>, Condvar)>,
        released: Arc<(Mutex<bool>, Condvar)>,
        block_once: bool,
    }

    impl BlockingBackend {
        fn new() -> (Self, Gate) {
            let entered = Arc::new((Mutex::new(false), Condvar::new()));
            let released = Arc::new((Mutex::new(false), Condvar::new()));
            (
                Self {
                    mock: MockInputBackend::new(),
                    entered: Arc::clone(&entered),
                    released: Arc::clone(&released),
                    block_once: true,
                },
                Gate { entered, released },
            )
        }
    }

    impl InputBackend for BlockingBackend {
        fn status(&self) -> BackendStatus {
            self.mock.status()
        }

        fn refresh_status(&mut self, reason: StatusRefreshReason) -> BackendStatus {
            self.mock.refresh_status(reason)
        }

        fn execute(&mut self, action: &InputAction) -> Result<(), BackendError> {
            if self.block_once {
                self.block_once = false;
                let (entered_lock, entered_ready) = &*self.entered;
                *lock_recover(entered_lock) = true;
                entered_ready.notify_all();
                let (released_lock, released_ready) = &*self.released;
                let mut released = lock_recover(released_lock);
                while !*released {
                    released = released_ready
                        .wait(released)
                        .unwrap_or_else(PoisonError::into_inner);
                }
            }
            self.mock.execute(action)
        }

        fn release_all(&mut self) -> ReleaseReport {
            self.mock.release_all()
        }
    }

    struct Gate {
        entered: Arc<(Mutex<bool>, Condvar)>,
        released: Arc<(Mutex<bool>, Condvar)>,
    }

    struct BlockingReleaseBackend {
        mock: MockInputBackend,
        entered: Arc<(Mutex<bool>, Condvar)>,
        released: Arc<(Mutex<bool>, Condvar)>,
        block_once: bool,
    }

    impl BlockingReleaseBackend {
        fn new() -> (Self, Gate) {
            let entered = Arc::new((Mutex::new(false), Condvar::new()));
            let released = Arc::new((Mutex::new(false), Condvar::new()));
            (
                Self {
                    mock: MockInputBackend::new(),
                    entered: Arc::clone(&entered),
                    released: Arc::clone(&released),
                    block_once: true,
                },
                Gate { entered, released },
            )
        }
    }

    impl InputBackend for BlockingReleaseBackend {
        fn status(&self) -> BackendStatus {
            self.mock.status()
        }

        fn refresh_status(&mut self, reason: StatusRefreshReason) -> BackendStatus {
            self.mock.refresh_status(reason)
        }

        fn execute(&mut self, action: &InputAction) -> Result<(), BackendError> {
            self.mock.execute(action)
        }

        fn release_all(&mut self) -> ReleaseReport {
            if self.block_once {
                self.block_once = false;
                let (entered_lock, entered_ready) = &*self.entered;
                *lock_recover(entered_lock) = true;
                entered_ready.notify_all();
                let (released_lock, released_ready) = &*self.released;
                let mut released = lock_recover(released_lock);
                while !*released {
                    released = released_ready
                        .wait(released)
                        .unwrap_or_else(PoisonError::into_inner);
                }
            }
            self.mock.release_all()
        }
    }

    impl Gate {
        fn wait_until_entered(&self) {
            let (lock, ready) = &*self.entered;
            let entered = lock_recover(lock);
            let (entered, timeout) = ready
                .wait_timeout_while(entered, TEST_TIMEOUT, |entered| !*entered)
                .unwrap_or_else(PoisonError::into_inner);
            assert!(*entered && !timeout.timed_out(), "backend did not block");
        }

        fn release(&self) {
            let (lock, ready) = &*self.released;
            *lock_recover(lock) = true;
            ready.notify_all();
        }
    }

    // Declare after the dispatcher in a blocked-worker test so an assertion
    // failure opens the gate before dispatcher Drop joins that worker.
    struct UnblockOnDrop<'a>(&'a Gate);

    impl Drop for UnblockOnDrop<'_> {
        fn drop(&mut self) {
            self.0.release();
        }
    }

    #[test]
    fn nonwaiting_receipt_inspection_preserves_pending_and_completed_outcomes() {
        let position = DispatchPosition(7);
        for expected in [
            DispatchOutcome::Executed { position },
            DispatchOutcome::BackendRejected {
                position,
                error: BackendError::new(
                    BackendErrorCategory::NativeCallFailed,
                    OperationClass::PointerMove,
                    RetryScope::AfterStatusRefresh,
                ),
            },
            DispatchOutcome::Rejected {
                position,
                reason: RejectionReason::DuplicateDown,
            },
            DispatchOutcome::Cancelled {
                position,
                reason: CancellationReason::Emergency(EmergencyReason::ConnectionLost),
            },
        ] {
            for timed_wait in [false, true] {
                let (sender, receiver) = mpsc::channel();
                let mut receipt = DispatchReceipt {
                    position,
                    receiver,
                    observed: None,
                };
                for _ in 0..16 {
                    assert_eq!(receipt.try_outcome(), Ok(None));
                }
                sender.send(expected).unwrap();
                drop(sender);
                assert_eq!(receipt.try_outcome(), Ok(Some(expected)));
                assert_eq!(receipt.try_outcome(), Ok(Some(expected)));
                assert_eq!(receipt.position(), position);
                let completed = if timed_wait {
                    receipt.wait_timeout(Duration::ZERO)
                } else {
                    receipt.wait()
                };
                assert_eq!(completed, Ok(expected));
            }
        }
    }

    #[test]
    fn nonwaiting_receipt_reports_producer_loss_instead_of_pending_forever() {
        let (sender, receiver) = mpsc::channel();
        let mut receipt = DispatchReceipt {
            position: DispatchPosition(1),
            receiver,
            observed: None,
        };
        assert_eq!(receipt.try_outcome(), Ok(None));
        drop(sender);
        assert_eq!(receipt.try_outcome(), Err(TicketError::WorkerStopped));
        assert_eq!(receipt.try_outcome(), Err(TicketError::WorkerStopped));
    }

    #[test]
    fn nonwaiting_release_inspection_preserves_reserved_cleanup_and_cancellation() {
        let (backend, gate) = BlockingBackend::new();
        let dispatcher =
            InputDispatcher::start(backend, DispatcherConfig::new(2).unwrap()).unwrap();
        let _unblock = UnblockOnDrop(&gate);
        let running = dispatcher.submit(pointer(1, 0)).unwrap();
        gate.wait_until_entered();
        let mut queued = dispatcher
            .submit(InputAction::PhysicalKey(KeyAction::Down(key(4))))
            .unwrap();
        let release = dispatcher
            .request_release(EmergencyReason::ConnectionLost)
            .unwrap();
        let repeated = dispatcher
            .request_release(EmergencyReason::ConnectionLost)
            .unwrap();
        assert_eq!(release.position(), repeated.position());
        assert!(matches!(
            queued.try_outcome(),
            Ok(Some(DispatchOutcome::Cancelled { .. }))
        ));
        for _ in 0..16 {
            assert_eq!(release.try_outcome(), None);
        }
        gate.release();
        assert!(matches!(wait(running), DispatchOutcome::Executed { .. }));
        let outcome = release.clone().wait_timeout(TEST_TIMEOUT).unwrap();
        assert!(outcome.report.neutral);
        assert_eq!(release.try_outcome(), Some(outcome.clone()));
        assert_eq!(repeated.try_outcome(), Some(outcome));
        let snapshot = dispatcher.snapshot();
        assert_eq!(snapshot.mode, DispatcherMode::Suspended);
        assert_eq!((snapshot.held_keys, snapshot.held_buttons), (0, 0));
        let stopped = dispatcher.shutdown();
        assert_eq!(stopped.backend.mock.held_counts(), (0, 0));
    }

    #[test]
    fn config_enforces_nonzero_reviewed_bounds() {
        assert_eq!(DispatcherConfig::new(0), Err(DispatcherConfigError));
        assert_eq!(
            DispatcherConfig::new(MAX_QUEUE_CAPACITY + 1),
            Err(DispatcherConfigError)
        );
        assert_eq!(DispatcherConfig::new(3).expect("valid").queue_capacity(), 3);
    }

    #[test]
    fn ordinary_position_exhaustion_reserves_final_barrier_for_release() {
        let dispatcher =
            InputDispatcher::start(MockInputBackend::new(), DispatcherConfig::default())
                .expect("worker starts");
        lock_recover(&dispatcher.shared.state).next_position = Some(u64::MAX);
        assert!(matches!(
            dispatcher.submit(pointer(1, 1)),
            Err(AdmissionError::PositionExhausted)
        ));
        wait_until_worker_stops(&dispatcher);
        let shutdown = dispatcher.shutdown();
        assert_eq!(shutdown.backend.release_calls(), 1);
        assert_eq!(shutdown.release, Err(TicketError::WorkerStopped));
    }

    #[test]
    fn positions_and_execution_are_deterministic_across_release_barrier() {
        let dispatcher =
            InputDispatcher::start(MockInputBackend::new(), DispatcherConfig::default())
                .expect("worker starts");
        let first = dispatcher.submit(pointer(1, 2)).expect("admitted");
        let second = dispatcher
            .submit(scroll(ScrollAxis::Vertical, 3))
            .expect("admitted");
        assert_eq!(first.position().get(), 1);
        assert_eq!(second.position().get(), 2);
        assert!(matches!(wait(first), DispatchOutcome::Executed { .. }));
        assert!(matches!(wait(second), DispatchOutcome::Executed { .. }));
        let release = dispatcher
            .request_release(EmergencyReason::ConnectionLost)
            .expect("release accepted");
        assert_eq!(release.position().get(), 3);
        assert!(
            release
                .wait_timeout(TEST_TIMEOUT)
                .expect("released")
                .report
                .neutral
        );
        assert_eq!(dispatcher.snapshot().mode, DispatcherMode::Suspended);
        let shutdown = dispatcher.shutdown();
        assert!(!shutdown.worker_panicked);
        assert_eq!(shutdown.backend.release_calls(), 2);
    }

    #[test]
    fn compatible_motion_coalesces_but_state_change_is_a_barrier() {
        let (backend, gate) = BlockingBackend::new();
        let dispatcher =
            InputDispatcher::start(backend, DispatcherConfig::default()).expect("worker starts");
        let blocker = dispatcher
            .submit(InputAction::LogicalCommand(crate::LogicalCommand::Copy))
            .expect("admitted");
        gate.wait_until_entered();
        let first = dispatcher.submit(pointer(10, 20)).expect("admitted");
        let second = dispatcher.submit(pointer(30, 40)).expect("admitted");
        let barrier = dispatcher
            .submit(InputAction::MouseButton(
                ButtonAction::click(MouseButton::Left, 1).expect("click"),
            ))
            .expect("admitted");
        let after = dispatcher.submit(pointer(50, 60)).expect("admitted");
        gate.release();
        for receipt in [blocker, first, second, barrier, after] {
            assert!(matches!(wait(receipt), DispatchOutcome::Executed { .. }));
        }
        let shutdown = dispatcher.shutdown();
        let classes = shutdown
            .backend
            .mock
            .calls()
            .iter()
            .map(|call| call.operation)
            .collect::<Vec<_>>();
        assert_eq!(
            classes,
            vec![
                OperationClass::LogicalCommand,
                OperationClass::PointerMove,
                OperationClass::MouseButton,
                OperationClass::PointerMove,
            ]
        );
    }

    #[test]
    fn coalescing_flushes_before_fixed_point_bound_instead_of_overflowing() {
        let (backend, gate) = BlockingBackend::new();
        let dispatcher =
            InputDispatcher::start(backend, DispatcherConfig::default()).expect("worker starts");
        let blocker = dispatcher
            .submit(InputAction::LogicalCommand(crate::LogicalCommand::Copy))
            .expect("admitted");
        gate.wait_until_entered();
        let first = dispatcher.submit(pointer(5_000_000, 0)).expect("admitted");
        let second = dispatcher.submit(pointer(5_000_000, 0)).expect("admitted");
        gate.release();
        for receipt in [blocker, first, second] {
            assert!(matches!(wait(receipt), DispatchOutcome::Executed { .. }));
        }
        let shutdown = dispatcher.shutdown();
        let pointer_calls = shutdown
            .backend
            .mock
            .calls()
            .iter()
            .filter(|call| call.operation == OperationClass::PointerMove)
            .count();
        assert_eq!(pointer_calls, 2);
    }

    #[test]
    fn saturation_cancels_backlog_and_releases_through_reserved_path() {
        let (backend, gate) = BlockingBackend::new();
        let dispatcher =
            InputDispatcher::start(backend, DispatcherConfig::new(2).expect("valid capacity"))
                .expect("worker starts");
        let blocker = dispatcher
            .submit(InputAction::LogicalCommand(crate::LogicalCommand::Copy))
            .expect("admitted");
        gate.wait_until_entered();
        let queued_one = dispatcher.submit(pointer(1, 0)).expect("admitted");
        let queued_two = dispatcher.submit(pointer(2, 0)).expect("admitted");
        let error = dispatcher
            .submit(pointer(3, 0))
            .expect_err("third waiting action saturates");
        assert!(matches!(error, AdmissionError::QueueSaturated { .. }));
        let release = dispatcher
            .request_release(EmergencyReason::QueueSaturated)
            .expect("coalesced release ticket");
        gate.release();
        assert!(matches!(wait(blocker), DispatchOutcome::Executed { .. }));
        for receipt in [queued_one, queued_two] {
            assert!(matches!(
                wait(receipt),
                DispatchOutcome::Cancelled {
                    reason: CancellationReason::Emergency(EmergencyReason::QueueSaturated),
                    ..
                }
            ));
        }
        let outcome = release
            .wait_timeout(TEST_TIMEOUT)
            .expect("release completes");
        assert_eq!(outcome.reason, EmergencyReason::QueueSaturated);
        assert_eq!(dispatcher.snapshot().mode, DispatcherMode::Faulted);
        let shutdown = dispatcher.shutdown();
        assert_eq!(shutdown.backend.mock.release_calls(), 2);
    }

    #[test]
    fn concurrent_release_requests_share_one_reserved_mailbox_slot() {
        let (backend, gate) = BlockingBackend::new();
        let dispatcher =
            InputDispatcher::start(backend, DispatcherConfig::default()).expect("worker starts");
        let blocker = dispatcher
            .submit(InputAction::LogicalCommand(crate::LogicalCommand::Copy))
            .expect("admitted");
        gate.wait_until_entered();
        let first = dispatcher
            .request_release(EmergencyReason::ExplicitStop)
            .expect("first release accepted");
        let second = dispatcher
            .request_release(EmergencyReason::ConnectionLost)
            .expect("second request coalesced");
        assert_eq!(first.position(), second.position());
        gate.release();
        assert!(matches!(wait(blocker), DispatchOutcome::Executed { .. }));
        let first_outcome = first.wait_timeout(TEST_TIMEOUT).expect("first waiter");
        let second_outcome = second.wait_timeout(TEST_TIMEOUT).expect("second waiter");
        assert_eq!(first_outcome, second_outcome);
        assert_eq!(first_outcome.reason, EmergencyReason::ConnectionLost);
        let shutdown = dispatcher.shutdown();
        assert_eq!(shutdown.backend.mock.release_calls(), 2);
    }

    #[test]
    fn dropping_receipt_does_not_cancel_accepted_execution() {
        let dispatcher =
            InputDispatcher::start(MockInputBackend::new(), DispatcherConfig::default())
                .expect("worker starts");
        drop(
            dispatcher
                .submit(InputAction::LogicalCommand(crate::LogicalCommand::Copy))
                .expect("admitted"),
        );
        let observed = dispatcher.submit(pointer(2, 2)).expect("admitted");
        assert!(matches!(wait(observed), DispatchOutcome::Executed { .. }));
        let shutdown = dispatcher.shutdown();
        assert_eq!(shutdown.backend.calls().len(), 2);
        assert!(
            shutdown
                .backend
                .calls()
                .iter()
                .all(|call| call.outcome == MockOutcome::Succeeded)
        );
    }

    #[test]
    fn invalid_transition_fails_closed_and_releases_held_state() {
        let dispatcher =
            InputDispatcher::start(MockInputBackend::new(), DispatcherConfig::default())
                .expect("worker starts");
        let down = dispatcher
            .submit(InputAction::PhysicalKey(KeyAction::Down(key(0x04))))
            .expect("admitted");
        assert!(matches!(wait(down), DispatchOutcome::Executed { .. }));
        let duplicate = dispatcher
            .submit(InputAction::PhysicalKey(KeyAction::Down(key(0x04))))
            .expect("admitted");
        assert!(matches!(
            wait(duplicate),
            DispatchOutcome::Rejected {
                reason: RejectionReason::DuplicateDown,
                ..
            }
        ));
        let release = dispatcher
            .request_release(EmergencyReason::InvalidTransition)
            .expect("release ticket");
        release
            .wait_timeout(TEST_TIMEOUT)
            .expect("release completes");
        assert_eq!(dispatcher.snapshot().held_keys, 0);
        let shutdown = dispatcher.shutdown();
        assert!(shutdown.backend.is_neutral());
        assert!(shutdown.backend.release_calls() >= 2);
    }

    #[test]
    fn terminal_fault_survives_ordinary_and_repeated_releases() {
        let dispatcher =
            InputDispatcher::start(MockInputBackend::new(), DispatcherConfig::default())
                .expect("worker starts");
        let down = dispatcher
            .submit(InputAction::PhysicalKey(KeyAction::Down(key(0x04))))
            .expect("admitted");
        assert!(matches!(wait(down), DispatchOutcome::Executed { .. }));
        let duplicate = dispatcher
            .submit(InputAction::PhysicalKey(KeyAction::Down(key(0x04))))
            .expect("admitted");
        assert!(matches!(
            wait(duplicate),
            DispatchOutcome::Rejected {
                reason: RejectionReason::DuplicateDown,
                ..
            }
        ));
        dispatcher
            .request_release(EmergencyReason::InvalidTransition)
            .expect("fault release remains observable")
            .wait_timeout(TEST_TIMEOUT)
            .expect("fault release completes");
        assert_eq!(dispatcher.snapshot().mode, DispatcherMode::Faulted);
        assert!(dispatcher.snapshot().terminal_fault);

        for reason in [
            EmergencyReason::ExplicitStop,
            EmergencyReason::ConnectionLost,
            EmergencyReason::Lifecycle,
        ] {
            dispatcher
                .request_release(reason)
                .expect("repeat cleanup remains available")
                .wait_timeout(TEST_TIMEOUT)
                .expect("repeat cleanup completes");
            let snapshot = dispatcher.snapshot();
            assert_eq!(snapshot.mode, DispatcherMode::Faulted);
            assert!(snapshot.terminal_fault);
        }

        assert_eq!(
            dispatcher.resume(),
            Err(ResumeError::NotSuspended(DispatcherMode::Faulted))
        );
        assert!(matches!(
            dispatcher.submit(pointer(1, 1)),
            Err(AdmissionError::NotAdmitting(DispatcherMode::Faulted))
        ));

        let shutdown = dispatcher.shutdown();
        assert_eq!(
            shutdown.release.expect("shutdown cleanup").reason,
            EmergencyReason::Shutdown
        );
        assert!(shutdown.backend.is_neutral());
    }

    #[test]
    fn terminal_fault_latch_survives_release_race() {
        let (backend, gate) = BlockingReleaseBackend::new();
        let dispatcher =
            InputDispatcher::start(backend, DispatcherConfig::default()).expect("worker starts");
        let terminal = dispatcher
            .request_release(EmergencyReason::QueueSaturated)
            .expect("terminal release accepted");
        gate.wait_until_entered();
        let during_release = dispatcher.snapshot();
        assert_eq!(during_release.mode, DispatcherMode::Releasing);
        assert!(during_release.terminal_fault);

        let ordinary = dispatcher
            .request_release(EmergencyReason::ExplicitStop)
            .expect("ordinary cleanup races through bounded mailbox");
        gate.release();
        assert_eq!(
            terminal
                .wait_timeout(TEST_TIMEOUT)
                .expect("terminal cleanup")
                .reason,
            EmergencyReason::QueueSaturated
        );
        assert_eq!(
            ordinary
                .wait_timeout(TEST_TIMEOUT)
                .expect("ordinary cleanup")
                .reason,
            EmergencyReason::ExplicitStop
        );
        let snapshot = dispatcher.snapshot();
        assert_eq!(snapshot.mode, DispatcherMode::Faulted);
        assert!(snapshot.terminal_fault);
        let shutdown = dispatcher.shutdown();
        assert_eq!(shutdown.backend.mock.release_calls(), 3);
    }

    #[test]
    fn shutdown_from_faulted_still_stops_and_joins_worker() {
        let dispatcher =
            InputDispatcher::start(MockInputBackend::new(), DispatcherConfig::default())
                .expect("worker starts");
        dispatcher
            .request_release(EmergencyReason::BackendFault)
            .expect("fault release accepted")
            .wait_timeout(TEST_TIMEOUT)
            .expect("fault release completes");
        assert_eq!(dispatcher.snapshot().mode, DispatcherMode::Faulted);
        assert!(dispatcher.snapshot().terminal_fault);

        let shutdown = dispatcher.shutdown();
        assert!(!shutdown.worker_panicked);
        assert_eq!(
            shutdown.release.expect("shutdown cleanup").reason,
            EmergencyReason::Shutdown
        );
        assert_eq!(shutdown.backend.release_calls(), 2);
    }

    #[test]
    fn click_on_held_button_is_rejected_without_mutating_ledger() {
        let mut ledger = HeldLedger::default();
        ledger.apply(&InputAction::MouseButton(ButtonAction::Down(
            MouseButton::Left,
        )));
        let before = ledger.release_plan();
        let click = InputAction::MouseButton(
            ButtonAction::click(MouseButton::Left, 1).expect("valid click"),
        );
        assert_eq!(
            ledger.validate(&click),
            Err(RejectionReason::ButtonAlreadyHeld)
        );
        assert_eq!(ledger.release_plan(), before);

        let dispatcher =
            InputDispatcher::start(MockInputBackend::new(), DispatcherConfig::default())
                .expect("worker starts");
        assert!(matches!(
            wait(
                dispatcher
                    .submit(InputAction::MouseButton(ButtonAction::Down(
                        MouseButton::Left,
                    )))
                    .expect("down admitted")
            ),
            DispatchOutcome::Executed { .. }
        ));
        assert!(matches!(
            wait(
                dispatcher
                    .submit(click)
                    .expect("click admitted for validation")
            ),
            DispatchOutcome::Rejected {
                reason: RejectionReason::ButtonAlreadyHeld,
                ..
            }
        ));
        dispatcher
            .request_release(EmergencyReason::InvalidTransition)
            .expect("fault cleanup remains observable")
            .wait_timeout(TEST_TIMEOUT)
            .expect("fault cleanup completes");
        let shutdown = dispatcher.shutdown();
        assert_eq!(shutdown.backend.calls().len(), 1);
        assert!(shutdown.backend.is_neutral());
    }

    #[test]
    fn click_on_unheld_button_and_ordinary_drag_remain_valid() {
        let dispatcher =
            InputDispatcher::start(MockInputBackend::new(), DispatcherConfig::default())
                .expect("worker starts");
        let actions = [
            InputAction::MouseButton(
                ButtonAction::click(MouseButton::Right, 1).expect("valid click"),
            ),
            InputAction::MouseButton(ButtonAction::Down(MouseButton::Left)),
            pointer(8, -3),
            InputAction::MouseButton(ButtonAction::Up(MouseButton::Left)),
        ];
        for action in actions {
            assert!(matches!(
                wait(dispatcher.submit(action).expect("valid action admitted")),
                DispatchOutcome::Executed { .. }
            ));
        }
        let snapshot = dispatcher.snapshot();
        assert_eq!(snapshot.mode, DispatcherMode::Running);
        assert!(!snapshot.terminal_fault);
        assert_eq!(snapshot.held_buttons, 0);
        let shutdown = dispatcher.shutdown();
        assert_eq!(shutdown.backend.calls().len(), 4);
        assert!(shutdown.backend.is_neutral());
    }

    #[test]
    fn fatal_backend_error_stops_admission_and_neutralizes() {
        let mut backend = MockInputBackend::new();
        backend.push_execute_result(Err(BackendError::new(
            BackendErrorCategory::PartialStateChange,
            OperationClass::MouseButton,
            RetryScope::AfterSessionRestart,
        )));
        let dispatcher =
            InputDispatcher::start(backend, DispatcherConfig::default()).expect("worker starts");
        let receipt = dispatcher
            .submit(InputAction::MouseButton(ButtonAction::Down(
                MouseButton::Left,
            )))
            .expect("admitted");
        assert!(matches!(
            wait(receipt),
            DispatchOutcome::BackendRejected { .. }
        ));
        let release = dispatcher
            .request_release(EmergencyReason::BackendFault)
            .expect("fault release ticket");
        release
            .wait_timeout(TEST_TIMEOUT)
            .expect("release completes");
        assert_eq!(dispatcher.snapshot().mode, DispatcherMode::Faulted);
        let shutdown = dispatcher.shutdown();
        assert!(shutdown.backend.is_neutral());
    }

    #[test]
    fn failed_release_retains_dispatcher_ledger_and_faults() {
        let mut backend = MockInputBackend::new();
        backend.push_release_report(ReleaseReport {
            attempted: 1,
            released: 0,
            failures: vec![ReleaseFailure::NonModifierKey],
            neutral: false,
        });
        let dispatcher =
            InputDispatcher::start(backend, DispatcherConfig::default()).expect("worker starts");
        let down = dispatcher
            .submit(InputAction::PhysicalKey(KeyAction::Down(key(0x04))))
            .expect("admitted");
        assert!(matches!(wait(down), DispatchOutcome::Executed { .. }));
        let release = dispatcher
            .request_release(EmergencyReason::ConnectionLost)
            .expect("release accepted");
        assert!(
            !release
                .wait_timeout(TEST_TIMEOUT)
                .expect("release")
                .report
                .neutral
        );
        assert_eq!(dispatcher.snapshot().held_keys, 1);
        assert_eq!(dispatcher.snapshot().mode, DispatcherMode::Faulted);
        let shutdown = dispatcher.shutdown();
        assert!(shutdown.release.expect("shutdown release").report.neutral);
    }

    #[test]
    fn inconsistent_neutral_report_fails_closed() {
        let mut backend = MockInputBackend::new();
        backend.push_release_report(ReleaseReport {
            attempted: 1,
            released: 0,
            failures: Vec::new(),
            neutral: true,
        });
        let dispatcher =
            InputDispatcher::start(backend, DispatcherConfig::default()).expect("worker starts");
        assert!(matches!(
            wait(
                dispatcher
                    .submit(InputAction::PhysicalKey(KeyAction::Down(key(0x04))))
                    .expect("admitted")
            ),
            DispatchOutcome::Executed { .. }
        ));
        let release = dispatcher
            .request_release(EmergencyReason::ConnectionLost)
            .expect("release accepted")
            .wait_timeout(TEST_TIMEOUT)
            .expect("release completes");
        assert!(release.report.neutral);
        assert!(!release.report.is_confirmed_neutral());
        assert_eq!(dispatcher.snapshot().mode, DispatcherMode::Faulted);
        assert_eq!(dispatcher.snapshot().held_keys, 1);
        let _shutdown = dispatcher.shutdown();
    }

    #[test]
    fn recoverable_release_requires_explicit_resume() {
        let dispatcher =
            InputDispatcher::start(MockInputBackend::new(), DispatcherConfig::default())
                .expect("worker starts");
        dispatcher
            .request_release(EmergencyReason::OwnershipTransfer)
            .expect("release accepted")
            .wait_timeout(TEST_TIMEOUT)
            .expect("release completes");
        assert!(matches!(
            dispatcher.submit(pointer(1, 1)),
            Err(AdmissionError::NotAdmitting(DispatcherMode::Suspended))
        ));
        dispatcher
            .resume()
            .expect("neutral suspended state resumes");
        assert!(matches!(
            wait(dispatcher.submit(pointer(1, 1)).expect("admitted")),
            DispatchOutcome::Executed { .. }
        ));
        let _shutdown = dispatcher.shutdown();
    }

    #[test]
    fn held_ledger_preserves_press_order_for_release_plan() {
        let modifier_one = key(0xE1);
        let normal_one = key(0x04);
        let modifier_two = key(0xE0);
        let normal_two = key(0x05);
        let mut ledger = HeldLedger::default();
        for action in [
            InputAction::PhysicalKey(KeyAction::Down(modifier_one)),
            InputAction::MouseButton(ButtonAction::Down(MouseButton::Left)),
            InputAction::PhysicalKey(KeyAction::Down(normal_one)),
            InputAction::PhysicalKey(KeyAction::Down(modifier_two)),
            InputAction::MouseButton(ButtonAction::Down(MouseButton::Right)),
            InputAction::PhysicalKey(KeyAction::Down(normal_two)),
        ] {
            ledger.apply(&action);
        }
        assert_eq!(
            ledger.release_plan(),
            vec![
                HeldItem::Button(MouseButton::Right),
                HeldItem::Button(MouseButton::Left),
                HeldItem::Key(normal_two),
                HeldItem::Key(normal_one),
                HeldItem::Key(modifier_two),
                HeldItem::Key(modifier_one),
            ]
        );
    }

    #[test]
    fn held_ledger_matches_a_small_transition_model() {
        #[derive(Clone, Copy)]
        enum ModelAction {
            KeyDown,
            KeyUp,
            ButtonDown,
            ButtonUp,
        }

        let actions = [
            ModelAction::KeyDown,
            ModelAction::KeyUp,
            ModelAction::ButtonDown,
            ModelAction::ButtonUp,
        ];
        for first in actions {
            for second in actions {
                for third in actions {
                    let mut ledger = HeldLedger::default();
                    let mut key_held = false;
                    let mut button_held = false;
                    for action in [first, second, third] {
                        let (input, model_valid) = match action {
                            ModelAction::KeyDown => (
                                InputAction::PhysicalKey(KeyAction::Down(key(0x04))),
                                !key_held,
                            ),
                            ModelAction::KeyUp => {
                                (InputAction::PhysicalKey(KeyAction::Up(key(0x04))), key_held)
                            }
                            ModelAction::ButtonDown => (
                                InputAction::MouseButton(ButtonAction::Down(MouseButton::Left)),
                                !button_held,
                            ),
                            ModelAction::ButtonUp => (
                                InputAction::MouseButton(ButtonAction::Up(MouseButton::Left)),
                                button_held,
                            ),
                        };
                        assert_eq!(ledger.validate(&input).is_ok(), model_valid);
                        if model_valid {
                            ledger.apply(&input);
                            match action {
                                ModelAction::KeyDown => key_held = true,
                                ModelAction::KeyUp => key_held = false,
                                ModelAction::ButtonDown => button_held = true,
                                ModelAction::ButtonUp => button_held = false,
                            }
                        }
                        assert_eq!(ledger.keys.len(), usize::from(key_held));
                        assert_eq!(ledger.buttons.len(), usize::from(button_held));
                    }
                }
            }
        }
    }

    #[test]
    fn poisoned_state_lock_is_recovered_without_losing_worker() {
        let dispatcher =
            InputDispatcher::start(MockInputBackend::new(), DispatcherConfig::default())
                .expect("worker starts");
        let shared = Arc::clone(&dispatcher.shared);
        let _expected_panic = panic::catch_unwind(AssertUnwindSafe(|| {
            let _guard = shared.state.lock().expect("not yet poisoned");
            panic!("intentional test poison");
        }));
        assert!(dispatcher.snapshot().worker_alive);
        assert!(matches!(
            wait(dispatcher.submit(pointer(1, 1)).expect("admitted")),
            DispatchOutcome::Executed { .. }
        ));
        let _shutdown = dispatcher.shutdown();
    }

    struct PanickingBackend {
        release_calls: Arc<AtomicUsize>,
        panic_once: AtomicBool,
    }

    impl InputBackend for PanickingBackend {
        fn status(&self) -> BackendStatus {
            BackendStatus::unavailable(BackendKind::Mock)
        }

        fn refresh_status(&mut self, _reason: StatusRefreshReason) -> BackendStatus {
            self.status()
        }

        fn execute(&mut self, _action: &InputAction) -> Result<(), BackendError> {
            assert!(
                !self.panic_once.swap(false, Ordering::SeqCst),
                "injected backend panic"
            );
            Ok(())
        }

        fn release_all(&mut self) -> ReleaseReport {
            self.release_calls.fetch_add(1, Ordering::SeqCst);
            ReleaseReport::neutral()
        }
    }

    #[test]
    fn backend_panic_is_contained_and_best_effort_release_runs() {
        let releases = Arc::new(AtomicUsize::new(0));
        let backend = PanickingBackend {
            release_calls: Arc::clone(&releases),
            panic_once: AtomicBool::new(true),
        };
        let dispatcher =
            InputDispatcher::start(backend, DispatcherConfig::default()).expect("worker starts");
        let outcome = wait(dispatcher.submit(pointer(1, 1)).expect("admitted"));
        assert!(matches!(outcome, DispatchOutcome::BackendRejected { .. }));
        wait_until_worker_stops(&dispatcher);
        let shutdown = dispatcher.shutdown();
        assert!(shutdown.worker_panicked);
        assert_eq!(releases.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn same_axis_scroll_coalesces_without_cross_axis_leakage() {
        let mut vertical = scroll(ScrollAxis::Vertical, MAX_ABSOLUTE_FIXED_DELTA);
        assert!(!try_coalesce(
            &mut vertical,
            &scroll(ScrollAxis::Vertical, 1)
        ));
        assert!(!try_coalesce(
            &mut vertical,
            &scroll(ScrollAxis::Horizontal, -1)
        ));
        assert!(try_coalesce(
            &mut vertical,
            &scroll(ScrollAxis::Vertical, -1)
        ));
    }
}
