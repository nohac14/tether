use super::*;
use crate::interfaces::{
    AddressState, AssignedAddress, IdentityPersistence, Interface, InterfaceIndexes, InterfaceKind,
    InterfaceStatus, LinkState, NetworkProfile,
};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
use std::net::Ipv4Addr;
use std::num::NonZeroU16;
use std::rc::Rc;
use tether_config::{ApprovedInterfaceId, DesktopSettings};
use tether_input_core::{
    ButtonAction, DispatchOutcome, DispatcherConfig, EmergencyReason, InputAction, InputDispatcher,
    MockInputBackend, MouseButton, ReleaseTicket,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Release(ReleaseCause, Option<NonZeroU64>),
    Confirmed,
    Survey,
    Bind,
    DropListeners,
    Close(u8),
}

type Trace = Rc<RefCell<Vec<Event>>>;

struct FakeSet(Trace);

impl Drop for FakeSet {
    fn drop(&mut self) {
        self.0.borrow_mut().push(Event::DropListeners);
    }
}

#[derive(Clone, Copy)]
enum CloseResult {
    Success,
    Failure,
    Panic,
}

struct FakeShutdown {
    trace: Trace,
    id: u8,
    outcome: CloseResult,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum DriverFault {
    None,
    RefreshPending,
    Refresh,
    Bind,
    Accept,
}

struct FakeDriver {
    inventory: InterfaceInventory,
    trace: Trace,
    records: Vec<Interface>,
    pending: bool,
    fault: DriverFault,
    ready: VecDeque<(u8, CloseResult)>,
    startup_network: Option<NetworkFingerprint>,
}

impl FakeDriver {
    fn new(trace: Trace) -> Self {
        Self {
            inventory: InterfaceInventory::default(),
            trace,
            records: vec![interface("Fixture")],
            pending: false,
            fault: DriverFault::None,
            ready: VecDeque::new(),
            startup_network: None,
        }
    }
}

impl ListenerDriver for FakeDriver {
    type Set = FakeSet;
    type Transport = u8;
    type Shutdown = FakeShutdown;
    type Error = ();

    fn inventory(&self) -> &InterfaceInventory {
        &self.inventory
    }

    fn begin_refresh(&mut self, _now: Instant) -> Result<(), InterfaceError> {
        assert!(!self.pending);
        self.pending = true;
        self.trace.borrow_mut().push(Event::Survey);
        Ok(())
    }

    fn cancel_refresh(&mut self) {
        self.pending = false;
    }

    fn poll_refresh(&mut self, _now: Instant) -> Poll<Result<(), InterfaceError>> {
        assert!(self.pending);
        if self.fault == DriverFault::RefreshPending {
            return Poll::Pending;
        }
        self.pending = false;
        let records = if self.fault == DriverFault::Refresh {
            Err(InterfaceError::ProviderUnavailable)
        } else {
            Ok(self.records.clone())
        };
        if let Err(error) = self.inventory.refresh(records) {
            return Poll::Ready(Err(error));
        }
        let networks = self.startup_network.map_or_else(BTreeMap::new, |network| {
            BTreeMap::from([(self.records[0].id().clone(), network)])
        });
        self.inventory.update_startup_networks(networks);
        Poll::Ready(Ok(()))
    }

    fn bind(&mut self, plan: BindingPlan, generation: NonZeroU64) -> Result<FakeSet, ()> {
        assert!(plan.is_current(&self.inventory, generation));
        self.trace.borrow_mut().push(Event::Bind);
        if self.fault == DriverFault::Bind {
            return Err(());
        }
        Ok(FakeSet(Rc::clone(&self.trace)))
    }

    fn accept(&mut self, _set: &mut FakeSet, _generation: NonZeroU64) -> AcceptResult<Self> {
        if self.fault == DriverFault::Accept {
            return Err(());
        }
        Ok(self
            .ready
            .pop_front()
            .map(|(id, outcome)| AcceptedTransport {
                transport: id,
                shutdown: FakeShutdown {
                    trace: Rc::clone(&self.trace),
                    id,
                    outcome,
                },
            }))
    }

    fn close(shutdown: &FakeShutdown) -> Result<(), ()> {
        shutdown.trace.borrow_mut().push(Event::Close(shutdown.id));
        match shutdown.outcome {
            CloseResult::Success => Ok(()),
            CloseResult::Failure => Err(()),
            CloseResult::Panic => panic!("synthetic driver panic"),
        }
    }
}

struct FakeSafety {
    trace: Trace,
    result: Rc<Cell<Poll<Result<(), SafetyError>>>>,
    reject: bool,
}

struct FakeTicket {
    trace: Trace,
    result: Rc<Cell<Poll<Result<(), SafetyError>>>>,
}

impl SafetyCoordinator for FakeSafety {
    type Ticket = FakeTicket;

    fn request_release(
        &mut self,
        cause: ReleaseCause,
        obsolete: Option<NonZeroU64>,
    ) -> Result<FakeTicket, SafetyError> {
        self.trace
            .borrow_mut()
            .push(Event::Release(cause, obsolete));
        if self.reject {
            return Err(SafetyError::Unavailable);
        }
        Ok(FakeTicket {
            trace: Rc::clone(&self.trace),
            result: Rc::clone(&self.result),
        })
    }
}

impl NeutralityTicket for FakeTicket {
    fn poll_neutrality(&mut self) -> Poll<Result<(), SafetyError>> {
        let result = self.result.get();
        if result == Poll::Ready(Ok(())) {
            self.trace.borrow_mut().push(Event::Confirmed);
        }
        result
    }
}

type Subject = ListenerSupervisor<FakeDriver, FakeSafety>;

fn interface(name: &str) -> Interface {
    Interface::new(
        ApprovedInterfaceId::new("fixture-adapter".to_owned()).unwrap(),
        name,
        IdentityPersistence::Stable,
        InterfaceIndexes {
            ipv4: None,
            ipv6: None,
        },
        InterfaceStatus {
            kind: InterfaceKind::Ethernet,
            link: LinkState::Up,
            profile: NetworkProfile::Private,
        },
        vec![
            AssignedAddress::new(
                Ipv4Addr::new(192, 0, 2, 1).into(),
                24,
                AddressState::Preferred,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn fixture(now: Instant) -> (Subject, Trace) {
    let trace = Trace::default();
    let driver = FakeDriver::new(Rc::clone(&trace));
    let safety = FakeSafety {
        trace: Rc::clone(&trace),
        result: Rc::new(Cell::new(Poll::Pending)),
        reject: false,
    };
    let mut subject = ListenerSupervisor::new(driver, safety);
    subject.request_inventory(now).unwrap();
    subject.poll_transition(now).unwrap();
    trace.borrow_mut().clear();
    (subject, trace)
}

fn approval(subject: &ListenerSupervisor<FakeDriver, impl SafetyCoordinator>) -> ApprovedSelection {
    let snapshot = subject.inventory().current().unwrap();
    snapshot
        .approve(
            snapshot.revision(),
            &[snapshot.records()[0].id().clone()],
            NonZeroU16::new(54321).unwrap(),
        )
        .unwrap()
}

fn running(subject: &mut Subject, now: Instant) {
    subject.start(approval(subject), now).unwrap();
    subject.safety.result.set(Poll::Ready(Ok(())));
    for _ in 0..3 {
        subject.poll_transition(now).unwrap();
    }
    assert!(matches!(subject.status(), ListenerStatus::Listening { .. }));
}

fn admit(
    subject: &mut Subject,
    id: u8,
    outcome: CloseResult,
    now: Instant,
) -> ConnectionAuthority<FakeShutdown> {
    subject.driver.ready.push_back((id, outcome));
    for _ in 0..2 {
        subject.poll_transition(now).unwrap();
    }
    let (received, guard) = subject.poll_accept(now).unwrap().unwrap();
    assert_eq!(received, id);
    guard
}

#[test]
fn release_and_both_refreshes_precede_admission() {
    let now = Instant::now();
    let (mut subject, trace) = fixture(now);
    subject.start(approval(&subject), now).unwrap();
    subject.poll_transition(now).unwrap();
    assert_eq!(subject.status(), ListenerStatus::Draining);
    assert!(subject.poll_accept(now).unwrap().is_none());
    assert_eq!(
        *trace.borrow(),
        [Event::Release(ReleaseCause::StartOrRebind, None)]
    );

    subject.safety.result.set(Poll::Ready(Ok(())));
    subject.poll_transition(now).unwrap();
    subject.poll_transition(now).unwrap();
    assert_eq!(subject.status(), ListenerStatus::Starting);
    assert!(subject.poll_accept(now).unwrap().is_none());
    subject.poll_transition(now).unwrap();
    let guard = admit(&mut subject, 1, CloseResult::Success, now);
    assert!(guard.is_current());
    assert_eq!(
        *trace.borrow(),
        [
            Event::Release(ReleaseCause::StartOrRebind, None),
            Event::Confirmed,
            Event::Survey,
            Event::Bind,
            Event::Survey,
        ]
    );
}

#[test]
fn start_requires_unchanged_approval_before_and_after_binding() {
    let now = Instant::now();
    for change_after_bind in [false, true] {
        let (mut subject, trace) = fixture(now);
        subject.start(approval(&subject), now).unwrap();
        subject.safety.result.set(Poll::Ready(Ok(())));
        subject.poll_transition(now).unwrap();
        if change_after_bind {
            subject.poll_transition(now).unwrap();
        }
        subject.driver.records = vec![interface("Changed")];
        assert_eq!(
            subject.poll_transition(now),
            Err(SupervisorError::Interface(InterfaceError::StaleInventory))
        );
        assert_eq!(subject.status(), ListenerStatus::Stopped);
        assert!(subject.poll_accept(now).unwrap().is_none());
        let trace = trace.borrow();
        assert_eq!(trace.contains(&Event::Bind), change_after_bind);
        assert_eq!(trace.contains(&Event::DropListeners), change_after_bind);
    }
}

#[test]
fn automatic_listener_closes_when_the_network_evidence_disappears() {
    let now = Instant::now();
    let (mut subject, trace) = fixture(now);
    let network = NetworkFingerprint::new([1; 32]);
    subject.driver.startup_network = Some(network);
    subject.request_inventory(now).unwrap();
    subject.poll_transition(now).unwrap();
    let (settings, _) = approval(&subject)
        .project_settings(&DesktopSettings::default(), false)
        .unwrap();
    let settings = settings.with_startup_network(Some(network)).unwrap();
    let selection = subject
        .inventory()
        .current()
        .unwrap()
        .restore_startup(&settings)
        .unwrap();
    subject.start(selection, now).unwrap();
    subject.safety.result.set(Poll::Ready(Ok(())));
    for _ in 0..3 {
        subject.poll_transition(now).unwrap();
    }
    let guard = admit(&mut subject, 1, CloseResult::Success, now);
    subject.driver.startup_network = None;
    let later = now + INVENTORY_INTERVAL;
    subject.poll_transition(later).unwrap();
    assert_eq!(
        subject.poll_transition(later),
        Err(SupervisorError::Interface(InterfaceError::StaleInventory))
    );
    assert!(!guard.is_current());
    assert!(trace.borrow().contains(&Event::Release(
        ReleaseCause::InterfaceChanged,
        Some(guard.generation())
    )));
}

#[test]
fn rebind_revokes_the_previous_generation_before_closing_or_publishing() {
    let now = Instant::now();
    let (mut subject, trace) = fixture(now);
    running(&mut subject, now);
    let guard = admit(&mut subject, 1, CloseResult::Success, now);
    subject.safety.result.set(Poll::Pending);
    subject.start(approval(&subject), now).unwrap();
    assert!(!guard.is_current());
    assert!(subject.poll_accept(now).unwrap().is_none());
    assert!(!trace.borrow().contains(&Event::Close(1)));
    subject.safety.result.set(Poll::Ready(Ok(())));
    for _ in 0..3 {
        subject.poll_transition(now).unwrap();
    }
    let replacement = admit(&mut subject, 2, CloseResult::Success, now);
    assert!(replacement.is_current());
    assert!(!guard.is_current());
    assert_ne!(replacement.generation(), guard.generation());
    let events = trace.borrow();
    let closed = events
        .iter()
        .position(|event| *event == Event::Close(1))
        .unwrap();
    let second_bind = events
        .iter()
        .rposition(|event| *event == Event::Bind)
        .unwrap();
    assert!(closed < second_bind);
}

#[test]
fn idle_inventory_failure_stops_admission_even_without_connections_arriving() {
    let now = Instant::now();
    let (mut subject, trace) = fixture(now);
    running(&mut subject, now);
    let guard = admit(&mut subject, 1, CloseResult::Success, now);
    let idle = now + INVENTORY_INTERVAL;
    subject.driver.fault = DriverFault::Refresh;
    subject.safety.result.set(Poll::Pending);
    subject.poll_transition(idle).unwrap();
    assert_eq!(
        subject.poll_transition(idle),
        Err(SupervisorError::Interface(
            InterfaceError::ProviderUnavailable
        ))
    );
    assert_eq!(subject.status(), ListenerStatus::Draining);
    assert!(!guard.is_current());
    assert!(trace.borrow().contains(&Event::Release(
        ReleaseCause::InterfaceChanged,
        Some(guard.generation())
    )));
    assert!(!trace.borrow().contains(&Event::Close(1)));
    subject.safety.result.set(Poll::Ready(Ok(())));
    subject.poll_transition(idle).unwrap();
    assert!(trace.borrow().contains(&Event::Close(1)));
}

#[test]
fn repeated_stop_retains_the_deadline_and_faults_closed_on_timeout() {
    let now = Instant::now();
    let (mut subject, trace) = fixture(now);
    running(&mut subject, now);
    let guard = admit(&mut subject, 1, CloseResult::Success, now);
    subject.safety.result.set(Poll::Pending);
    subject.start(approval(&subject), now).unwrap();
    let delay = RELEASE_TIMEOUT
        .checked_sub(Duration::from_millis(1))
        .unwrap();
    subject.stop(now + delay).unwrap();
    subject.poll_transition(now + delay).unwrap();
    assert_eq!(
        subject.poll_transition(now + RELEASE_TIMEOUT),
        Err(SupervisorError::ReleaseTimedOut)
    );
    assert_eq!(subject.status(), ListenerStatus::Faulted);
    assert!(!guard.is_current());
    assert!(trace.borrow().contains(&Event::Close(1)));
    assert_eq!(
        trace
            .borrow()
            .iter()
            .filter(|event| matches!(event, Event::Release(..)))
            .count(),
        2
    );
    assert_eq!(
        subject.start(approval(&subject), now + RELEASE_TIMEOUT),
        Err(SupervisorError::Faulted)
    );
}

#[test]
fn stop_cancels_a_queued_start_even_when_its_original_ticket_succeeds() {
    let now = Instant::now();
    let (mut subject, trace) = fixture(now);
    subject.start(approval(&subject), now).unwrap();
    subject.stop(now).unwrap();
    subject.safety.result.set(Poll::Ready(Ok(())));
    subject.poll_transition(now).unwrap();
    assert_eq!(subject.status(), ListenerStatus::Stopped);
    assert_eq!(
        *trace.borrow(),
        [
            Event::Release(ReleaseCause::StartOrRebind, None),
            Event::Confirmed
        ]
    );
}

#[test]
fn the_shutdown_registry_stays_bounded_and_recovers_released_slots() {
    let now = Instant::now();
    let (mut subject, _) = fixture(now);
    running(&mut subject, now);
    let mut held: Vec<_> = (0..u8::try_from(MAX_TRANSPORTS).unwrap())
        .map(|id| admit(&mut subject, id, CloseResult::Success, now))
        .collect();
    subject.driver.ready.push_back((100, CloseResult::Success));
    for _ in 0..2 {
        subject.poll_transition(now).unwrap();
    }
    assert!(subject.poll_accept(now).unwrap().is_none());
    assert_eq!(subject.driver.ready.len(), 1);
    drop(held.pop());
    let (_, recovered) = subject.poll_accept(now).unwrap().unwrap();
    assert!(recovered.is_current());
}

#[test]
fn an_exhausted_generation_never_wraps_to_a_previous_authority() {
    static EXHAUSTED: AtomicU64 = AtomicU64::new(u64::MAX);
    let now = Instant::now();
    let (mut subject, trace) = fixture(now);
    subject.generation_counter = &EXHAUSTED;
    subject.start(approval(&subject), now).unwrap();
    subject.safety.result.set(Poll::Ready(Ok(())));
    subject.poll_transition(now).unwrap();
    assert_eq!(
        subject.poll_transition(now),
        Err(SupervisorError::CounterExhausted)
    );
    assert_eq!(subject.status(), ListenerStatus::Faulted);
    assert!(!trace.borrow().contains(&Event::Bind));
}

#[test]
fn replacement_supervisors_use_different_generation_numbers() {
    let now = Instant::now();
    let (mut first, _) = fixture(now);
    running(&mut first, now);
    let retired = admit(&mut first, 1, CloseResult::Success, now);
    drop(first);
    assert!(!retired.is_current());
    let (mut second, _) = fixture(now);
    running(&mut second, now);
    let current = admit(&mut second, 2, CloseResult::Success, now);
    assert!(current.is_current());
    assert_ne!(retired.generation(), current.generation());
}

#[test]
fn stop_interrupts_a_pending_inventory_query_and_cancels_the_start() {
    let now = Instant::now();
    let (mut subject, trace) = fixture(now);
    subject.start(approval(&subject), now).unwrap();
    subject.safety.result.set(Poll::Ready(Ok(())));
    subject.poll_transition(now).unwrap();
    subject.driver.fault = DriverFault::RefreshPending;
    subject.poll_transition(now).unwrap();
    assert_eq!(subject.status(), ListenerStatus::Starting);
    subject.stop(now).unwrap();
    assert!(!subject.driver.pending);
    subject.poll_transition(now).unwrap();
    assert_eq!(subject.status(), ListenerStatus::Stopped);
    assert!(!trace.borrow().contains(&Event::Bind));
}

#[test]
fn closes_all_transports_after_a_shutdown_error_or_panic_and_latches_fault() {
    let now = Instant::now();
    for cleanup in [CloseResult::Failure, CloseResult::Panic] {
        let (mut subject, trace) = fixture(now);
        running(&mut subject, now);
        let first = admit(&mut subject, 1, cleanup, now);
        let second = admit(&mut subject, 2, CloseResult::Success, now);
        subject.stop(now).unwrap();
        assert!(!first.is_current());
        assert!(!second.is_current());
        assert_eq!(
            subject.poll_transition(now),
            Err(SupervisorError::CleanupFailed)
        );
        assert_eq!(subject.status(), ListenerStatus::Faulted);
        let events = trace.borrow();
        assert!(events.contains(&Event::Close(1)));
        assert!(events.contains(&Event::Close(2)));
        assert!(events.contains(&Event::DropListeners));
    }
}

#[test]
fn bind_failure_is_retryable_but_unconfirmed_release_is_fatal() {
    let now = Instant::now();
    let (mut subject, _) = fixture(now);
    subject.driver.fault = DriverFault::Bind;
    subject.start(approval(&subject), now).unwrap();
    subject.safety.result.set(Poll::Ready(Ok(())));
    subject.poll_transition(now).unwrap();
    assert_eq!(
        subject.poll_transition(now),
        Err(SupervisorError::Socket(()))
    );
    assert_eq!(subject.status(), ListenerStatus::Stopped);
    subject.driver.fault = DriverFault::None;
    running(&mut subject, now);
    let guard = admit(&mut subject, 1, CloseResult::Success, now);
    subject
        .safety
        .result
        .set(Poll::Ready(Err(SafetyError::NotNeutral)));
    subject.stop(now).unwrap();
    assert_eq!(
        subject.poll_transition(now),
        Err(SupervisorError::Safety(SafetyError::NotNeutral))
    );
    assert!(!guard.is_current());
    assert_eq!(subject.status(), ListenerStatus::Faulted);
}

#[test]
fn transport_error_uses_reserved_release_and_failed_submission_closes_sockets() {
    let now = Instant::now();
    let (mut subject, trace) = fixture(now);
    running(&mut subject, now);
    let guard = admit(&mut subject, 1, CloseResult::Success, now);
    for _ in 0..2 {
        subject.poll_transition(now).unwrap();
    }
    subject.driver.fault = DriverFault::Accept;
    subject.safety.reject = true;
    assert_eq!(
        subject.poll_accept(now).unwrap_err(),
        SupervisorError::Safety(SafetyError::Unavailable)
    );
    assert!(!guard.is_current());
    assert_eq!(subject.status(), ListenerStatus::Faulted);
    assert!(trace.borrow().contains(&Event::Release(
        ReleaseCause::TransportFault,
        Some(guard.generation())
    )));
    assert!(trace.borrow().contains(&Event::Close(1)));
}

struct DispatcherSafety<'a> {
    dispatcher: &'a InputDispatcher<MockInputBackend>,
    receipt: &'a RefCell<Option<ReleaseTicket>>,
}

struct DispatcherTicket(ReleaseTicket);

impl SafetyCoordinator for DispatcherSafety<'_> {
    type Ticket = DispatcherTicket;

    fn request_release(
        &mut self,
        _cause: ReleaseCause,
        _obsolete: Option<NonZeroU64>,
    ) -> Result<DispatcherTicket, SafetyError> {
        let ticket = self
            .dispatcher
            .request_release(EmergencyReason::Lifecycle)
            .map_err(|_| SafetyError::Unavailable)?;
        self.receipt.replace(Some(ticket.clone()));
        Ok(DispatcherTicket(ticket))
    }
}

impl NeutralityTicket for DispatcherTicket {
    fn poll_neutrality(&mut self) -> Poll<Result<(), SafetyError>> {
        match self.0.try_outcome() {
            Some(outcome) if outcome.report.is_confirmed_neutral() => Poll::Ready(Ok(())),
            Some(_) => Poll::Ready(Err(SafetyError::NotNeutral)),
            None => Poll::Pending,
        }
    }
}

#[test]
fn dropping_the_supervisor_does_not_cancel_an_accepted_mock_dispatcher_release() {
    let now = Instant::now();
    let dispatcher =
        InputDispatcher::start(MockInputBackend::new(), DispatcherConfig::default()).unwrap();
    let held = dispatcher
        .submit(InputAction::MouseButton(ButtonAction::Down(
            MouseButton::Left,
        )))
        .unwrap()
        .wait_timeout(RELEASE_TIMEOUT)
        .unwrap();
    assert!(matches!(held, DispatchOutcome::Executed { .. }));
    let receipt = RefCell::default();
    let safety = DispatcherSafety {
        dispatcher: &dispatcher,
        receipt: &receipt,
    };
    let mut subject = ListenerSupervisor::new(FakeDriver::new(Trace::default()), safety);
    subject.request_inventory(now).unwrap();
    subject.poll_transition(now).unwrap();
    subject.start(approval(&subject), now).unwrap();
    assert_eq!(subject.status(), ListenerStatus::Draining);
    drop(subject);
    let report = receipt
        .into_inner()
        .unwrap()
        .wait_timeout(RELEASE_TIMEOUT)
        .unwrap();
    assert_eq!(report.report.attempted, 1);
    assert!(report.report.is_confirmed_neutral());
    assert!(dispatcher.shutdown().backend.is_neutral());
}
