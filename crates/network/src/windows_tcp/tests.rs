use super::*;
use crate::assets::{EmbeddedAssets, EmbeddedFile};
use crate::http1::StaticHttpServer;
use crate::interfaces::{
    AddressState, AssignedAddress, IdentityPersistence, Interface, InterfaceIndexes, InterfaceKind,
    InterfaceStatus, LinkState, NetworkProfile,
};
use crate::supervisor::{
    ConnectionAuthority, ListenerStatus, ListenerSupervisor, NeutralityTicket, ReleaseCause,
    SafetyCoordinator, SafetyError,
};
use std::cell::Cell;
use std::io::Read;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::num::{NonZeroU16, NonZeroU32};
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant};
use tether_config::ApprovedInterfaceId;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const DEADLINE: Duration = Duration::from_secs(2);

#[test]
fn exclusive_socket_rejects_a_second_live_binding() {
    let listener = open_listener(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();
    let bound = listener.local_addr().unwrap();
    let competing = open_listener(bound);
    assert!(matches!(
        competing,
        Err(TcpError::Io {
            operation: TcpOperation::Bind,
            kind: io::ErrorKind::AddrInUse | io::ErrorKind::PermissionDenied,
        })
    ));
    drop(listener);
    assert!(open_listener(bound).is_ok());
}

#[test]
fn ipv6_is_explicitly_single_family_and_failed_sets_roll_back() {
    let occupied = open_listener(SocketAddr::from((Ipv6Addr::LOCALHOST, 0))).unwrap();
    let occupied = Socket::from(occupied);
    assert!(occupied.only_v6().unwrap());
    let ipv6 = occupied.local_addr().unwrap().as_socket().unwrap();
    let ipv4 = SocketAddr::from((Ipv4Addr::LOCALHOST, ipv6.port()));
    // The IPv4 candidate succeeds first. The occupied IPv6 endpoint aborts
    // the entire set, which must close the already-bound IPv4 candidate.
    let guard = open_listener(ipv4).unwrap();
    drop(guard);
    let candidate = ListenerSockets::bind([ipv4, ipv6].into_iter());
    assert!(matches!(
        candidate,
        Err(TcpError::Io {
            operation: TcpOperation::Bind,
            kind: io::ErrorKind::AddrInUse | io::ErrorKind::PermissionDenied,
        })
    ));
    assert!(open_listener(ipv4).is_ok());
}

#[test]
fn accepted_endpoints_are_checked_and_shutdown_controls_the_live_stream() {
    let listener = open_listener(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();
    let address = listener.local_addr().unwrap();
    let mut sockets = ListenerSockets {
        listeners: vec![(address, listener)],
        next: 0,
        quota: ConnectionQuota::default(),
    };
    let mut client = TcpStream::connect_timeout(&address, DEADLINE).unwrap();
    client.set_read_timeout(Some(DEADLINE)).unwrap();
    let peer = client.local_addr().unwrap();
    let started = Instant::now();
    let transport = loop {
        if let Some(transport) = sockets
            .poll(|local, source| {
                assert_eq!(local, address);
                assert_eq!(source, peer);
                Some(source.ip())
            })
            .unwrap()
        {
            break transport;
        }
        assert!(
            started.elapsed() < DEADLINE,
            "loopback acceptance timed out"
        );
        thread::yield_now();
    };
    assert_eq!(transport.peer(), IpAddr::V4(Ipv4Addr::LOCALHOST));
    let shutdown = transport.shutdown_handle().unwrap();
    let (stream, _permit) = transport.into_parts();
    assert!(stream.nodelay().unwrap());
    shutdown.close().unwrap();
    shutdown.close().unwrap();
    let mut byte = [0];
    assert_eq!(client.read(&mut byte).unwrap(), 0);
}

#[test]
fn a_saturated_peer_cannot_reach_the_application_transport() {
    let listener = open_listener(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).unwrap();
    let address = listener.local_addr().unwrap();
    let mut sockets = ListenerSockets {
        listeners: vec![(address, listener)],
        next: 0,
        quota: ConnectionQuota::default(),
    };
    let _permits: Vec<_> = (0..8)
        .map(|_| sockets.quota.reserve(address.ip()).unwrap().unwrap())
        .collect();
    let mut client = TcpStream::connect_timeout(&address, DEADLINE).unwrap();
    client.set_read_timeout(Some(DEADLINE)).unwrap();
    let started = Instant::now();
    loop {
        let mut observed = false;
        assert!(
            sockets
                .poll(|_, peer| {
                    observed = true;
                    Some(peer.ip())
                })
                .unwrap()
                .is_none()
        );
        if observed {
            break;
        }
        assert!(
            started.elapsed() < DEADLINE,
            "loopback acceptance timed out"
        );
        thread::yield_now();
    }
    let mut byte = [0];
    match client.read(&mut byte) {
        Ok(received) => assert_eq!(received, 0),
        Err(error) => assert_eq!(error.kind(), io::ErrorKind::ConnectionReset),
    }
}

#[test]
fn plans_with_retired_authority_stop_before_binding() {
    let mut inventory = InterfaceInventory::default();
    let snapshot = inventory.refresh(Ok(vec![fixture_adapter()])).unwrap();
    let approval = snapshot
        .approve(
            snapshot.revision(),
            &[snapshot.records()[0].id().clone()],
            NonZeroU16::new(54321).unwrap(),
        )
        .unwrap();
    let plan = BindingPlan::new(snapshot, &approval, NonZeroU64::MIN).unwrap();
    inventory.retire();
    assert!(matches!(
        WindowsListeners::bind(plan, &inventory, NonZeroU64::MIN),
        Err(TcpError::StaleAuthority)
    ));
    assert!(matches!(
        open_listener("0.0.0.0:54321".parse().unwrap()),
        Err(TcpError::InvalidEndpoint)
    ));
    assert!(matches!(
        open_listener("[::]:54321".parse().unwrap()),
        Err(TcpError::InvalidEndpoint)
    ));
}

fn fixture_adapter() -> Interface {
    Interface::new(
        ApprovedInterfaceId::new("socket-test".to_owned()).unwrap(),
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
                Ipv4Addr::new(192, 0, 2, 1).into(),
                24,
                AddressState::Preferred,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

// Keep all real socket activity on loopback. The inventory remains synthetic
// because production plans intentionally reject loopback as a LAN interface.
struct LoopbackDriver {
    inventory: InterfaceInventory,
    endpoint: Rc<Cell<Option<SocketAddr>>>,
}

impl ListenerDriver for LoopbackDriver {
    type Set = ListenerSockets;
    type Transport = WindowsTransport;
    type Shutdown = WindowsShutdown;
    type Error = TcpError;

    fn inventory(&self) -> &InterfaceInventory {
        &self.inventory
    }

    fn begin_refresh(&mut self, _now: Instant) -> Result<(), InterfaceError> {
        Ok(())
    }

    fn cancel_refresh(&mut self) {}

    fn poll_refresh(&mut self, _now: Instant) -> Poll<Result<(), InterfaceError>> {
        Poll::Ready(
            self.inventory
                .refresh(Ok(vec![fixture_adapter()]))
                .map(|_| ()),
        )
    }

    fn bind(&mut self, plan: BindingPlan, generation: NonZeroU64) -> Result<Self::Set, TcpError> {
        assert!(plan.is_current(&self.inventory, generation));
        let listener = open_listener(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
        let address = listener.local_addr().unwrap();
        self.endpoint.set(Some(address));
        Ok(ListenerSockets {
            listeners: vec![(address, listener)],
            next: 0,
            quota: ConnectionQuota::default(),
        })
    }

    fn accept(&mut self, listeners: &mut Self::Set, _generation: NonZeroU64) -> AcceptResult<Self> {
        listeners
            .poll(|_, peer| peer.ip().is_loopback().then_some(peer.ip()))?
            .map(|transport| {
                let shutdown = transport.shutdown_handle()?;
                Ok(AcceptedTransport {
                    transport,
                    shutdown,
                })
            })
            .transpose()
    }

    fn close(shutdown: &Self::Shutdown) -> Result<(), TcpError> {
        shutdown.close()
    }
}

struct ReleaseFlag(Rc<Cell<bool>>);

impl NeutralityTicket for ReleaseFlag {
    fn poll_neutrality(&mut self) -> Poll<Result<(), SafetyError>> {
        if self.0.get() {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}

impl SafetyCoordinator for ReleaseFlag {
    type Ticket = Self;

    fn request_release(
        &mut self,
        _cause: ReleaseCause,
        _obsolete: Option<NonZeroU64>,
    ) -> Result<Self, SafetyError> {
        Ok(Self(Rc::clone(&self.0)))
    }
}

struct RunningLoopback {
    supervisor: ListenerSupervisor<LoopbackDriver, ReleaseFlag>,
    endpoint: SocketAddr,
    release_ready: Rc<Cell<bool>>,
}

impl RunningLoopback {
    fn accept(&mut self) -> (WindowsTransport, ConnectionAuthority<WindowsShutdown>) {
        let started = Instant::now();
        loop {
            self.supervisor.poll_transition(Instant::now()).unwrap();
            if let Some(accepted) = self.supervisor.poll_accept(Instant::now()).unwrap() {
                return accepted;
            }
            assert!(started.elapsed() < DEADLINE);
            thread::yield_now();
        }
    }
}

fn running_loopback() -> RunningLoopback {
    let endpoint = Rc::new(Cell::new(None));
    let release_ready = Rc::new(Cell::new(true));
    let driver = LoopbackDriver {
        inventory: InterfaceInventory::default(),
        endpoint: Rc::clone(&endpoint),
    };
    let mut supervisor = ListenerSupervisor::new(driver, ReleaseFlag(Rc::clone(&release_ready)));
    let now = Instant::now();
    supervisor.request_inventory(now).unwrap();
    supervisor.poll_transition(now).unwrap();
    let snapshot = supervisor.inventory().current().unwrap();
    let approval = snapshot
        .approve(
            snapshot.revision(),
            &[snapshot.records()[0].id().clone()],
            NonZeroU16::new(54321).unwrap(),
        )
        .unwrap();
    supervisor.start(approval, now).unwrap();
    for _ in 0..3 {
        supervisor.poll_transition(now).unwrap();
    }
    RunningLoopback {
        supervisor,
        endpoint: endpoint.get().unwrap(),
        release_ready,
    }
}

#[test]
fn supervisor_revokes_and_terminates_a_retained_windows_socket() {
    let mut fixture = running_loopback();
    let mut client = TcpStream::connect_timeout(&fixture.endpoint, DEADLINE).unwrap();
    client.set_read_timeout(Some(DEADLINE)).unwrap();
    let (transport, authority) = fixture.accept();
    let supervisor = &mut fixture.supervisor;
    assert!(authority.is_current());
    fixture.release_ready.set(false);
    supervisor.stop(Instant::now()).unwrap();
    assert!(!authority.is_current());
    supervisor.poll_transition(Instant::now()).unwrap();
    assert_eq!(supervisor.status(), ListenerStatus::Draining);
    fixture.release_ready.set(true);
    supervisor.poll_transition(Instant::now()).unwrap();
    assert_eq!(supervisor.status(), ListenerStatus::Stopped);
    let (accepted_socket, _permit) = transport.into_parts();
    assert!(accepted_socket.nodelay().unwrap());
    assert_eq!(client.read(&mut [0]).unwrap(), 0);
}

#[test]
fn windows_supervised_http_serves_assets_and_refuses_an_unrelated_host() {
    const PAGE: &[u8] = b"<title>Windows loopback fixture</title>";
    const SCRIPT: &[u8] = b"export const fixture = true;";
    let mut fixture = running_loopback();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for valid in [true, false] {
        let client = TcpStream::connect_timeout(&fixture.endpoint, DEADLINE).unwrap();
        client.set_nonblocking(true).unwrap();
        let (transport, authority) = fixture.accept();
        let host = if valid {
            fixture.endpoint.to_string()
        } else {
            "unapproved.invalid:54321".to_owned()
        };
        let head = format!(
            "GET / HTTP/1.1\r\nHost: {host}\r\n\r\nGET /assets/main-1234abcd.js HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n",
        );
        runtime.block_on(async {
            let assets = EmbeddedAssets::new(
                PAGE,
                &[EmbeddedFile {
                    key: "main-1234abcd.js",
                    bytes: SCRIPT,
                }],
            )
            .unwrap();
            let worker = tokio::spawn(async move {
                StaticHttpServer::new(assets)
                    .serve_windows(transport, authority, None)
                    .await
            });
            let mut io = tokio::net::TcpStream::from_std(client).unwrap();
            io.write_all(head.as_bytes()).await.unwrap();
            let mut response = String::new();
            tokio::time::timeout(DEADLINE, io.read_to_string(&mut response))
                .await
                .unwrap()
                .unwrap();
            worker.await.unwrap().unwrap();
            if valid {
                assert_eq!(response.matches("HTTP/1.1 200").count(), 2);
                assert!(response.contains(std::str::from_utf8(PAGE).unwrap()));
                assert!(response.ends_with(std::str::from_utf8(SCRIPT).unwrap()));
            } else {
                assert!(response.starts_with("HTTP/1.1 403"));
                assert!(!response.contains("unapproved.invalid"));
            }
        });
    }
}
