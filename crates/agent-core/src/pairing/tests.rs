use super::*;
use prost::Message;
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, Ordering};
use tether_input_core::{
    BackendCapabilities, BackendKind, BackendStatus, DispatcherConfig, InputDispatcher,
    MockInputBackend, MouseButton, PermissionState,
};
use tether_protocol::{validation::Limits, wire};
use tether_session::actor::{Availability, SessionActor, SessionError};
use tether_test_support::PrivateTempDir;

mod http;

const GENERATION: NonZeroU64 = NonZeroU64::MIN;
const PEER: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 20));
const ZERO: Duration = Duration::ZERO;

struct TestTransport {
    generation: NonZeroU64,
    active: AtomicBool,
}

impl PairingTransport for TestTransport {
    fn generation(&self) -> NonZeroU64 {
        self.generation
    }
    fn is_current(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
}

struct Fixture {
    store: SettingsStore,
    coordinator: PairingCoordinator,
    transport: TestTransport,
    host: HostScope,
    initial: PairingTime,
    _temp: PrivateTempDir,
}

impl Fixture {
    fn new(safety: SafetySignal) -> Self {
        Self::in_generation(safety, GENERATION)
    }

    fn in_generation(safety: SafetySignal, generation: NonZeroU64) -> Self {
        let temp = PrivateTempDir::new().unwrap();
        let (store, _) = SettingsStore::open(&temp.path().join("settings")).unwrap();
        let mut coordinator = PairingCoordinator::new(safety);
        coordinator.activate_transport(generation).unwrap();
        Self {
            store,
            coordinator,
            transport: TestTransport {
                generation,
                active: AtomicBool::new(true),
            },
            host: HostScope::ip(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 18))),
            initial: PairingTime {
                monotonic: Instant::now(),
                wall: UNIX_EPOCH + Duration::from_secs(1_700_000_000),
            },
            _temp: temp,
        }
    }

    fn submit(&mut self) -> (ApprovalId, String) {
        let now = self.initial.monotonic;
        let qr = self
            .coordinator
            .issue_qr(
                &self.store,
                self.transport.generation,
                now,
                self.host.clone(),
            )
            .unwrap();
        let peer = PairingPeer {
            transport: &self.transport,
            host: &self.host,
            address: PEER,
        };
        let label = DeviceLabel::new("Phone <label>").unwrap();
        let completion = self
            .coordinator
            .submit(
                &self.store,
                &peer,
                now,
                &qr.encode_for_fragment(),
                label.clone(),
            )
            .unwrap();
        assert!(
            self.coordinator
                .submit(&self.store, &peer, now, &qr.encode_for_fragment(), label)
                .is_err()
        );
        let approval = self
            .coordinator
            .approvals(self.transport.generation, now)
            .unwrap()
            .last()
            .unwrap();
        assert_eq!(approval.peer(), PEER);
        assert_eq!(approval.label().for_display(), "Phone <label>");
        assert_eq!(approval.host(), &self.host);
        (approval.id(), completion.encode_for_response_body())
    }

    fn approve(&mut self, id: ApprovalId) {
        self.coordinator
            .decide(
                self.transport.generation,
                self.initial.monotonic,
                id,
                ApprovalDecision::Approve,
            )
            .unwrap();
    }

    fn publish(&mut self, completion: &str, clock: PairingTime) -> DeviceCookie {
        let peer = PairingPeer {
            transport: &self.transport,
            host: &self.host,
            address: PEER,
        };
        let mut emitted = None;
        let result = self.coordinator.complete(
            &mut self.store,
            &peer,
            completion,
            || clock,
            |cookie| {
                emitted = Some(cookie);
                true
            },
        );
        assert_eq!(result.unwrap(), Completion::Published);
        emitted.unwrap()
    }

    fn verified(&mut self, cookie: &DeviceCookie) -> Option<VerifiedConnection<DeviceId>> {
        let cookie = cookie
            .as_set_cookie()
            .split(';')
            .next()
            .unwrap()
            .strip_prefix("tether_device=")
            .unwrap();
        let peer = PairingPeer {
            transport: &self.transport,
            host: &self.host,
            address: PEER,
        };
        self.coordinator
            .verify_control(&self.store, &peer, cookie, self.initial.wall, ZERO)
    }

    fn count(&self) -> usize {
        self.store.devices_snapshot().unwrap().devices().len()
    }
}

#[test]
fn pending_poll_requires_local_approval_then_publishes_one_durable_host_scoped_cookie() {
    let mut fixture = Fixture::new(SafetySignal::default());
    let (id, completion) = fixture.submit();
    let peer = PairingPeer {
        transport: &fixture.transport,
        host: &fixture.host,
        address: PEER,
    };
    let pending = fixture.coordinator.complete(
        &mut fixture.store,
        &peer,
        &completion,
        || fixture.initial,
        |_| panic!("unapproved cookie"),
    );
    assert_eq!(pending.unwrap(), Completion::Pending);
    fixture.approve(id);
    let delta = Duration::from_secs(2);
    let clock = PairingTime {
        monotonic: fixture.initial.monotonic + delta,
        wall: fixture.initial.wall + delta,
    };
    let cookie = fixture.publish(&completion, clock);
    fixture.initial = clock;
    assert_eq!(fixture.count(), 1);
    assert!(fixture.verified(&cookie).is_some());
    let peer = PairingPeer {
        transport: &fixture.transport,
        host: &fixture.host,
        address: PEER,
    };
    assert!(
        fixture
            .coordinator
            .complete(&mut fixture.store, &peer, &completion, || clock, |_| false)
            .is_err()
    );
    fixture.store.reload().unwrap();
    assert!(fixture.verified(&cookie).is_some());
    let wrong_host = HostScope::local("tether-0123456789ab.local").unwrap();
    let peer = PairingPeer {
        host: &wrong_host,
        transport: &fixture.transport,
        address: PEER,
    };
    let value = cookie
        .as_set_cookie()
        .split(';')
        .next()
        .unwrap()
        .strip_prefix("tether_device=")
        .unwrap();
    assert!(
        fixture
            .coordinator
            .verify_control(&fixture.store, &peer, value, clock.wall, ZERO)
            .is_none()
    );
}

#[test]
fn denied_expired_and_old_generation_approvals_cannot_publish() {
    let mut fixture = Fixture::new(SafetySignal::default());
    let (denied, completion) = fixture.submit();
    fixture
        .coordinator
        .decide(
            GENERATION,
            fixture.initial.monotonic,
            denied,
            ApprovalDecision::Deny,
        )
        .unwrap();
    let peer = PairingPeer {
        transport: &fixture.transport,
        host: &fixture.host,
        address: PEER,
    };
    assert!(
        fixture
            .coordinator
            .complete(
                &mut fixture.store,
                &peer,
                &completion,
                || fixture.initial,
                |_| false
            )
            .is_err()
    );
    let (expired, _) = fixture.submit();
    let late = fixture.initial.monotonic + Duration::from_secs(120);
    assert!(
        fixture
            .coordinator
            .decide(GENERATION, late, expired, ApprovalDecision::Approve)
            .is_err()
    );
    fixture.coordinator.retire_transport(GENERATION);
    let next = NonZeroU64::new(2).unwrap();
    fixture.coordinator.activate_transport(next).unwrap();
    assert!(
        fixture
            .coordinator
            .decide(GENERATION, late, expired, ApprovalDecision::Approve)
            .is_err()
    );
    assert!(
        fixture
            .coordinator
            .decide(next, late, expired, ApprovalDecision::Approve)
            .is_err()
    );
    assert_eq!(fixture.count(), 0);
}

#[test]
fn cancellation_or_retirement_between_durable_commit_and_response_leaves_no_device() {
    for reason in [
        "output closed",
        "transport retired",
        "approval expired",
        "clock regressed",
    ] {
        let mut fixture = Fixture::new(SafetySignal::default());
        let (id, completion) = fixture.submit();
        fixture.approve(id);
        let peer = PairingPeer {
            transport: &fixture.transport,
            host: &fixture.host,
            address: PEER,
        };
        let mut readings = 0;
        let mut offered = false;
        let result = fixture.coordinator.complete(
            &mut fixture.store,
            &peer,
            &completion,
            || {
                readings += 1;
                if readings == 3 {
                    match reason {
                        "transport retired" => {
                            fixture.transport.active.store(false, Ordering::Release);
                        }
                        "approval expired" => {
                            return PairingTime {
                                monotonic: fixture.initial.monotonic + Duration::from_secs(120),
                                ..fixture.initial
                            };
                        }
                        "clock regressed" => {
                            return PairingTime {
                                wall: UNIX_EPOCH,
                                ..fixture.initial
                            };
                        }
                        _ => {}
                    }
                }
                fixture.initial
            },
            |_| {
                offered = true;
                false
            },
        );
        assert!(result.is_err());
        assert_eq!(offered, reason == "output closed");
        assert_eq!(fixture.count(), 0, "{reason}: {result:?}");
        fixture.store.reload().unwrap();
        assert_eq!(fixture.count(), 0);
    }
}

fn envelope(sequence: u64, epoch: u64, payload: wire::client_envelope::Payload) -> Vec<u8> {
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

fn hello() -> Vec<u8> {
    envelope(
        0,
        0,
        wire::client_envelope::Payload::ClientHello(wire::ClientHello {
            protocol_major: 1,
            build_identifier: "test".into(),
            client_display_name: "Phone".into(),
            limits: Some(wire::ClientLimits {
                maximum_encoded_message_bytes: 65_536,
                maximum_operations_per_batch: 256,
                maximum_text_operation_bytes: 4096,
            }),
            capabilities: vec![1, 2],
            ..Default::default()
        }),
    )
}

fn action(sequence: u64, epoch: u64, value: wire::client_operation::Action) -> Vec<u8> {
    envelope(
        sequence,
        epoch,
        wire::client_envelope::Payload::OperationBatch(wire::ClientOperationBatch {
            operations: vec![wire::ClientOperation {
                timestamp_offset_micros: 0,
                action: Some(value),
            }],
        }),
    )
}

type TestActor<'a> = SessionActor<'a, MockInputBackend, DeviceId>;

fn run_until(actor: &mut TestActor<'_>, ready: impl Fn(&TestActor<'_>) -> bool) {
    let end = Instant::now() + Duration::from_secs(3);
    loop {
        actor.poll(ZERO);
        if ready(actor) {
            return;
        }
        assert!(Instant::now() < end, "actor state: {:?}", actor.snapshot());
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn stored_cookie_controls_the_mock_then_revocation_invalidates_active_and_queued_authority() {
    let mut dispatcher =
        InputDispatcher::start(MockInputBackend::new(), DispatcherConfig::default()).unwrap();
    let mut capabilities = BackendCapabilities::unavailable();
    capabilities.mouse_buttons.insert(MouseButton::Left);
    let mut actor = SessionActor::new(
        &mut dispatcher,
        TransportGeneration(GENERATION),
        BackendStatus {
            revision: 1,
            kind: BackendKind::Mock,
            initialized: true,
            available: true,
            permission: PermissionState::NotRequired,
            capabilities,
            fault: None,
        },
        Limits::HARD,
        ZERO,
    );
    run_until(&mut actor, |actor| {
        actor.snapshot().availability == Availability::Ready
    });
    let mut fixture = Fixture::new(actor.safety_signal());
    let (id, completion) = fixture.submit();
    fixture.approve(id);
    let cookie = fixture.publish(&completion, fixture.initial);
    let proof = fixture.verified(&cookie).unwrap();
    let stale_proof = fixture.verified(&cookie).unwrap();
    let device = proof.device;
    let lease = actor.register(ZERO, proof).unwrap();
    actor.receive(ZERO, &lease, &hello()).unwrap();
    actor
        .receive(
            ZERO,
            &lease,
            &action(
                1,
                actor.snapshot().epoch,
                wire::client_operation::Action::RequestControl(wire::RequestControl {}),
            ),
        )
        .unwrap();
    run_until(&mut actor, |actor| actor.snapshot().has_owner);
    actor
        .receive(
            ZERO,
            &lease,
            &action(
                2,
                actor.snapshot().epoch,
                wire::client_operation::Action::MouseButtonTransition(
                    wire::MouseButtonTransition {
                        button: 1,
                        state: 1,
                    },
                ),
            ),
        )
        .unwrap();
    run_until(&mut actor, |actor| actor.snapshot().pending_actions == 0);
    let revision = ready_revision(&fixture.store).unwrap();
    fixture
        .coordinator
        .revoke(
            &mut fixture.store,
            Some(revision),
            DeviceRevocation::Device(device),
        )
        .unwrap();
    assert!(fixture.verified(&cookie).is_none());
    assert_eq!(
        actor.register(ZERO, stale_proof).unwrap_err(),
        SessionError::StaleAuthority
    );
    run_until(&mut actor, |actor| actor.snapshot().confirmed_neutral);
    assert!(lease.is_closed());
    assert_eq!(fixture.count(), 0);
    drop(actor);
    let stopped = dispatcher.shutdown();
    assert!(stopped.backend.is_neutral());
    assert_eq!(stopped.backend.calls().len(), 1);
}

#[test]
fn revoke_all_clears_waiting_approvals_and_all_verified_proofs() {
    let mut fixture = Fixture::new(SafetySignal::default());
    let (id, completion) = fixture.submit();
    fixture.approve(id);
    let cookie = fixture.publish(&completion, fixture.initial);
    let stale = fixture.verified(&cookie).unwrap();
    let (waiting, _) = fixture.submit();
    let revision = ready_revision(&fixture.store).unwrap();
    fixture
        .coordinator
        .revoke(&mut fixture.store, Some(revision), DeviceRevocation::All)
        .unwrap();
    assert!(!stale.credential.is_current());
    assert!(
        fixture
            .coordinator
            .decide(
                GENERATION,
                fixture.initial.monotonic,
                waiting,
                ApprovalDecision::Approve
            )
            .is_err()
    );
    assert!(fixture.verified(&cookie).is_none());
    assert_eq!(fixture.count(), 0);
}
