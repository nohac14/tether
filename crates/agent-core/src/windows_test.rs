//! Windows personal test composition. The local UI only exchanges snapshots
//! and bounded semantic commands with this reactor.

use crate::control::ControlInbox;
use crate::listener_safety::ListenerSessionSafety;
use crate::pairing::{PairingCoordinator, PairingInbox, pairing_channel_with_control};
use std::io::ErrorKind;
use std::net::{IpAddr, SocketAddr};
use std::num::{NonZeroU16, NonZeroU64};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tether_config::{DeviceId, DeviceRevocation, SettingsStore, StorageError};
use tether_input_core::{DispatcherConfig, InputBackend, InputDispatcher};
use tether_input_windows::WindowsBackend;
use tether_network::assets::EmbeddedAssets;
use tether_network::http1::StaticHttpServer;
use tether_network::interfaces::{Interface, InterfaceInventory};
use tether_network::supervisor::{ListenerStatus, ListenerSupervisor, SupervisorError};
use tether_network::windows_tcp::{TcpError, TcpOperation, WindowsListenerDriver};
use tether_pairing::{ApprovalDecision, HostScope};
use tether_protocol::validation::Limits;
use tether_session::actor::{
    Availability, GlobalStop, SafetySignal, SessionActor, TransportGeneration,
};
use tokio::task::JoinSet;

const DEFAULT_PORT: u16 = 54321;
pub const PORT_OPTIONS: [u16; 4] = [DEFAULT_PORT, 55001, 57181, 59631];
const TICK: Duration = Duration::from_millis(16);
const INVENTORY_POLL: Duration = Duration::from_secs(2);
const DESKTOP_POLL: Duration = Duration::from_millis(200);
const QR_LIFETIME: Duration = Duration::from_secs(120);
const STOP: u8 = 1;
const QUIT: u8 = 2;
const MAX_TASKS: usize = 64;

#[derive(Clone, Copy)]
pub enum Command {
    Refresh,
    Start {
        revision: u64,
        index: usize,
        port: u16,
    },
    NewCode,
    Decide {
        generation: u64,
        request: u64,
        approve: bool,
    },
    RevokePhones,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Phase {
    Startup,
    ChooseNetwork,
    Starting,
    Listening,
    Stopping,
    Unavailable,
}

#[derive(Clone)]
pub struct NetworkChoice {
    pub index: usize,
    pub label: String,
    pub address: IpAddr,
}

#[derive(Clone)]
pub struct PendingPhone {
    pub id: u64,
    pub label: String,
}

// Deliberately has no Debug: its pairing fragment is secret until consumed.
#[derive(Clone)]
pub struct Snapshot {
    pub phase: Phase,
    pub networks: Vec<NetworkChoice>,
    pub inventory_revision: u64,
    pub port: u16,
    pub generation: u64,
    pub phones: Vec<PendingPhone>,
    pub offer: Option<String>,
    pub listening_address: Option<SocketAddr>,
    pub paired: usize,
    pub connected: usize,
    pub controlling: bool,
    pub notice: Option<&'static str>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            phase: Phase::Startup,
            networks: Vec::new(),
            inventory_revision: 0,
            port: DEFAULT_PORT,
            generation: 0,
            phones: Vec::new(),
            offer: None,
            listening_address: None,
            paired: 0,
            connected: 0,
            controlling: false,
            notice: None,
        }
    }
}

struct Shared {
    view: Mutex<Snapshot>,
    stop: AtomicU8,
    safety: Mutex<Option<SafetySignal>>,
}

pub struct Pilot {
    shared: Arc<Shared>,
    commands: SyncSender<Command>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Clone)]
pub struct StopHandle(Arc<Shared>);

impl StopHandle {
    pub fn stop(&self) {
        request_stop(&self.0, STOP);
    }
}

impl Pilot {
    /// Starts the local reactor. Non-loopback binding requires a later command
    /// matching a freshly published private-network inventory revision.
    ///
    /// # Errors
    /// Returns a closed message if the reactor could not be started.
    pub fn spawn(app_data: PathBuf, assets: EmbeddedAssets) -> Result<Self, &'static str> {
        let shared = Arc::new(Shared {
            view: Mutex::new(Snapshot::default()),
            stop: AtomicU8::new(0),
            safety: Mutex::new(None),
        });
        let (commands, inbox) = sync_channel(8);
        let worker = {
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name("tether-control".into())
                .spawn(move || {
                    let guarded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        run(&app_data, assets, &shared, inbox)
                    }));
                    let message = match guarded {
                        Ok(Ok(())) => return,
                        Ok(Err(message)) => message,
                        Err(_) => "Control stopped after an internal error.",
                    };
                    if let Ok(mut old) = shared.view.lock() {
                        *old = Snapshot {
                            phase: Phase::Unavailable,
                            notice: Some(message),
                            ..Snapshot::default()
                        };
                    }
                })
                .map_err(|_| "Could not start the control service.")?
        };
        Ok(Self {
            shared,
            commands,
            worker: Mutex::new(Some(worker)),
        })
    }

    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        self.shared.view.lock().map_or_else(
            |_| Snapshot {
                phase: Phase::Unavailable,
                ..Snapshot::default()
            },
            |view| view.clone(),
        )
    }

    /// Enqueues one foreground request without blocking the local UI.
    ///
    /// # Errors
    /// Returns a closed message when the service is busy or unavailable.
    pub fn send(&self, command: Command) -> Result<(), &'static str> {
        self.commands
            .try_send(command)
            .map_err(|_| "Tether is busy or unavailable. Try again.")
    }

    pub fn stop(&self) {
        request_stop(&self.shared, STOP);
    }

    #[must_use]
    pub fn stop_handle(&self) -> StopHandle {
        StopHandle(Arc::clone(&self.shared))
    }

    pub fn shutdown(&self) {
        request_stop(&self.shared, QUIT);
        if let Ok(mut worker) = self.worker.lock()
            && let Some(worker) = worker.take()
        {
            let _ = worker.join();
        }
    }
}

impl Drop for Pilot {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn request_stop(shared: &Shared, level: u8) {
    shared.stop.fetch_max(level, Ordering::AcqRel);
    if let Ok(signal) = shared.safety.lock()
        && let Some(signal) = &*signal
    {
        signal.request(GlobalStop::RetireTransport);
    }
}

type Listener = ListenerSupervisor<WindowsListenerDriver, ListenerSessionSafety>;
type Actor<'a> = SessionActor<'a, WindowsBackend, DeviceId>;

struct Reactor {
    listener: Listener,
    pairing: PairingCoordinator,
    pairing_inbox: PairingInbox,
    control: ControlInbox,
    store: SettingsStore,
    origin: Instant,
    requested: Option<SocketAddr>,
    active: Option<NonZeroU64>,
    offer: Option<(String, Instant)>,
    notice: Option<&'static str>,
}

fn run(
    app_data: &Path,
    assets: EmbeddedAssets,
    shared: &Shared,
    commands: Receiver<Command>,
) -> Result<(), &'static str> {
    let (store, _) = SettingsStore::open(app_data).map_err(|error| match error {
        StorageError::LockHeld => "Another Tether process is using the private settings.",
        StorageError::PlatformPermissionEnforcementUnavailable => {
            "Windows could not verify owner-only access to Tether’s settings."
        }
        _ => "Could not open Tether’s private settings.",
    })?;
    if store.snapshot().is_none() || store.identity_snapshot().is_none() {
        return Err("Private settings or device identity needs recovery.");
    }
    let backend = WindowsBackend::new();
    let status = backend.status();
    if !status.available {
        return Err("Open Tether from your unlocked Windows desktop.");
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "Could not start Windows networking.")?;
    let mut dispatcher = InputDispatcher::start(backend, DispatcherConfig::default())
        .map_err(|_| "Could not start Windows input.")?;
    let mut actor =
        SessionActor::without_transport(&mut dispatcher, status, Limits::HARD, Duration::ZERO);
    let signal = actor.safety_signal();
    if let Ok(mut destination) = shared.safety.lock() {
        *destination = Some(signal.clone());
    }
    let origin = Instant::now();
    let (bridge, pairing_inbox, control) = pairing_channel_with_control(origin);
    let server = Arc::new(StaticHttpServer::with_pairing(assets, bridge));
    let mut reactor = Reactor {
        listener: ListenerSupervisor::new(
            WindowsListenerDriver::default(),
            ListenerSessionSafety::new(signal),
        ),
        pairing: PairingCoordinator::new(actor.safety_signal()),
        pairing_inbox,
        control,
        store,
        origin,
        requested: None,
        active: None,
        offer: None,
        notice: None,
    };
    runtime.block_on(reactor.run(&mut actor, shared, commands, server));
    drop(actor);
    let _result = dispatcher.shutdown();
    Ok(())
}

impl Reactor {
    async fn run(
        &mut self,
        actor: &mut Actor<'_>,
        shared: &Shared,
        commands: Receiver<Command>,
        server: Arc<StaticHttpServer<crate::pairing::PairingHttpBridge>>,
    ) {
        let mut tasks = JoinSet::new();
        let mut next_inventory = self.origin;
        let mut next_desktop = self.origin;
        let mut quitting = None;
        loop {
            let now = Instant::now();
            let request = shared.stop.swap(0, Ordering::AcqRel);
            if request != 0 {
                self.stop(now);
                // Discard any earlier start/approve clicks when control stops.
                for _ in 0..8 {
                    let _ = commands.try_recv();
                }
                if request == QUIT {
                    quitting.get_or_insert(now + Duration::from_secs(3));
                }
            }
            if now >= next_desktop {
                next_desktop = now + DESKTOP_POLL;
                if self.requested.is_some() && !WindowsBackend::desktop_available() {
                    self.stop(now);
                    self.notice = Some("Windows desktop changed. Start again after unlocking.");
                }
            }
            self.control.drive(actor);
            if quitting.is_none()
                && self.requested.is_some()
                && actor.snapshot().availability == Availability::Faulted
            {
                self.stop(now);
                self.notice = Some("Windows input became unavailable. Quit and reopen Tether.");
            }
            if let Err(error) = self.listener.poll_transition(now)
                && (error != SupervisorError::Faulted || self.notice.is_none())
            {
                self.notice = Some(transition_notice(error, self.requested.is_some()));
                self.stop(now);
            }
            self.synchronize(actor, now);
            if let Some(deadline) = quitting {
                if matches!(
                    self.listener.status(),
                    ListenerStatus::Stopped | ListenerStatus::Faulted
                ) || now >= deadline
                {
                    break;
                }
            } else {
                if self.listener.status() == ListenerStatus::Stopped && now >= next_inventory {
                    self.refresh(now);
                    next_inventory = now + INVENTORY_POLL;
                }
                if let Ok(command) = commands.try_recv() {
                    self.command(command, now);
                }
                self.pairing_inbox
                    .process_one(&mut self.pairing, &mut self.store);
                self.pairing_inbox
                    .process_control_one(&mut self.pairing, &mut self.store);
                while tasks.try_join_next().is_some() {}
                for _ in 0..8 {
                    if self.active.is_none() || tasks.len() >= MAX_TASKS {
                        break;
                    }
                    match self.listener.poll_accept(Instant::now()) {
                        Ok(Some((transport, admission))) => {
                            let server = Arc::clone(&server);
                            tasks.spawn(async move {
                                let _ = server.serve_windows(transport, admission, None).await;
                            });
                        }
                        Ok(None) => break,
                        Err(_) => {
                            self.stop(Instant::now());
                            self.notice =
                                Some("The selected network changed or became unavailable.");
                            break;
                        }
                    }
                }
            }
            self.publish(shared, actor, Instant::now());
            self.control.wait_for_activity(TICK).await;
        }
        tasks.shutdown().await;
        self.control.drive(actor);
    }

    fn refresh(&mut self, now: Instant) {
        if self.listener.status() == ListenerStatus::Stopped
            && self.listener.request_inventory(now).is_err()
        {
            self.notice = Some("Could not read Windows network interfaces.");
        }
    }

    fn stop(&mut self, now: Instant) {
        self.requested = None;
        self.offer = None;
        if let Some(generation) = self.active.take() {
            self.pairing.retire_transport(generation);
        }
        let _ = self.listener.stop(now);
    }

    fn synchronize(&mut self, actor: &mut Actor<'_>, now: Instant) {
        let ListenerStatus::Listening { generation } = self.listener.status() else {
            if let Some(previous) = self.active.take() {
                self.pairing.retire_transport(previous);
                self.offer = None;
                self.requested = None;
                self.notice =
                    Some("Network changed. Select the current private network to continue.");
            }
            return;
        };
        if self.active == Some(generation) {
            return;
        }
        if self.requested.is_none()
            || self
                .control
                .recover_transport(actor, TransportGeneration(generation))
                .is_err()
            || self.pairing.activate_transport(generation).is_err()
        {
            self.stop(now);
            self.notice = Some("Could not safely enable control. Quit and reopen Tether.");
            return;
        }
        self.active = Some(generation);
        self.issue_offer(now);
    }

    fn command(&mut self, command: Command, now: Instant) {
        match command {
            Command::Refresh => self.refresh(now),
            Command::Start {
                revision,
                index,
                port,
            } => {
                self.notice = self.start(revision, index, port, now).err();
            }
            Command::NewCode => self.issue_offer(now),
            Command::Decide {
                generation,
                request,
                approve,
            } => {
                self.notice = self.decide(generation, request, approve, now).err();
            }
            Command::RevokePhones => {
                let revision = self
                    .store
                    .devices_snapshot()
                    .map(tether_config::DevicesSnapshot::revision);
                self.notice =
                    match self
                        .pairing
                        .revoke(&mut self.store, revision, DeviceRevocation::All)
                    {
                        Ok(_) => Some("Paired phones were removed."),
                        Err(_) => Some("Could not safely update paired phones."),
                    };
                self.stop(now);
            }
        }
    }

    fn start(
        &mut self,
        revision: u64,
        index: usize,
        port: u16,
        now: Instant,
    ) -> Result<(), &'static str> {
        if self.listener.status() != ListenerStatus::Stopped || !WindowsBackend::desktop_available()
        {
            return Err("Unlock Windows and wait for control to stop before starting again.");
        }
        let inventory = self
            .listener
            .inventory()
            .current()
            .ok_or("Refresh the network list before starting.")?;
        if inventory.revision() != revision {
            return Err("The network changed. Select it again.");
        }
        let record = inventory
            .records()
            .get(index)
            .ok_or("Select a current private network.")?;
        let address = eligible(record).ok_or("Select a current private network.")?;
        let current = self
            .store
            .snapshot()
            .ok_or("Private settings are unavailable.")?;
        let port = approved_port(port, current.settings().server_port())?;
        let selection = inventory
            .approve(revision, std::slice::from_ref(record.id()), port)
            .map_err(|_| "This network cannot be used for control.")?;
        let (settings, _) = selection
            .project_settings(current.settings(), false)
            .map_err(|_| "Could not save network approval.")?;
        self.store
            .commit(current.revision(), settings)
            .map_err(|_| "Could not save network approval.")?;
        self.listener
            .start(selection, now)
            .map_err(|_| "Could not start listening on the selected network.")?;
        self.requested = Some(SocketAddr::new(address, port.get()));
        Ok(())
    }

    fn issue_offer(&mut self, now: Instant) {
        let (Some(generation), Some(endpoint)) = (self.active, self.requested) else {
            return;
        };
        if let Ok(token) =
            self.pairing
                .issue_qr(&self.store, generation, now, HostScope::ip(endpoint.ip()))
        {
            self.offer = Some((
                format!("http://{endpoint}/#pair={}", token.encode_for_fragment()),
                now + QR_LIFETIME,
            ));
            self.notice = None;
        } else {
            self.offer = None;
            self.notice = Some("Could not create a pairing code.");
        }
    }

    fn decide(
        &mut self,
        generation: u64,
        request: u64,
        approve: bool,
        now: Instant,
    ) -> Result<(), &'static str> {
        let generation = self
            .active
            .filter(|current| current.get() == generation)
            .ok_or("That pairing request is no longer current.")?;
        let id = self
            .pairing
            .approvals(generation, now)
            .map_err(|_| "That pairing request expired.")?
            .find(|approval| approval.id().get() == request && !approval.is_approved())
            .map(tether_pairing::Approval::id)
            .ok_or("That pairing request expired.")?;
        let decision = if approve {
            ApprovalDecision::Approve
        } else {
            ApprovalDecision::Deny
        };
        self.pairing
            .decide(generation, now, id, decision)
            .map_err(|_| "That pairing request expired.")?;
        self.offer = None;
        Ok(())
    }

    fn publish(&mut self, shared: &Shared, actor: &Actor<'_>, now: Instant) {
        let phase = match self.listener.status() {
            ListenerStatus::Listening { .. } if self.active.is_some() => Phase::Listening,
            ListenerStatus::Draining if self.requested.is_none() => Phase::Stopping,
            ListenerStatus::Stopped => Phase::ChooseNetwork,
            ListenerStatus::Faulted => Phase::Unavailable,
            _ => Phase::Starting,
        };
        let phase = if actor.snapshot().availability == Availability::Faulted {
            Phase::Unavailable
        } else {
            phase
        };
        let (inventory_revision, networks) = if phase == Phase::ChooseNetwork {
            network_choices(self.listener.inventory())
        } else {
            (0, Vec::new())
        };
        let phones = self
            .active
            .and_then(|generation| self.pairing.approvals(generation, now).ok())
            .map_or_else(Vec::new, |approvals| {
                approvals
                    .filter(|a| !a.is_approved())
                    .map(|a| PendingPhone {
                        id: a.id().get(),
                        label: a.label().for_display().to_owned(),
                    })
                    .collect()
            });
        if self
            .offer
            .as_ref()
            .is_some_and(|(_, deadline)| now >= *deadline)
        {
            self.offer = None;
        }
        let session = actor.snapshot();
        let port = self
            .store
            .snapshot()
            .and_then(|current| current.settings().server_port())
            .filter(|&port| port >= 1024)
            .unwrap_or(DEFAULT_PORT);
        if let Ok(mut view) = shared.view.lock() {
            *view = Snapshot {
                phase,
                networks,
                inventory_revision,
                port,
                generation: self.active.map_or(0, NonZeroU64::get),
                phones,
                offer: self.offer.as_ref().map(|(offer, _)| offer.clone()),
                listening_address: self.active.and(self.requested),
                paired: self
                    .store
                    .devices_snapshot()
                    .map_or(0, |value| value.devices().len()),
                connected: session.connections,
                controlling: session.has_owner,
                notice: self.notice,
            };
        }
    }
}

fn approved_port(port: u16, saved: Option<u16>) -> Result<NonZeroU16, &'static str> {
    if PORT_OPTIONS.contains(&port) || (port >= 1024 && saved == Some(port)) {
        return NonZeroU16::new(port).ok_or("Select a connection port from the list.");
    }
    Err("Select a connection port from the list.")
}

fn transition_notice(error: SupervisorError<TcpError>, starting_or_active: bool) -> &'static str {
    match error {
        SupervisorError::Socket(TcpError::Io {
            operation: TcpOperation::Bind,
            kind: ErrorKind::AddrInUse | ErrorKind::PermissionDenied,
        }) => {
            "Windows blocked this port or another app is using it. Choose a different connection port and click Start again."
        }
        SupervisorError::Interface(_) if !starting_or_active => {
            "Could not read Windows network interfaces. Click Refresh and try again."
        }
        SupervisorError::Interface(_) => {
            "The selected network changed or became unavailable. Select it again."
        }
        SupervisorError::Faulted
        | SupervisorError::Safety(_)
        | SupervisorError::ReleaseTimedOut
        | SupervisorError::CleanupFailed => {
            "Tether could not confirm a safe input state. Close and reopen Tether."
        }
        _ => "Could not listen on the selected private network.",
    }
}

fn eligible(interface: &Interface) -> Option<IpAddr> {
    if interface.default_exclusion().is_some() {
        return None;
    }
    interface
        .addresses()
        .iter()
        .filter(|address| address.is_usable())
        .min_by_key(|address| address.address().is_ipv6())
        .map(tether_network::interfaces::AssignedAddress::address)
}

fn network_choices(inventory: &InterfaceInventory) -> (u64, Vec<NetworkChoice>) {
    inventory.current().map_or_else(
        || (0, Vec::new()),
        |snapshot| {
            let choices = snapshot
                .records()
                .iter()
                .enumerate()
                .filter_map(|(index, record)| {
                    let address = eligible(record)?;
                    Some(NetworkChoice {
                        index,
                        label: record.name().to_owned(),
                        address,
                    })
                })
                .collect();
            (snapshot.revision(), choices)
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_PORT, ErrorKind, PORT_OPTIONS, SupervisorError, TcpError, TcpOperation};
    use super::{approved_port, transition_notice};

    #[test]
    fn a_port_change_requires_an_explicit_valid_selection() {
        for port in PORT_OPTIONS {
            assert_eq!(approved_port(port, Some(DEFAULT_PORT)).unwrap().get(), port);
        }
        assert_eq!(approved_port(55002, Some(55002)).unwrap().get(), 55002);
        for port in [0, 80, 55002] {
            assert!(approved_port(port, Some(DEFAULT_PORT)).is_err());
        }
        assert!(approved_port(80, Some(80)).is_err());
    }

    #[test]
    fn a_blocked_or_occupied_port_has_actionable_guidance() {
        for kind in [ErrorKind::PermissionDenied, ErrorKind::AddrInUse] {
            let error = SupervisorError::Socket(TcpError::Io {
                operation: TcpOperation::Bind,
                kind,
            });
            assert!(transition_notice(error, true).contains("Choose a different connection port"));
        }
    }
}
