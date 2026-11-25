use super::*;
use crate::listener_safety::ListenerSessionSafety;
use futures_util::future::join;
use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use tether_config::ApprovedInterfaceId;
use tether_network::assets::EmbeddedAssets;
use tether_network::http1::{
    PairingHttpService, PairingOperation, PairingRejection, PairingReply, PairingRequest,
    StaticHttpServer,
};
use tether_network::interfaces::{
    AddressState, ApprovedSelection, AssignedAddress, BindingPlan, IdentityPersistence, Interface,
    InterfaceError, InterfaceIndexes, InterfaceInventory, InterfaceKind, InterfaceStatus,
    LinkState, NetworkProfile,
};
use tether_network::supervisor::{
    AcceptResult, AcceptedTransport, ConnectionAuthority, ListenerDriver, ListenerStatus,
    ListenerSupervisor, NeutralityTicket, ReleaseCause, SafetyCoordinator, SafetyError,
};
use tether_session::actor::ConnectionLease;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const ENDPOINT: &str = "192.0.2.18:54321";
const WAIT: Duration = Duration::from_secs(5);
const CONTROL_OPEN: &str = "Connection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: tether.control.v1\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==";

#[derive(Default)]
struct FixtureDriver(InterfaceInventory);

impl ListenerDriver for FixtureDriver {
    type Set = ();
    type Transport = ();
    type Shutdown = ();
    type Error = ();

    fn inventory(&self) -> &InterfaceInventory {
        &self.0
    }
    fn begin_refresh(&mut self, _: Instant) -> Result<(), InterfaceError> {
        Ok(())
    }
    fn cancel_refresh(&mut self) {}
    fn poll_refresh(&mut self, _: Instant) -> Poll<Result<(), InterfaceError>> {
        let adapter = Interface::new(
            ApprovedInterfaceId::new("pairing-fixture".into()).unwrap(),
            "Fixture adapter",
            IdentityPersistence::Stable,
            InterfaceIndexes {
                ipv4: NonZeroU32::new(1),
                ipv6: None,
            },
            InterfaceStatus {
                kind: InterfaceKind::Ethernet,
                link: LinkState::Up,
                profile: NetworkProfile::Private,
            },
            vec![
                AssignedAddress::new(
                    Ipv4Addr::new(192, 0, 2, 18).into(),
                    24,
                    AddressState::Preferred,
                )
                .unwrap(),
            ],
        )
        .unwrap();
        Poll::Ready(self.0.refresh(Ok(vec![adapter])).map(|_| ()))
    }
    fn bind(&mut self, plan: BindingPlan, generation: NonZeroU64) -> Result<(), ()> {
        assert!(plan.is_current(&self.0, generation));
        Ok(())
    }
    fn accept(&mut self, (): &mut (), _: NonZeroU64) -> AcceptResult<Self> {
        Ok(Some(AcceptedTransport {
            transport: (),
            shutdown: (),
        }))
    }
    fn close((): &()) -> Result<(), ()> {
        Ok(())
    }
}

struct NeutralFixture;

impl SafetyCoordinator for NeutralFixture {
    type Ticket = Self;
    fn request_release(
        &mut self,
        _: ReleaseCause,
        _: Option<NonZeroU64>,
    ) -> Result<Self, SafetyError> {
        Ok(Self)
    }
}

impl NeutralityTicket for NeutralFixture {
    fn poll_neutrality(&mut self) -> Poll<Result<(), SafetyError>> {
        Poll::Ready(Ok(()))
    }
}

struct Harness {
    server: Arc<StaticHttpServer<PairingHttpBridge>>,
    inbox: PairingInbox,
    state: PairingCoordinator,
    supervisor: ListenerSupervisor<FixtureDriver, NeutralFixture>,
    generation: NonZeroU64,
    store: SettingsStore,
    _temp: PrivateTempDir,
}

fn fixture_selection<S: SafetyCoordinator>(
    listener: &ListenerSupervisor<FixtureDriver, S>,
) -> ApprovedSelection {
    let snapshot = listener.inventory().current().unwrap();
    snapshot
        .approve(
            snapshot.revision(),
            &[snapshot.records()[0].id().clone()],
            NonZeroU16::new(54_321).unwrap(),
        )
        .unwrap()
}

fn reach_listening<S: SafetyCoordinator>(
    listener: &mut ListenerSupervisor<FixtureDriver, S>,
    now: Instant,
) -> NonZeroU64 {
    for _ in 0..3 {
        listener.poll_transition(now).unwrap();
    }
    let ListenerStatus::Listening { generation } = listener.status() else {
        panic!("fixture stops before listening");
    };
    generation
}

impl Harness {
    fn start() -> Self {
        let temp = PrivateTempDir::new().unwrap();
        let (store, _) = SettingsStore::open(&temp.path().join("settings")).unwrap();
        let mut supervisor = ListenerSupervisor::new(FixtureDriver::default(), NeutralFixture);
        let now = Instant::now();
        supervisor.request_inventory(now).unwrap();
        supervisor.poll_transition(now).unwrap();
        supervisor
            .start(fixture_selection(&supervisor), now)
            .unwrap();
        let generation = reach_listening(&mut supervisor, now);
        let mut state = PairingCoordinator::new(SafetySignal::default());
        state.activate_transport(generation).unwrap();
        let (bridge, inbox) = pairing_channel(now);
        let assets = EmbeddedAssets::new(b"<title>Pairing fixture</title>", &[]).unwrap();
        let server = Arc::new(StaticHttpServer::with_pairing(assets, bridge));
        Self {
            server,
            inbox,
            state,
            supervisor,
            generation,
            store,
            _temp: temp,
        }
    }

    fn accept(&mut self) -> ConnectionAuthority<()> {
        let ((), authority) = (0..4)
            .find_map(|_| {
                self.supervisor.poll_transition(Instant::now()).unwrap();
                self.supervisor.poll_accept(Instant::now()).unwrap()
            })
            .unwrap();
        authority
    }

    fn enable_control(&mut self, origin: Instant) -> crate::control::ControlInbox {
        let (bridge, inbox, control) = pairing_channel_with_control(origin);
        self.inbox = inbox;
        self.server = Arc::new(StaticHttpServer::with_pairing(
            EmbeddedAssets::new(b"<title>Joined fixture</title>", &[]).unwrap(),
            bridge,
        ));
        control
    }

    fn control_socket(
        &mut self,
    ) -> (
        tokio::io::DuplexStream,
        tokio::task::JoinHandle<Result<(), tether_network::http1::ServeError>>,
    ) {
        let authority = self.accept();
        let server = Arc::clone(&self.server);
        let (client, transport) = tokio::io::duplex(8192);
        let socket = tokio::spawn(async move {
            server
                .serve_admitted(transport, ENDPOINT.parse().unwrap(), PEER, authority, None)
                .await
        });
        (client, socket)
    }

    async fn post(&mut self, path: &str, json: &str) -> String {
        let authority = self.accept();
        let (mut client, server_io) = tokio::io::duplex(8192);
        let server = Arc::clone(&self.server);
        let task = tokio::spawn(async move {
            server
                .serve_admitted(server_io, ENDPOINT.parse().unwrap(), PEER, authority, None)
                .await
        });
        client.write_all(format!("POST /pairing/{path} HTTP/1.1\r\nHost: {ENDPOINT}\r\nOrigin: http://{ENDPOINT}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n", json.len()).as_bytes()).await.unwrap();
        client.write_all(json.as_bytes()).await.unwrap();
        tokio::time::timeout(WAIT, async {
            while !self.inbox.process_one(&mut self.state, &mut self.store) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let mut response = String::new();
        tokio::time::timeout(WAIT, client.read_to_string(&mut response))
            .await
            .unwrap()
            .unwrap();
        task.await.unwrap().unwrap();
        response
    }

    async fn control(&mut self, cookie: &str) -> String {
        let authority = self.accept();
        let (mut client, server_io) = tokio::io::duplex(8192);
        let server = Arc::clone(&self.server);
        let task = tokio::spawn(async move {
            server
                .serve_admitted(server_io, ENDPOINT.parse().unwrap(), PEER, authority, None)
                .await
        });
        client
            .write_all(control_open(cookie).as_bytes())
            .await
            .unwrap();
        tokio::time::timeout(WAIT, async {
            while !self
                .inbox
                .process_control_one(&mut self.state, &mut self.store)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let mut response = String::new();
        tokio::time::timeout(WAIT, client.read_to_string(&mut response))
            .await
            .unwrap()
            .unwrap();
        task.await.unwrap().unwrap();
        response
    }
}

fn control_open(cookie: &str) -> String {
    format!(
        "GET /control HTTP/1.1\r\nHost: {ENDPOINT}\r\nOrigin: http://{ENDPOINT}\r\n{CONTROL_OPEN}\r\nCookie: tether_device={cookie}\r\n\r\n"
    )
}

async fn http_head(stream: &mut tokio::io::DuplexStream) -> String {
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") && head.len() < 16 * 1024 {
        head.push(stream.read_u8().await.unwrap());
    }
    String::from_utf8(head).unwrap()
}

async fn send_message(stream: &mut tokio::io::DuplexStream, payload: &[u8]) {
    let length = u8::try_from(payload.len()).unwrap();
    assert!(length < 126);
    let mask = [1, 2, 3, 4];
    let mut frame = vec![0x82, 0x80 | length];
    frame.extend_from_slice(&mask);
    frame.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    stream.write_all(&frame).await.unwrap();
}

async fn server_message(stream: &mut tokio::io::DuplexStream) -> wire::ServerEnvelope {
    assert_eq!(stream.read_u8().await.unwrap(), 0x82);
    let length = match stream.read_u8().await.unwrap() {
        126 => usize::from(stream.read_u16().await.unwrap()),
        length if length < 126 => usize::from(length),
        _ => panic!("invalid test frame length"),
    };
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes).await.unwrap();
    wire::ServerEnvelope::decode(bytes.as_slice()).unwrap()
}

async fn approve_phone(fixture: &mut Harness) -> String {
    let token = fixture
        .state
        .issue_qr(
            &fixture.store,
            fixture.generation,
            Instant::now(),
            HostScope::ip(Ipv4Addr::new(192, 0, 2, 18).into()),
        )
        .unwrap();
    let request = format!(
        r#"{{"token":"{}","deviceName":"Phone"}}"#,
        token.encode_for_fragment()
    );
    let first = fixture.post("request", &request).await;
    assert!(first.starts_with("HTTP/1.1 202 "));
    let proof = first.split_once("\r\n\r\n").unwrap().1;
    let pending = fixture
        .state
        .approvals(fixture.generation, Instant::now())
        .unwrap()
        .next()
        .unwrap()
        .id();
    fixture
        .state
        .decide(
            fixture.generation,
            Instant::now(),
            pending,
            ApprovalDecision::Approve,
        )
        .unwrap();
    let complete = fixture.post("complete", proof).await;
    assert!(complete.starts_with("HTTP/1.1 200 "));
    complete
        .lines()
        .find_map(|line| {
            line.strip_prefix("set-cookie: tether_device=")?
                .split_once(';')
                .map(|(value, _)| value.to_owned())
        })
        .unwrap()
}

#[test]
fn paired_http_cookie_reaches_the_mock_input_actor_and_disconnect_releases() {
    let mut fixture = Harness::start();
    let (mut dispatcher, backend) = dispatcher_for_listener();
    let mut control = fixture.enable_control(Instant::now());
    let mut actor = SessionActor::new(
        &mut dispatcher,
        TransportGeneration(fixture.generation),
        backend,
        Limits::HARD,
        ZERO,
    );
    run_until(&mut actor, |actor| actor.snapshot().confirmed_neutral);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let cookie = approve_phone(&mut fixture).await;
        let (client, socket) = fixture.control_socket();
        let (duplicate, second) = fixture.control_socket();
        let browser = browser_exchange(client, duplicate, cookie);
        let driver = async {
            while !socket.is_finished() {
                fixture
                    .inbox
                    .process_control_one(&mut fixture.state, &mut fixture.store);
                control.drive(&mut actor);
                control.wait_for_activity(Duration::from_millis(16)).await;
            }
        };
        let drive_until_browser = async {
            let mut driver = std::pin::pin!(driver);
            match futures_util::future::select(std::pin::pin!(browser), &mut driver).await {
                futures_util::future::Either::Left((client, _)) => client,
                futures_util::future::Either::Right(_) => {
                    panic!("connection closed before exchange")
                }
            }
        };
        let client = tokio::time::timeout(WAIT, drive_until_browser)
            .await
            .unwrap();
        assert_eq!(second.await.unwrap(), Ok(()));
        assert!(actor.snapshot().has_owner);
        tokio::time::timeout(WAIT, async {
            while actor.snapshot().pending_actions > 0 {
                control.drive(&mut actor);
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        drop(client);
        let result = tokio::time::timeout(
            WAIT,
            join(socket, async {
                while actor.snapshot().connections > 0 || !actor.snapshot().confirmed_neutral {
                    control.drive(&mut actor);
                    tokio::task::yield_now().await;
                }
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(result.unwrap(), Ok(()));
        assert!(!actor.snapshot().has_owner);
    });
    drop(actor);
    let result = dispatcher.shutdown();
    assert!(result.backend.is_neutral());
    assert_eq!(result.backend.calls().len(), 1);
    assert_eq!(
        result.backend.calls()[0].operation,
        tether_input_core::OperationClass::MouseButton
    );
}

#[test]
fn incompatible_and_malformed_hello_close_without_input_authority() {
    let mut fixture = Harness::start();
    let (mut dispatcher, backend) = dispatcher_for_listener();
    let mut control = fixture.enable_control(Instant::now());
    let mut actor = SessionActor::new(
        &mut dispatcher,
        TransportGeneration(fixture.generation),
        backend,
        Limits::HARD,
        ZERO,
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let cookie = approve_phone(&mut fixture).await;
        let mut incompatible = wire::ClientEnvelope::decode(hello().as_slice()).unwrap();
        incompatible.protocol_major = 2;
        for (request, code) in [(incompatible.encode_to_vec(), 4002), (vec![0x80], 1008)] {
            let (mut client, server) = fixture.control_socket();
            let exchange = async {
                client
                    .write_all(control_open(&cookie).as_bytes())
                    .await
                    .unwrap();
                assert!(http_head(&mut client).await.starts_with("HTTP/1.1 101 "));
                send_message(&mut client, &request).await;
                assert_eq!(client.read_u8().await.unwrap(), 0x88);
                assert_eq!(client.read_u8().await.unwrap(), 2);
                assert_eq!(client.read_u16().await.unwrap(), code);
            };
            let drive = async {
                while !server.is_finished() {
                    fixture
                        .inbox
                        .process_control_one(&mut fixture.state, &mut fixture.store);
                    control.drive(&mut actor);
                    tokio::task::yield_now().await;
                }
            };
            tokio::time::timeout(WAIT, join(exchange, drive))
                .await
                .unwrap();
            assert_eq!(server.await.unwrap(), Ok(()));
            assert_eq!(actor.snapshot().connections, 0);
            assert!(!actor.snapshot().has_owner);
        }
    });
    drop(actor);
    assert_eq!(dispatcher.shutdown().backend.calls().len(), 0);
}

async fn browser_exchange(
    mut client: tokio::io::DuplexStream,
    mut duplicate: tokio::io::DuplexStream,
    cookie: String,
) -> tokio::io::DuplexStream {
    use wire::server_envelope::Payload;

    client
        .write_all(control_open(&cookie).as_bytes())
        .await
        .unwrap();
    let upgraded = http_head(&mut client).await;
    assert!(upgraded.starts_with("HTTP/1.1 101 "));
    assert!(upgraded.lines().any(|line| {
        line.strip_prefix("set-cookie: tether_device=")
            .and_then(|value| value.split_once(';'))
            .is_some_and(|(value, attrs)| value == cookie && attrs.contains("Max-Age="))
    }));
    duplicate
        .write_all(control_open(&cookie).as_bytes())
        .await
        .unwrap();
    let rejected = http_head(&mut duplicate).await;
    assert!(rejected.starts_with("HTTP/1.1 429 "));
    assert!(!rejected.contains("set-cookie"));
    send_message(&mut client, &hello()).await;
    let welcome = server_message(&mut client).await;
    let Some(Payload::ServerHello(welcome)) = welcome.payload else {
        panic!("expected server hello");
    };
    let before = welcome.control_state.unwrap();
    send_message(
        &mut client,
        &action(
            1,
            before.session_epoch,
            wire::client_operation::Action::RequestControl(wire::RequestControl {}),
        ),
    )
    .await;
    let update = server_message(&mut client).await;
    let Some(Payload::ControlState(grant)) = update.payload else {
        panic!("expected ownership update");
    };
    assert!(grant.this_client_controls);
    assert!(grant.session_epoch > before.session_epoch);
    send_message(
        &mut client,
        &action(
            2,
            grant.session_epoch,
            wire::client_operation::Action::MouseButtonTransition(wire::MouseButtonTransition {
                button: 1,
                state: 1,
            }),
        ),
    )
    .await;
    let heartbeat = wire::ClientEnvelope {
        protocol_major: 1,
        sequence: 3,
        session_epoch: grant.session_epoch,
        client_timestamp_micros: 3,
        payload: Some(wire::client_envelope::Payload::HeartbeatPing(
            wire::HeartbeatPing {
                heartbeat_identifier: 19,
            },
        )),
        ..Default::default()
    };
    send_message(&mut client, &heartbeat.encode_to_vec()).await;
    let pong = server_message(&mut client).await;
    assert!(
        matches!(pong.payload, Some(Payload::HeartbeatPong(pong)) if pong.heartbeat_identifier == 19)
    );
    client
}

#[test]
fn admitted_http_requests_reach_the_production_approval_and_cookie_bridge() {
    let mut fixture = Harness::start();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let host = HostScope::ip(Ipv4Addr::new(192, 0, 2, 18).into());
        let token = fixture
            .state
            .issue_qr(
                &fixture.store,
                fixture.generation,
                Instant::now(),
                host.clone(),
            )
            .unwrap();
        let request = format!(
            r#"{{"token":"{}","deviceName":"Phone"}}"#,
            token.encode_for_fragment()
        );
        let response = fixture.post("request", &request).await;
        assert!(response.starts_with("HTTP/1.1 202 "));
        let body = response.split_once("\r\n\r\n").unwrap().1.to_owned();
        assert_eq!(body.len(), r#"{"completion":""}"#.len() + 64);
        assert!(body.starts_with(r#"{"completion":""#.trim_end_matches('"')));
        assert!(!response.contains("set-cookie"));
        let pending = fixture
            .state
            .approvals(fixture.generation, Instant::now())
            .unwrap()
            .next()
            .unwrap()
            .id();
        fixture
            .state
            .decide(
                fixture.generation,
                Instant::now(),
                pending,
                ApprovalDecision::Approve,
            )
            .unwrap();
        let response = fixture.post("complete", &body).await;
        assert!(response.starts_with("HTTP/1.1 200 "));
        assert!(response.ends_with("\r\n\r\n"));
        let cookie = response
            .lines()
            .find_map(|line| line.strip_prefix("set-cookie: tether_device="))
            .unwrap();
        let (value, flags) = cookie.split_once(';').unwrap();
        assert_eq!(value.len(), 64);
        assert!(flags.starts_with(" HttpOnly; SameSite=Strict; Path=/control; Max-Age="));
        assert_eq!(
            DeviceCookie::clear().as_set_cookie(),
            "tether_device=; HttpOnly; SameSite=Strict; Path=/control; Max-Age=0"
        );
        assert!(
            fixture
                .store
                .verify_device_cookie(&host, value, SystemTime::now())
                .is_some()
        );
        let repeated = fixture.post("complete", &body).await;
        assert!(repeated.starts_with("HTTP/1.1 410 "));
        assert!(!repeated.contains("set-cookie"));
        assert!(fixture.control(value).await.starts_with("HTTP/1.1 503 "));
        let invalid = fixture.control(&"0".repeat(64)).await;
        assert!(invalid.starts_with("HTTP/1.1 401 "));
        let device = fixture
            .store
            .verify_device_cookie(&host, value, SystemTime::now())
            .unwrap();
        fixture
            .state
            .revoke(
                &mut fixture.store,
                Some(device.revision()),
                DeviceRevocation::Device(device.id()),
            )
            .unwrap();
        let revoked = fixture.control(value).await;
        assert!(revoked.starts_with("HTTP/1.1 401 "));
        assert!(revoked.ends_with("\r\n\r\n"));
        assert!(invalid.ends_with("\r\n\r\n"));
    });
}

#[test]
fn a_full_bridge_refuses_extra_work_and_cancelled_requests_do_not_spend_the_qr() {
    let mut fixture = Harness::start();
    let (bridge, inbox) = pairing_channel(Instant::now());
    let admission = fixture.accept().admission();
    let host = HostScope::ip(Ipv4Addr::new(192, 0, 2, 18).into());
    let token = fixture
        .state
        .issue_qr(
            &fixture.store,
            fixture.generation,
            Instant::now(),
            host.clone(),
        )
        .unwrap();
    let make_request = || PairingRequest {
        authority: admission.clone(),
        host: host.clone(),
        peer: PEER,
        operation: PairingOperation::Submit {
            token: token.encode_for_fragment(),
            label: DeviceLabel::new("Phone").unwrap(),
        },
    };
    let mut context = Context::from_waker(Waker::noop());
    let mut pending: Vec<_> = (0..8)
        .map(|_| Box::pin(bridge.handle(make_request())))
        .collect();
    for request in &mut pending {
        assert!(request.as_mut().poll(&mut context).is_pending());
    }
    let mut extra = std::pin::pin!(bridge.handle(make_request()));
    assert!(matches!(
        extra.as_mut().poll(&mut context),
        Poll::Ready(PairingReply::Rejected(PairingRejection::RateLimited))
    ));
    drop(pending);
    for _ in 0..8 {
        assert!(inbox.process_one(&mut fixture.state, &mut fixture.store));
    }
    assert!(!inbox.process_one(&mut fixture.state, &mut fixture.store));
    assert_eq!(
        fixture
            .state
            .approvals(fixture.generation, Instant::now())
            .unwrap()
            .len(),
        0
    );
    let mut live = std::pin::pin!(bridge.handle(make_request()));
    assert!(live.as_mut().poll(&mut context).is_pending());
    assert!(inbox.process_one(&mut fixture.state, &mut fixture.store));
    assert!(matches!(
        live.as_mut().poll(&mut context),
        Poll::Ready(PairingReply::Submitted(_))
    ));
}

fn dispatcher_for_listener() -> (InputDispatcher<MockInputBackend>, BackendStatus) {
    let dispatcher =
        InputDispatcher::start(MockInputBackend::new(), DispatcherConfig::default()).unwrap();
    let mut capabilities = BackendCapabilities::unavailable();
    capabilities.mouse_buttons.insert(MouseButton::Left);
    (
        dispatcher,
        BackendStatus {
            revision: 1,
            kind: BackendKind::Mock,
            initialized: true,
            available: true,
            permission: PermissionState::NotRequired,
            capabilities,
            fault: None,
        },
    )
}

fn paired_owner_holding_button(
    actor: &mut TestActor<'_>,
    generation: NonZeroU64,
) -> (Fixture, ConnectionLease) {
    let mut fixture = Fixture::in_generation(actor.safety_signal(), generation);
    let (id, completion) = fixture.submit();
    fixture.approve(id);
    let cookie = fixture.publish(&completion, fixture.initial);
    let proof = fixture.verified(&cookie).unwrap();
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
    run_until(actor, |actor| actor.snapshot().has_owner);
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
    run_until(actor, |actor| actor.snapshot().pending_actions == 0);
    (fixture, lease)
}

#[test]
fn listener_recovery_uses_the_same_clock_as_the_control_inbox() {
    let (mut dispatcher, backend) = dispatcher_for_listener();
    let mut actor = SessionActor::without_transport(&mut dispatcher, backend, Limits::HARD, ZERO);
    let mut listener = ListenerSupervisor::new(
        FixtureDriver::default(),
        ListenerSessionSafety::new(actor.safety_signal()),
    );
    let origin = Instant::now();
    let (_, mut control) = crate::control::channel(origin);
    listener.request_inventory(origin).unwrap();
    listener.poll_transition(origin).unwrap();
    listener
        .start(fixture_selection(&listener), origin)
        .unwrap();
    let generation = loop {
        let before_drive = Instant::now();
        control.drive(&mut actor);
        listener.poll_transition(before_drive).unwrap();
        if let ListenerStatus::Listening { generation } = listener.status() {
            break generation;
        }
        assert!(origin.elapsed() < WAIT, "listener did not start");
        std::thread::yield_now();
    };
    control
        .recover_transport(&mut actor, TransportGeneration(generation))
        .unwrap();
    assert_eq!(actor.snapshot().availability, Availability::Ready);
}

#[test]
fn supervisor_waits_for_actor_retirement_and_publishes_a_fresh_generation_on_rebind() {
    let (mut dispatcher, backend) = dispatcher_for_listener();
    let mut actor = SessionActor::without_transport(&mut dispatcher, backend, Limits::HARD, ZERO);
    run_until(&mut actor, |actor| actor.snapshot().confirmed_neutral);
    let mut listener = ListenerSupervisor::new(
        FixtureDriver::default(),
        ListenerSessionSafety::new(actor.safety_signal()),
    );
    let now = Instant::now();
    listener.request_inventory(now).unwrap();
    listener.poll_transition(now).unwrap();
    listener.start(fixture_selection(&listener), now).unwrap();
    listener.poll_transition(now).unwrap();
    assert_eq!(listener.status(), ListenerStatus::Draining);
    let first_epoch = actor.snapshot().epoch;
    run_until(&mut actor, |actor| actor.snapshot().epoch > first_epoch);
    let generation = reach_listening(&mut listener, now);
    actor
        .recover(ZERO, TransportGeneration(generation))
        .unwrap();
    let (_store, lease) = paired_owner_holding_button(&mut actor, generation);

    listener.start(fixture_selection(&listener), now).unwrap();
    listener.poll_transition(now).unwrap();
    assert_eq!(listener.status(), ListenerStatus::Draining);
    run_until(&mut actor, |actor| actor.snapshot().confirmed_neutral);
    assert!(lease.is_closed());
    assert!(!actor.snapshot().has_owner);
    assert_eq!(
        actor.recover(ZERO, TransportGeneration(generation)),
        Err(SessionError::StaleAuthority)
    );
    let replacement = reach_listening(&mut listener, now);
    assert!(replacement > generation);
    actor
        .recover(ZERO, TransportGeneration(replacement))
        .unwrap();
    assert!(!actor.snapshot().has_owner);
    drop(listener);
    run_until(&mut actor, |actor| {
        actor.snapshot().availability == Availability::Stopped
    });
    drop(actor);
    let stopped = dispatcher.shutdown();
    assert_eq!(stopped.backend.calls().len(), 1);
    assert!(stopped.backend.is_neutral());
}
