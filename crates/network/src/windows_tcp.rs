//! Windows socket adapter for the core-owned listener supervisor.
//!
//! The supervisor owns readiness, fresh OS observations and session cleanup.
//! Socket admission checks the published plan and connection quota before any
//! caller can hand a stream to HTTP.

use crate::interfaces::{
    BindingPlan, InterfaceError, InterfaceInventory, WindowsInterfaceProvider,
};
use crate::supervisor::{AcceptResult, AcceptedTransport, ListenerDriver};
use socket2::{Domain, Protocol, Socket, Type};
use std::fmt;
use std::io;
use std::net::{IpAddr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::num::NonZeroU64;
use std::os::windows::io::AsRawSocket;
use std::task::Poll;
use std::time::Instant;
use windows::Win32::Networking::WinSock::{
    SO_EXCLUSIVEADDRUSE, SOCKET, SOL_SOCKET, WSAGetLastError, setsockopt,
};

mod quota;

pub use quota::ConnectionPermit;
use quota::ConnectionQuota;

const BACKLOG: i32 = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TcpOperation {
    Open,
    SetOptions,
    Bind,
    Listen,
    ReadEndpoint,
    Accept,
    Clone,
    Shutdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TcpError {
    StaleAuthority,
    InvalidEndpoint,
    QuotaUnavailable,
    Io {
        operation: TcpOperation,
        kind: io::ErrorKind,
    },
}

impl fmt::Display for TcpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleAuthority => formatter.write_str("listener authority changed"),
            Self::InvalidEndpoint => {
                formatter.write_str("socket endpoint does not match its listener")
            }
            Self::QuotaUnavailable => {
                formatter.write_str("transport connection quota is unavailable")
            }
            Self::Io { operation, kind } => {
                write!(formatter, "Windows TCP {operation:?} failed ({kind:?})")
            }
        }
    }
}

impl std::error::Error for TcpError {}

fn socket_error(operation: TcpOperation, error: &io::Error) -> TcpError {
    TcpError::Io {
        operation,
        kind: error.kind(),
    }
}

/// Windows provider and socket driver for the serialized listener supervisor.
#[derive(Default)]
pub struct WindowsListenerDriver {
    provider: WindowsInterfaceProvider,
}

impl ListenerDriver for WindowsListenerDriver {
    type Set = WindowsListeners;
    type Transport = WindowsTransport;
    type Shutdown = WindowsShutdown;
    type Error = TcpError;

    fn inventory(&self) -> &InterfaceInventory {
        self.provider.inventory()
    }

    fn begin_refresh(&mut self, now: Instant) -> Result<(), InterfaceError> {
        self.provider.begin_refresh(now)
    }

    fn cancel_refresh(&mut self) {
        self.provider.cancel_refresh();
    }

    fn poll_refresh(&mut self, now: Instant) -> Poll<Result<(), InterfaceError>> {
        self.provider.poll_refresh(now)
    }

    fn bind(&mut self, plan: BindingPlan, generation: NonZeroU64) -> Result<Self::Set, TcpError> {
        WindowsListeners::bind(plan, self.provider.inventory(), generation)
    }

    fn accept(&mut self, listeners: &mut Self::Set, generation: NonZeroU64) -> AcceptResult<Self> {
        listeners
            .poll_accept(self.provider.inventory(), generation)?
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

/// A complete candidate socket set. Dropping the set closes every listener.
/// The supervisor revalidates the provider after binding before publishing it.
pub struct WindowsListeners {
    plan: BindingPlan,
    sockets: ListenerSockets,
}

impl WindowsListeners {
    /// Binds the exact approved set on one port. A single failure closes every
    /// candidate. The caller must have closed and neutralized the previous
    /// generation before invoking this operation.
    ///
    /// # Errors
    /// Returns a stale authority, invalid endpoint or sanitized socket error.
    pub fn bind(
        plan: BindingPlan,
        inventory: &InterfaceInventory,
        generation: NonZeroU64,
    ) -> Result<Self, TcpError> {
        if !plan.is_current(inventory, generation) {
            return Err(TcpError::StaleAuthority);
        }
        let sockets = ListenerSockets::bind(plan.bind_addresses())?;
        Ok(Self { plan, sockets })
    }

    /// Polls each listener at most once, rotating the starting socket. The
    /// supervisor supplies a fresh inventory. This method acquires a quota
    /// permit before returning a transport.
    ///
    /// # Errors
    /// A stale plan or unexpected socket error requires supervisor teardown.
    pub fn poll_accept(
        &mut self,
        inventory: &InterfaceInventory,
        generation: NonZeroU64,
    ) -> Result<Option<WindowsTransport>, TcpError> {
        if !self.plan.is_current(inventory, generation) {
            return Err(TcpError::StaleAuthority);
        }
        let plan = &self.plan;
        self.sockets
            .poll(|local, peer| plan.admit_peer(inventory, generation, local, peer))
    }
}

/// Accepted on-link socket. TCP acceptance grants no application authority.
pub struct WindowsTransport {
    stream: TcpStream,
    local: SocketAddr,
    peer: IpAddr,
    permit: ConnectionPermit,
}

impl WindowsTransport {
    #[must_use]
    pub const fn local_endpoint(&self) -> SocketAddr {
        self.local
    }

    #[must_use]
    pub const fn peer(&self) -> IpAddr {
        self.peer
    }

    /// Gives the supervisor an independent handle for generation shutdown.
    ///
    /// # Errors
    /// Returns a sanitized clone error.
    pub fn shutdown_handle(&self) -> Result<WindowsShutdown, TcpError> {
        self.stream
            .try_clone()
            .map(WindowsShutdown)
            .map_err(|error| socket_error(TcpOperation::Clone, &error))
    }

    /// Transfers the stream and its permit to the HTTP runtime. The permit
    /// must live until the runtime finishes with the stream.
    pub fn into_parts(self) -> (TcpStream, ConnectionPermit) {
        (self.stream, self.permit)
    }
}

pub struct WindowsShutdown(TcpStream);

impl WindowsShutdown {
    /// Shuts down both directions even while a transport still owns its stream.
    ///
    /// # Errors
    /// Returns a sanitized socket error if shutdown cannot be confirmed.
    pub fn close(&self) -> Result<(), TcpError> {
        match self.0.shutdown(Shutdown::Both) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotConnected => Ok(()),
            Err(error) => Err(socket_error(TcpOperation::Shutdown, &error)),
        }
    }
}

struct ListenerSockets {
    listeners: Vec<(SocketAddr, TcpListener)>,
    next: usize,
    quota: ConnectionQuota,
}

impl ListenerSockets {
    fn bind(addresses: impl ExactSizeIterator<Item = SocketAddr>) -> Result<Self, TcpError> {
        let listeners = addresses
            .map(|address| {
                let listener = open_listener(address)?;
                let local = listener
                    .local_addr()
                    .map_err(|error| socket_error(TcpOperation::ReadEndpoint, &error))?;
                if local != address {
                    return Err(TcpError::InvalidEndpoint);
                }
                Ok((local, listener))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            listeners,
            next: 0,
            quota: ConnectionQuota::default(),
        })
    }

    fn poll(
        &mut self,
        mut admit: impl FnMut(SocketAddr, SocketAddr) -> Option<IpAddr>,
    ) -> Result<Option<WindowsTransport>, TcpError> {
        for _ in 0..self.listeners.len() {
            let (expected, listener) = &self.listeners[self.next];
            self.next = (self.next + 1) % self.listeners.len();
            let (stream, peer) = match listener.accept() {
                Ok(connection) => connection,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock
                            | io::ErrorKind::ConnectionAborted
                            | io::ErrorKind::ConnectionReset
                    ) =>
                {
                    continue;
                }
                Err(error) => return Err(socket_error(TcpOperation::Accept, &error)),
            };
            let Ok(local) = stream.local_addr() else {
                continue;
            };
            if local != *expected {
                return Err(TcpError::InvalidEndpoint);
            }
            let Some(peer) = admit(local, peer) else {
                continue;
            };
            let Some(permit) = self
                .quota
                .reserve(peer)
                .map_err(|_| TcpError::QuotaUnavailable)?
            else {
                continue;
            };
            if stream
                .set_nonblocking(true)
                .and_then(|()| stream.set_nodelay(true))
                .is_err()
            {
                continue;
            }
            return Ok(Some(WindowsTransport {
                stream,
                local,
                peer,
                permit,
            }));
        }
        Ok(None)
    }
}

fn open_listener(address: SocketAddr) -> Result<TcpListener, TcpError> {
    if address.ip().is_unspecified()
        || address.ip().is_multicast()
        || matches!(address, SocketAddr::V6(v6) if v6.scope_id() != 0 || v6.ip().to_ipv4_mapped().is_some())
    {
        return Err(TcpError::InvalidEndpoint);
    }
    let socket = Socket::new(
        Domain::for_address(address),
        Type::STREAM,
        Some(Protocol::TCP),
    )
    .map_err(|error| socket_error(TcpOperation::Open, &error))?;
    if address.is_ipv6() {
        socket
            .set_only_v6(true)
            .map_err(|error| socket_error(TcpOperation::SetOptions, &error))?;
    }
    set_exclusive(&socket)?;
    socket
        .set_nonblocking(true)
        .map_err(|error| socket_error(TcpOperation::SetOptions, &error))?;
    socket
        .bind(&address.into())
        .map_err(|error| socket_error(TcpOperation::Bind, &error))?;
    socket
        .listen(BACKLOG)
        .map_err(|error| socket_error(TcpOperation::Listen, &error))?;
    Ok(socket.into())
}

#[allow(unsafe_code)]
fn set_exclusive(socket: &Socket) -> Result<(), TcpError> {
    let raw = usize::try_from(socket.as_raw_socket()).map_err(|_| TcpError::InvalidEndpoint)?;
    let enabled = 1_i32.to_ne_bytes();
    // SAFETY: socket2 owns the live Winsock handle and initializes Winsock.
    // The fixed-size option buffer is the documented Windows BOOL value.
    // Winsock copies the option synchronously before the borrow ends.
    let result =
        unsafe { setsockopt(SOCKET(raw), SOL_SOCKET, SO_EXCLUSIVEADDRUSE, Some(&enabled)) };
    if result == 0 {
        return Ok(());
    }
    // SAFETY: the failed call immediately preceded this thread-local query.
    let code = unsafe { WSAGetLastError() };
    Err(socket_error(
        TcpOperation::SetOptions,
        &io::Error::from_raw_os_error(code.0),
    ))
}

#[cfg(test)]
mod tests;
