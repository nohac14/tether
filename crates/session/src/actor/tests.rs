use super::*;
use prost::Message;
use std::sync::{Condvar, Mutex};
use std::thread;
use std::time::Instant;
use tether_input_core::{
    BackendCapabilities, BackendError, BackendErrorCategory, BackendKind, CapabilityState,
    DispatcherConfig, MockInputBackend, MouseButton, OperationClass, ReleaseFailure, ReleaseReport,
    RetryScope, StatusRefreshReason, TextMode,
};
use tether_protocol::wire;
use wire::{client_envelope::Payload, client_operation::Action};

const WAIT: Duration = Duration::from_secs(3);
const NOW: Duration = Duration::ZERO;
const INITIAL: TransportGeneration = TransportGeneration(NonZeroU64::MIN);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Block {
    Execute,
    Release,
}

#[derive(Default)]
struct Gate {
    command: Mutex<Option<(Block, bool)>>,
    signal: Condvar,
}

impl Gate {
    fn arm(&self, block: Block) {
        let mut command = self.command.lock().unwrap();
        assert!(command.is_none());
        *command = Some((block, false));
    }

    fn pause(&self, block: Block) {
        let mut command = self.command.lock().unwrap();
        if *command == Some((block, false)) {
            *command = Some((block, true));
            self.signal.notify_all();
            let (_guard, outcome) = self
                .signal
                .wait_timeout_while(command, WAIT, |slot| slot.is_some())
                .unwrap();
            assert!(!outcome.timed_out(), "mock gate must be released");
        }
    }

    fn wait(&self, block: Block) {
        let command = self.command.lock().unwrap();
        let (_guard, outcome) = self
            .signal
            .wait_timeout_while(command, WAIT, |slot| *slot != Some((block, true)))
            .unwrap();
        assert!(!outcome.timed_out(), "mock worker must reach gate");
    }

    fn resume(&self) {
        *self.command.lock().unwrap() = None;
        self.signal.notify_all();
    }
}

struct TestBackend {
    mock: MockInputBackend,
    gate: Arc<Gate>,
}

impl InputBackend for TestBackend {
    fn status(&self) -> BackendStatus {
        backend_status()
    }
    fn refresh_status(&mut self, _: StatusRefreshReason) -> BackendStatus {
        self.status()
    }
    fn execute(&mut self, action: &InputAction) -> Result<(), BackendError> {
        self.gate.pause(Block::Execute);
        self.mock.execute(action)
    }
    fn release_all(&mut self) -> ReleaseReport {
        self.gate.pause(Block::Release);
        self.mock.release_all()
    }
}

fn backend_status() -> BackendStatus {
    let mut capabilities = BackendCapabilities::unavailable();
    capabilities.relative_pointer = CapabilityState::Available;
    capabilities.mouse_buttons.insert(MouseButton::Left);
    capabilities
        .text_modes
        .insert(TextMode::NativeUnicodeBestEffort);
    BackendStatus {
        revision: 1,
        initialized: true,
        available: true,
        kind: BackendKind::Mock,
        permission: PermissionState::NotRequired,
        capabilities,
        fault: None,
    }
}

fn dispatcher(
    capacity: usize,
    mock: MockInputBackend,
) -> (InputDispatcher<TestBackend>, Arc<Gate>) {
    let gate = Arc::new(Gate::default());
    let worker = InputDispatcher::start(
        TestBackend {
            mock,
            gate: Arc::clone(&gate),
        },
        DispatcherConfig::new(capacity).unwrap(),
    )
    .unwrap();
    (worker, gate)
}

type Actor<'d> = SessionActor<'d, TestBackend, u8>;

fn start(worker: &mut InputDispatcher<TestBackend>) -> Actor<'_> {
    let mut actor = SessionActor::new(worker, INITIAL, backend_status(), Limits::HARD, NOW);
    run_until(&mut actor, NOW, |actor| {
        actor.snapshot().availability == Availability::Ready
    });
    assert!(actor.snapshot().confirmed_neutral);
    actor
}

fn run_until(actor: &mut Actor<'_>, now: Duration, mut condition: impl FnMut(&Actor<'_>) -> bool) {
    let deadline = Instant::now() + WAIT;
    loop {
        actor.poll(now);
        if condition(actor) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "last state: {:?}",
            actor.snapshot()
        );
        thread::sleep(Duration::from_millis(1));
    }
}

fn authority(device: u8) -> VerifiedConnection<u8> {
    VerifiedConnection {
        device,
        authority_revision: NonZeroU64::MIN,
        expires_at: Duration::from_secs(30),
        generation: INITIAL,
        credential: CredentialAuthority::active(),
    }
}

fn hello() -> Vec<u8> {
    wire::ClientEnvelope {
        protocol_major: 1,
        payload: Some(Payload::ClientHello(wire::ClientHello {
            protocol_major: 1,
            build_identifier: String::from("test-build"),
            client_display_name: String::from("test-device"),
            limits: Some(wire::ClientLimits {
                maximum_encoded_message_bytes: 65_536,
                maximum_operations_per_batch: 256,
                maximum_text_operation_bytes: 4096,
            }),
            capabilities: vec![1, 2],
            ..Default::default()
        })),
        ..Default::default()
    }
    .encode_to_vec()
}

fn connect(actor: &mut Actor<'_>, device: u8) -> ConnectionLease {
    let lease = actor.register(NOW, authority(device)).unwrap();
    actor.receive(NOW, &lease, &hello()).unwrap();
    assert!(matches!(
        actor.next_output(NOW, &lease),
        Some(SessionOutput::Hello { .. })
    ));
    lease
}

fn message(sequence: u64, epoch: u64, payload: Payload) -> Vec<u8> {
    wire::ClientEnvelope {
        protocol_major: 1,
        sequence,
        session_epoch: epoch,
        client_timestamp_micros: sequence,
        payload: Some(payload),
        ..Default::default()
    }
    .encode_to_vec()
}

fn batch(
    actor: &mut Actor<'_>,
    lease: &ConnectionLease,
    sequence: u64,
    actions: impl IntoIterator<Item = Action>,
) -> Result<(), SessionError> {
    let payload = Payload::OperationBatch(wire::ClientOperationBatch {
        operations: actions
            .into_iter()
            .map(|action| wire::ClientOperation {
                action: Some(action),
                timestamp_offset_micros: 0,
            })
            .collect(),
    });
    actor.receive(
        NOW,
        lease,
        &message(sequence, actor.snapshot().epoch, payload),
    )
}

fn button(state: i32) -> Action {
    Action::MouseButtonTransition(wire::MouseButtonTransition { button: 1, state })
}

fn pointer() -> Action {
    Action::PointerMove(wire::PointerMove {
        delta_x_1024: 100,
        delta_y_1024: -200,
    })
}

fn ping(sequence: u64, epoch: u64, identifier: u64) -> Vec<u8> {
    message(
        sequence,
        epoch,
        Payload::HeartbeatPing(wire::HeartbeatPing {
            heartbeat_identifier: identifier,
        }),
    )
}

fn request(actor: &mut Actor<'_>, lease: &ConnectionLease, sequence: u64) {
    batch(
        actor,
        lease,
        sequence,
        [Action::RequestControl(wire::RequestControl {})],
    )
    .unwrap();
}

fn claim(actor: &mut Actor<'_>, lease: &ConnectionLease, sequence: u64) {
    request(actor, lease, sequence);
    run_until(actor, NOW, |actor| actor.snapshot().has_owner);
    let expected = ControlSnapshot {
        epoch: actor.snapshot().epoch,
        role: ControlRole::Controlling,
    };
    assert!(
        matches!(actor.next_output(NOW, lease), Some(SessionOutput::Control(value)) if value == expected)
    );
}

#[test]
fn actor_outputs_encode_a_valid_hello_control_update_and_heartbeat_in_order() {
    use tether_protocol::validation::{ServerContext, validate_server, validate_server_hello};

    let (mut worker, _) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let lease = actor.register(NOW, authority(1)).unwrap();
    let offer = validate_client_hello(&hello(), true, actor.limits).unwrap();
    let mut encoder = SessionOutputEncoder::new([7; 16]);
    assert_eq!(
        encoder.encode(SessionOutput::Pong {
            epoch: 1,
            identifier: 0
        }),
        Err(SessionError::InvalidMessage)
    );
    actor.receive(NOW, &lease, &hello()).unwrap();
    let hello = encoder
        .encode(actor.next_output(NOW, &lease).unwrap())
        .unwrap();
    let welcome = validate_server_hello(&hello, &offer)
        .unwrap()
        .into_envelope();
    let Some(wire::server_envelope::Payload::ServerHello(welcome)) = welcome.payload else {
        panic!("expected validated hello");
    };
    let control = welcome.control_state.unwrap();
    assert_eq!(welcome.connection_identifier, [7; 16]);
    let mut context = ServerContext {
        next_sequence: 1,
        epoch: control.session_epoch,
        capability_revision: welcome.capabilities.unwrap().revision,
        control_revision: control.revision,
        limits: offer.limits(),
    };

    request(&mut actor, &lease, 1);
    run_until(&mut actor, NOW, |actor| actor.snapshot().has_owner);
    let update = encoder
        .encode(actor.next_output(NOW, &lease).unwrap())
        .unwrap();
    let update = validate_server(&update, &context).unwrap().into_envelope();
    let Some(wire::server_envelope::Payload::ControlState(control)) = update.payload else {
        panic!("expected validated control update");
    };
    assert!(control.this_client_controls);
    context.epoch = control.session_epoch;
    context.control_revision = control.revision;
    context.next_sequence += 1;
    actor
        .receive(NOW, &lease, &ping(2, context.epoch, 42))
        .unwrap();
    let pong = encoder
        .encode(actor.next_output(NOW, &lease).unwrap())
        .unwrap();
    let pong = validate_server(&pong, &context).unwrap().into_envelope();
    assert!(
        matches!(pong.payload, Some(wire::server_envelope::Payload::HeartbeatPong(value)) if value.heartbeat_identifier == 42)
    );
}

#[test]
fn grant_waits_for_neutral_and_preserves_queued_down_motion_up() {
    let (mut worker, gate) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let lease = connect(&mut actor, 1);
    let before = actor.snapshot().epoch;
    claim(&mut actor, &lease, 1);
    assert!(actor.snapshot().epoch > before);
    gate.arm(Block::Execute);
    batch(&mut actor, &lease, 2, [button(1)]).unwrap();
    gate.wait(Block::Execute);
    batch(&mut actor, &lease, 3, [pointer(), button(2)]).unwrap();
    assert_eq!(actor.snapshot().pending_actions, 3);
    gate.resume();
    run_until(&mut actor, NOW, |actor| {
        actor.snapshot().pending_actions == 0
    });
    assert_eq!(actor.dispatcher.snapshot().held_buttons, 0);
    drop(actor);
    let stopped = worker.shutdown();
    assert!(stopped.backend.mock.is_neutral());
    assert_eq!(stopped.backend.mock.calls().len(), 3);
}

#[test]
fn transfer_removes_old_authority_before_cleanup_and_ignores_stale_close() {
    let (mut worker, gate) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let first = connect(&mut actor, 1);
    claim(&mut actor, &first, 1);
    batch(&mut actor, &first, 2, [button(1)]).unwrap();
    run_until(&mut actor, NOW, |actor| {
        actor.snapshot().pending_actions == 0
    });
    assert_eq!(actor.dispatcher.snapshot().held_buttons, 1);
    let second = connect(&mut actor, 2);
    gate.arm(Block::Release);
    request(&mut actor, &second, 1);
    gate.wait(Block::Release);
    let old_epoch = actor.snapshot().epoch;
    assert_eq!(actor.snapshot().availability, Availability::Releasing);
    assert!(!actor.snapshot().has_owner);
    assert!(actor.next_output(NOW, &second).is_none());
    assert_eq!(
        batch(&mut actor, &first, 3, [pointer()]),
        Err(SessionError::InvalidMessage)
    );
    assert!(first.is_closed());
    gate.resume();
    run_until(&mut actor, NOW, |actor| actor.snapshot().has_owner);
    assert!(actor.snapshot().epoch > old_epoch);
    assert_eq!(actor.dispatcher.snapshot().held_buttons, 0);
    first.disconnect();
    actor.poll(NOW);
    assert!(actor.snapshot().has_owner);
    assert!(
        matches!(actor.next_output(NOW, &second), Some(SessionOutput::Control(control)) if control.role == ControlRole::Controlling)
    );
}

#[test]
fn dropped_target_cannot_reacquire_after_the_release_acknowledgement() {
    let (mut worker, gate) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let lease = connect(&mut actor, 1);
    gate.arm(Block::Release);
    request(&mut actor, &lease, 1);
    gate.wait(Block::Release);
    drop(lease);
    actor.poll(NOW);
    assert_eq!(actor.snapshot().connections, 0);
    gate.resume();
    run_until(&mut actor, NOW, |actor| {
        actor.snapshot().availability == Availability::Ready
    });
    assert!(!actor.snapshot().has_owner);
    assert!(actor.snapshot().confirmed_neutral);
}

#[test]
fn equal_deadline_faults_and_late_completion_cannot_grant() {
    let (mut worker, gate) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let lease = connect(&mut actor, 1);
    gate.arm(Block::Release);
    request(&mut actor, &lease, 1);
    gate.wait(Block::Release);
    actor.poll(RELEASE_TIMEOUT);
    assert_eq!(actor.snapshot().availability, Availability::Faulted);
    assert!(!actor.snapshot().confirmed_neutral);
    assert!(lease.is_closed());
    gate.resume();
    run_until(&mut actor, RELEASE_TIMEOUT, |actor| {
        actor.dispatcher.snapshot().mode == DispatcherMode::Faulted
    });
    assert_eq!(
        actor.recover(RELEASE_TIMEOUT, INITIAL),
        Err(SessionError::Unavailable)
    );
    assert!(!actor.snapshot().has_owner);
}

#[test]
fn invalid_standby_work_and_unrelated_revocation_cannot_release_the_owner() {
    let (mut worker, _) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let owner = connect(&mut actor, 1);
    claim(&mut actor, &owner, 1);
    batch(&mut actor, &owner, 2, [button(1)]).unwrap();
    run_until(&mut actor, NOW, |actor| {
        actor.snapshot().pending_actions == 0
    });
    let other = connect(&mut actor, 2);
    assert_eq!(
        batch(&mut actor, &other, 1, [pointer()]),
        Err(SessionError::InvalidMessage)
    );
    assert!(other.is_closed());
    let replacement = connect(&mut actor, 2);
    actor.revoke_device(NOW, 2);
    assert!(replacement.is_closed());
    assert_eq!(actor.dispatcher.snapshot().held_buttons, 1);
    assert!(actor.snapshot().has_owner);
    actor.revoke_device(NOW, 1);
    run_until(&mut actor, NOW, |actor| actor.snapshot().confirmed_neutral);
    assert!(owner.is_closed());
    assert_eq!(actor.dispatcher.snapshot().held_buttons, 0);
}

#[test]
fn one_lease_per_device_is_reserved_before_hello_and_released_on_disconnect() {
    let (mut worker, _) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let first = actor.register(NOW, authority(1)).unwrap();
    assert_eq!(
        actor.register(NOW, authority(1)).unwrap_err(),
        SessionError::Capacity
    );
    assert_eq!(actor.snapshot().connections, 1);
    first.disconnect();
    let replacement = connect(&mut actor, 1);
    assert!(first.is_closed());
    assert!(!replacement.is_closed());
    assert_eq!(actor.snapshot().connections, 1);
}

#[test]
fn action_saturation_closes_owner_and_uses_reserved_cleanup() {
    let (mut worker, gate) = dispatcher(2, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let owner = connect(&mut actor, 1);
    claim(&mut actor, &owner, 1);
    gate.arm(Block::Execute);
    batch(&mut actor, &owner, 2, [button(1)]).unwrap();
    gate.wait(Block::Execute);
    batch(&mut actor, &owner, 3, [button(2)]).unwrap();
    assert_eq!(
        batch(&mut actor, &owner, 4, [pointer()]),
        Err(SessionError::Capacity)
    );
    assert!(owner.is_closed());
    assert_eq!(actor.dispatcher.snapshot().queued_actions, 0);
    gate.resume();
    run_until(&mut actor, NOW, |actor| actor.snapshot().confirmed_neutral);
    drop(actor);
    let result = worker.shutdown();
    assert!(result.backend.mock.is_neutral());
    assert_eq!(
        result.backend.mock.calls().len(),
        1,
        "only in-flight down could execute"
    );
}

#[test]
fn an_authenticated_message_burst_releases_held_input_through_the_reserved_path() {
    let (mut worker, _) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let lease = connect(&mut actor, 1);
    claim(&mut actor, &lease, 1);
    batch(&mut actor, &lease, 2, [button(1)]).unwrap();
    run_until(&mut actor, NOW, |actor| {
        actor.snapshot().pending_actions == 0
    });
    let epoch = actor.snapshot().epoch;
    for sequence in 3..=120 {
        actor
            .receive(NOW, &lease, &ping(sequence, epoch, sequence))
            .unwrap();
        assert!(matches!(
            actor.next_output(NOW, &lease),
            Some(SessionOutput::Pong { .. })
        ));
    }
    assert_eq!(
        actor.receive(NOW, &lease, &ping(121, epoch, 121)),
        Err(SessionError::RateLimited)
    );
    assert!(lease.is_closed());
    run_until(&mut actor, NOW, |actor| actor.snapshot().confirmed_neutral);
    assert_eq!(actor.dispatcher.snapshot().held_buttons, 0);
}

#[test]
fn explicit_release_preempts_a_full_queue_and_global_signal_cancels_regrant() {
    let (mut worker, gate) = dispatcher(1, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let owner = connect(&mut actor, 1);
    claim(&mut actor, &owner, 1);
    gate.arm(Block::Execute);
    batch(&mut actor, &owner, 2, [button(1)]).unwrap();
    gate.wait(Block::Execute);
    batch(
        &mut actor,
        &owner,
        3,
        [Action::ReleaseAll(wire::ReleaseAll {})],
    )
    .unwrap();
    let signals = actor.safety_signal();
    signals.request(GlobalStop::StopControl);
    signals.request(GlobalStop::Lifecycle);
    actor.poll(NOW);
    assert!(!actor.snapshot().has_owner);
    gate.resume();
    run_until(&mut actor, NOW, |actor| {
        actor.snapshot().availability == Availability::Suspended
    });
    assert_eq!(actor.dispatcher.snapshot().held_buttons, 0);
    actor.recover(NOW, INITIAL).unwrap();
    assert!(!actor.snapshot().has_owner);
    assert!(actor.snapshot().confirmed_neutral);
}

#[test]
fn a_stalled_owner_output_closes_and_neutralizes() {
    let (mut worker, _) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let owner = connect(&mut actor, 1);
    claim(&mut actor, &owner, 1);
    batch(&mut actor, &owner, 2, [button(1)]).unwrap();
    let epoch = actor.snapshot().epoch;
    for index in 0..=MAX_OUTPUTS {
        let sequence = u64::try_from(index).unwrap() + 3;
        let packet = message(
            sequence,
            epoch,
            Payload::HeartbeatPing(wire::HeartbeatPing {
                heartbeat_identifier: sequence,
            }),
        );
        let expected = if index == MAX_OUTPUTS {
            Err(SessionError::Capacity)
        } else {
            Ok(())
        };
        assert_eq!(actor.receive(NOW, &owner, &packet), expected);
    }
    run_until(&mut actor, NOW, |actor| actor.snapshot().confirmed_neutral);
    assert!(owner.is_closed());
    assert_eq!(actor.dispatcher.snapshot().held_buttons, 0);
}

#[test]
fn registrations_are_bounded_and_hello_expiry_wins_at_equality() {
    let (mut worker, _) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let leases: Vec<_> = (0..u8::try_from(MAX_CONNECTIONS).unwrap())
        .map(|device| actor.register(NOW, authority(device)).unwrap())
        .collect();
    assert_eq!(
        actor.register(NOW, authority(9)).unwrap_err(),
        SessionError::Capacity
    );
    assert_eq!(
        actor.receive(HELLO_TIMEOUT, &leases[0], &hello()),
        Err(SessionError::Closed)
    );
    assert!(leases.iter().all(ConnectionLease::is_closed));
    assert_eq!(actor.snapshot().connections, 0);
    assert_eq!(
        actor
            .register(
                HELLO_TIMEOUT,
                VerifiedConnection {
                    expires_at: HELLO_TIMEOUT,
                    ..authority(7)
                }
            )
            .unwrap_err(),
        SessionError::StaleAuthority
    );
}

#[test]
fn transport_retirement_closes_old_leases_and_requires_new_generation() {
    let (mut worker, _) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let lease = connect(&mut actor, 1);
    claim(&mut actor, &lease, 1);
    actor.safety_signal().request(GlobalStop::RetireTransport);
    run_until(&mut actor, NOW, |actor| {
        actor.snapshot().availability == Availability::Suspended
    });
    assert!(lease.is_closed());
    assert!(actor.snapshot().confirmed_neutral);
    assert_eq!(
        actor.recover(NOW, INITIAL),
        Err(SessionError::StaleAuthority)
    );
    let generation = TransportGeneration(NonZeroU64::new(2).unwrap());
    actor.recover(NOW, generation).unwrap();
    assert_eq!(
        actor.register(NOW, authority(1)).unwrap_err(),
        SessionError::StaleAuthority
    );
    let current = actor
        .register(
            NOW,
            VerifiedConnection {
                generation,
                ..authority(1)
            },
        )
        .unwrap();
    assert_eq!(
        actor.receive(NOW, &lease, &hello()),
        Err(SessionError::Closed)
    );
    actor.receive(NOW, &current, &hello()).unwrap();
    assert!(!actor.snapshot().has_owner);
}

#[test]
fn guarded_backend_rejection_cancels_dependent_work_before_actor_polls() {
    let mut mock = MockInputBackend::new();
    mock.push_execute_result(Err(BackendError::new(
        BackendErrorCategory::Unsupported,
        OperationClass::PointerMove,
        RetryScope::Never,
    )));
    let (mut worker, _) = dispatcher(4, mock);
    let mut actor = start(&mut worker);
    let owner = connect(&mut actor, 1);
    claim(&mut actor, &owner, 1);
    batch(&mut actor, &owner, 2, [pointer(), button(1)]).unwrap();
    run_until(&mut actor, NOW, |actor| {
        actor.snapshot().availability == Availability::Suspended
    });
    assert!(!actor.snapshot().has_owner);
    assert!(actor.snapshot().confirmed_neutral);
    drop(actor);
    let result = worker.shutdown();
    assert!(result.backend.mock.is_neutral());
    assert_eq!(result.backend.mock.calls().len(), 1);
}

#[test]
fn uncertain_startup_release_is_terminal_even_after_a_later_neutral_attempt() {
    let mut mock = MockInputBackend::new();
    mock.push_release_report(ReleaseReport {
        attempted: 1,
        released: 0,
        failures: vec![ReleaseFailure::BackendLocalState],
        neutral: false,
    });
    let (mut worker, _) = dispatcher(4, mock);
    let mut actor = SessionActor::new(&mut worker, INITIAL, backend_status(), Limits::HARD, NOW);
    run_until(&mut actor, NOW, |actor| {
        actor.snapshot().availability == Availability::Faulted
    });
    assert!(!actor.snapshot().confirmed_neutral);
    assert_eq!(
        actor.register(NOW, authority(1)).unwrap_err(),
        SessionError::Unavailable
    );
    assert_eq!(actor.recover(NOW, INITIAL), Err(SessionError::Unavailable));
}

#[test]
fn process_epochs_are_unique_across_actors_and_exhaustion_does_not_wrap() {
    let (mut a, _) = dispatcher(4, MockInputBackend::new());
    let first = start(&mut a);
    let (mut b, _) = dispatcher(4, MockInputBackend::new());
    let second = start(&mut b);
    assert!(second.snapshot().epoch > first.snapshot().epoch);
    let trial = AtomicU64::new(u64::MAX - 1);
    assert_eq!(allocate_epoch(&trial), Some(u64::MAX - 1));
    assert_eq!(allocate_epoch(&trial), None);
    assert_eq!(allocate_epoch(&trial), None);
}

#[test]
fn open_transport_and_input_traffic_cannot_extend_application_liveness() {
    let (mut worker, _) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let owner = connect(&mut actor, 1);
    claim(&mut actor, &owner, 1);
    batch(&mut actor, &owner, 2, [button(1)]).unwrap();
    run_until(&mut actor, NOW, |actor| {
        actor.snapshot().pending_actions == 0
    });
    assert!(actor.snapshot().has_owner);
    let epoch = actor.snapshot().epoch;
    let move_packet = message(
        3,
        epoch,
        Payload::OperationBatch(wire::ClientOperationBatch {
            operations: vec![wire::ClientOperation {
                action: Some(pointer()),
                timestamp_offset_micros: 0,
            }],
        }),
    );
    let timeout = APPLICATION_HEARTBEAT.timeout;
    actor
        .receive(
            timeout.checked_sub(Duration::from_millis(1)).unwrap(),
            &owner,
            &move_packet,
        )
        .unwrap();
    assert_eq!(actor.next_deadline(), Some(timeout));
    assert_eq!(
        actor.receive(timeout, &owner, &ping(4, epoch, 1)),
        Err(SessionError::Closed)
    );
    assert!(owner.is_closed());
    run_until(&mut actor, timeout, |actor| {
        actor.snapshot().confirmed_neutral
    });
    assert_eq!(actor.dispatcher.snapshot().held_buttons, 0);
    assert!(!actor.snapshot().has_owner);
}

#[test]
fn validated_ping_renews_from_server_time_and_late_ping_does_not_revive() {
    let (mut worker, _) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let owner = connect(&mut actor, 1);
    claim(&mut actor, &owner, 1);
    let epoch = actor.snapshot().epoch;
    let arrival = Duration::from_millis(3_900);
    actor.receive(arrival, &owner, &ping(2, epoch, 13)).unwrap();
    assert!(matches!(
        actor.next_output(arrival, &owner),
        Some(SessionOutput::Pong { identifier: 13, .. })
    ));
    let deadline = arrival + APPLICATION_HEARTBEAT.timeout;
    assert_eq!(actor.next_deadline(), Some(deadline));
    actor.poll(deadline.checked_sub(Duration::from_millis(1)).unwrap());
    assert!(actor.snapshot().has_owner);
    assert_eq!(
        actor.receive(deadline, &owner, &ping(3, epoch, 14)),
        Err(SessionError::Closed)
    );
    run_until(&mut actor, deadline, |actor| {
        actor.snapshot().confirmed_neutral
    });
    let fresh = actor.register(deadline, authority(1)).unwrap();
    actor.receive(deadline, &fresh, &hello()).unwrap();
    assert!(!actor.snapshot().has_owner);
}

#[test]
fn an_expired_takeover_target_is_removed_before_neutral_completion_can_grant() {
    let (mut worker, gate) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let owner = connect(&mut actor, 1);
    claim(&mut actor, &owner, 1);
    let target = connect(&mut actor, 2);
    let arrival = Duration::from_millis(3_500);
    let epoch = actor.snapshot().epoch;
    actor.receive(arrival, &owner, &ping(2, epoch, 17)).unwrap();
    gate.arm(Block::Release);
    let takeover = message(
        1,
        epoch,
        Payload::OperationBatch(wire::ClientOperationBatch {
            operations: vec![wire::ClientOperation {
                timestamp_offset_micros: 0,
                action: Some(Action::RequestControl(wire::RequestControl {})),
            }],
        }),
    );
    actor.receive(arrival, &target, &takeover).unwrap();
    gate.wait(Block::Release);
    actor.poll(APPLICATION_HEARTBEAT.timeout);
    assert!(target.is_closed());
    gate.resume();
    run_until(&mut actor, APPLICATION_HEARTBEAT.timeout, |actor| {
        actor.snapshot().confirmed_neutral
    });
    assert!(!actor.snapshot().has_owner);
    assert!(!owner.is_closed());
}

#[test]
fn heartbeat_clock_overflow_faults_without_publishing_hello() {
    let (mut worker, _) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let time = Duration::MAX.checked_sub(Duration::from_secs(3)).unwrap();
    let lease = actor
        .register(
            time,
            VerifiedConnection {
                expires_at: Duration::MAX,
                ..authority(1)
            },
        )
        .unwrap();
    assert_eq!(
        actor.receive(time, &lease, &hello()),
        Err(SessionError::InternalFault)
    );
    assert!(lease.is_closed());
    assert!(actor.next_output(time, &lease).is_none());
    assert!(!actor.snapshot().has_owner);
}

#[test]
fn actor_panic_with_held_input_closes_the_lease_and_neutralizes_the_mock() {
    let (mut worker, _) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let lease = connect(&mut actor, 1);
    claim(&mut actor, &lease, 1);
    batch(&mut actor, &lease, 2, [button(1)]).unwrap();
    run_until(&mut actor, NOW, |actor| {
        actor.snapshot().pending_actions == 0
    });
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _actor = actor;
        panic!("synthetic actor panic");
    }));
    assert!(unwind.is_err());
    assert!(lease.is_closed());
    let stopped = worker.shutdown();
    assert!(stopped.backend.mock.is_neutral());
}

#[test]
fn retained_text_budget_rejects_the_whole_envelope_before_dispatch() {
    let (mut worker, gate) = dispatcher(256, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let lease = connect(&mut actor, 1);
    claim(&mut actor, &lease, 1);
    let upload = || {
        (0..4).map(|_| {
            Action::TextInsertion(wire::TextInsertion {
                text: "x".repeat(4096),
            })
        })
    };
    gate.arm(Block::Execute);
    batch(&mut actor, &lease, 2, upload()).unwrap();
    gate.wait(Block::Execute);
    for sequence in 3..=8 {
        batch(&mut actor, &lease, sequence, upload()).unwrap();
    }
    assert!(actor.snapshot().pending_actions < MAX_ACTIONS);
    let rejection = batch(&mut actor, &lease, 9, upload());
    gate.resume();
    assert_eq!(rejection, Err(SessionError::Capacity));
    assert!(lease.is_closed());
    run_until(&mut actor, NOW, |actor| actor.snapshot().confirmed_neutral);
    drop(actor);
    let stopped = worker.shutdown();
    assert!(stopped.backend.mock.is_neutral());
    assert_eq!(stopped.backend.mock.calls().len(), 1);
}

#[test]
fn explicit_release_grants_a_fresh_neutral_epoch_and_shutdown_closes_it() {
    let (mut worker, _) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let lease = connect(&mut actor, 1);
    claim(&mut actor, &lease, 1);
    let previous = actor.snapshot().epoch;
    batch(&mut actor, &lease, 2, [button(1)]).unwrap();
    run_until(&mut actor, NOW, |actor| {
        actor.snapshot().pending_actions == 0
    });
    assert_eq!(actor.dispatcher.snapshot().held_buttons, 1);
    batch(
        &mut actor,
        &lease,
        3,
        [Action::ReleaseAll(wire::ReleaseAll {})],
    )
    .unwrap();
    run_until(&mut actor, NOW, |actor| actor.snapshot().has_owner);
    let epoch = actor.snapshot().epoch;
    assert!(epoch > previous);
    assert!(
        matches!(actor.next_output(NOW, &lease), Some(SessionOutput::Control(control))
        if control.epoch == epoch && control.role == ControlRole::Controlling)
    );
    assert_eq!(actor.dispatcher.snapshot().held_buttons, 0);
    actor.safety_signal().request(GlobalStop::Shutdown);
    run_until(&mut actor, NOW, |actor| {
        actor.snapshot().availability == Availability::Stopped
    });
    assert!(lease.is_closed());
    assert!(actor.snapshot().confirmed_neutral);
    assert!(!actor.snapshot().has_owner);
}

#[test]
fn malformed_owner_frame_closes_the_lease_and_releases_held_input() {
    let (mut worker, _) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let lease = connect(&mut actor, 1);
    claim(&mut actor, &lease, 1);
    batch(&mut actor, &lease, 2, [button(1)]).unwrap();
    run_until(&mut actor, NOW, |actor| {
        actor.snapshot().pending_actions == 0
    });
    assert_eq!(actor.dispatcher.snapshot().held_buttons, 1);
    assert_eq!(
        actor.receive(NOW, &lease, &[0xff]),
        Err(SessionError::InvalidMessage)
    );
    assert!(lease.is_closed());
    run_until(&mut actor, NOW, |actor| actor.snapshot().confirmed_neutral);
    assert_eq!(actor.dispatcher.snapshot().held_buttons, 0);
}

#[test]
fn quiescence_requires_fresh_release_and_dropped_ticket_cannot_cancel_it() {
    let (mut worker, gate) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let first_epoch = actor.snapshot().epoch;
    let signal = actor.safety_signal();
    gate.arm(Block::Release);
    let ticket = signal
        .request_quiescence(QuiescenceMode::RetireTransport)
        .unwrap();
    assert_eq!(ticket.poll(), std::task::Poll::Pending);
    drop(ticket);
    actor.poll(NOW);
    gate.wait(Block::Release);
    assert_eq!(actor.snapshot().availability, Availability::Releasing);
    let repeated = signal
        .request_quiescence(QuiescenceMode::RetireTransport)
        .unwrap();
    assert_eq!(repeated.poll(), std::task::Poll::Pending);
    gate.resume();
    run_until(&mut actor, NOW, |_| repeated.poll().is_ready());
    assert_eq!(repeated.poll(), std::task::Poll::Ready(Ok(())));
    assert!(actor.snapshot().epoch > first_epoch);
    assert_eq!(actor.snapshot().availability, Availability::Suspended);
}

#[test]
fn quiescence_timeout_reports_uncertain_and_never_reopens_admission() {
    let (mut worker, gate) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let lease = connect(&mut actor, 1);
    claim(&mut actor, &lease, 1);
    batch(&mut actor, &lease, 2, [button(1)]).unwrap();
    run_until(&mut actor, NOW, |actor| {
        actor.snapshot().pending_actions == 0
    });
    gate.arm(Block::Release);
    let ticket = actor
        .safety_signal()
        .request_quiescence(QuiescenceMode::RetireTransport)
        .unwrap();
    actor.poll(NOW);
    gate.wait(Block::Release);
    assert!(lease.is_closed());
    assert_eq!(ticket.poll(), std::task::Poll::Pending);
    actor.poll(RELEASE_TIMEOUT);
    assert_eq!(
        ticket.poll(),
        std::task::Poll::Ready(Err(QuiescenceError::NotNeutral))
    );
    gate.resume();
    assert_eq!(actor.snapshot().availability, Availability::Faulted);
    assert!(!actor.snapshot().confirmed_neutral);
}

#[test]
fn a_shutdown_upgrade_waits_for_stopped_and_drop_rejects_unobserved_tickets() {
    let (mut worker, gate) = dispatcher(4, MockInputBackend::new());
    let mut actor = start(&mut worker);
    let signal = actor.safety_signal();
    gate.arm(Block::Release);
    let retire = signal
        .request_quiescence(QuiescenceMode::RetireTransport)
        .unwrap();
    actor.poll(NOW);
    gate.wait(Block::Release);
    let stop = signal.request_quiescence(QuiescenceMode::Shutdown).unwrap();
    actor.poll(NOW);
    assert_eq!(retire.poll(), std::task::Poll::Pending);
    gate.resume();
    run_until(&mut actor, NOW, |_| stop.poll().is_ready());
    assert_eq!(actor.snapshot().availability, Availability::Stopped);
    assert_eq!(
        retire.poll(),
        std::task::Poll::Ready(Err(QuiescenceError::Unavailable))
    );
    assert_eq!(stop.poll(), std::task::Poll::Ready(Ok(())));
    assert!(matches!(
        signal.request_quiescence(QuiescenceMode::Shutdown),
        Err(QuiescenceError::Unavailable)
    ));

    drop(actor);
    let (mut another, _) = dispatcher(4, MockInputBackend::new());
    let actor = start(&mut another);
    let pending = actor
        .safety_signal()
        .request_quiescence(QuiescenceMode::RetireTransport)
        .unwrap();
    drop(actor);
    assert_eq!(
        pending.poll(),
        std::task::Poll::Ready(Err(QuiescenceError::Unavailable))
    );
}
