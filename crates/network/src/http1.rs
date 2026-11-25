//! Bounded HTTP/1 asset service. The listener supervisor owns TCP acceptance.

mod authority;
mod control;
mod framing;
mod pairing;
mod rate;

pub use authority::{AdvertisedHost, AuthorityError};
pub use control::{
    ControlAuthorization, ControlPayload, ControlRequest, ControlSession, ControlSocket,
    ControlSocketError,
};
pub use pairing::{
    PairingHttpService, PairingOperation, PairingRejection, PairingReply, PairingRequest,
    StaticOnly,
};

use crate::assets::{EmbeddedAssets, StaticResponsePolicy};
use crate::supervisor::{ConnectionAdmission, ConnectionAuthority};
use authority::RequestAuthorities;
use framing::{Frame, HeaderReadError, parse_head, read_head, write_response};
use http::header::{CONNECTION, HeaderValue, SET_COOKIE, UPGRADE};
use http::{Response, StatusCode};
use pairing::Route;
use rate::RequestRate;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf};
use tokio::time::{Instant, timeout, timeout_at};

#[cfg(windows)]
use crate::windows_tcp::{WindowsShutdown, WindowsTransport};

const HEADER_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const IDLE_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_REQUESTS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServeError {
    Authority(AuthorityError),
    Revoked,
    Socket(io::ErrorKind),
    Deadline,
}

/// Serves the preloaded asset bundle on admitted supervised connections.
pub struct StaticHttpServer<P = StaticOnly> {
    assets: EmbeddedAssets,
    rate: RequestRate,
    pairing_rate: RequestRate,
    control_rate: RequestRate,
    raw_quota: control::RawQuota,
    pairing: Option<P>,
}

impl StaticHttpServer<StaticOnly> {
    #[must_use]
    pub const fn new(assets: EmbeddedAssets) -> Self {
        Self {
            assets,
            rate: RequestRate::new(),
            pairing_rate: RequestRate::pairing(),
            control_rate: RequestRate::control(),
            raw_quota: control::RawQuota::new(),
            pairing: None,
        }
    }
}

struct PeerContext<'a> {
    authorities: &'a RequestAuthorities,
    peer: IpAddr,
    admission: Option<&'a ConnectionAdmission>,
}

impl<P: PairingHttpService> StaticHttpServer<P> {
    #[must_use]
    pub const fn with_pairing(assets: EmbeddedAssets, service: P) -> Self {
        Self {
            assets,
            rate: RequestRate::new(),
            pairing_rate: RequestRate::pairing(),
            control_rate: RequestRate::control(),
            raw_quota: control::RawQuota::new(),
            pairing: Some(service),
        }
    }

    /// Runs the Windows HTTP transport on an I/O-enabled Tokio runtime. The
    /// caller maintains at most one task per admitted socket. Dropping the
    /// future drops the socket and both of its supervisor and quota guards.
    ///
    /// # Errors
    /// Returns closed categories for stale authority, I/O and expired requests.
    #[cfg(windows)]
    pub async fn serve_windows(
        &self,
        transport: WindowsTransport,
        authority: ConnectionAuthority<WindowsShutdown>,
        advertised: Option<&AdvertisedHost>,
    ) -> Result<(), ServeError> {
        if !authority.is_current() {
            return Err(ServeError::Revoked);
        }
        let local = transport.local_endpoint();
        let peer = transport.peer();
        let (stream, _permit) = transport.into_parts();
        let stream = tokio::net::TcpStream::from_std(stream)
            .map_err(|error| ServeError::Socket(error.kind()))?;
        self.serve_admitted(stream, local, peer, authority, advertised)
            .await
    }

    /// Runs a socket already admitted by the listener supervisor. The driver
    /// supplies the actual local endpoint and retains any adapter quota lease
    /// through this future.
    ///
    /// # Errors
    /// Returns closed categories for rejected or expired transport authority.
    pub async fn serve_admitted<I, H>(
        &self,
        io: I,
        local: SocketAddr,
        peer: IpAddr,
        authority: ConnectionAuthority<H>,
        advertised: Option<&AdvertisedHost>,
    ) -> Result<(), ServeError>
    where
        I: AsyncRead + AsyncWrite + Unpin + Send,
    {
        if !authority.is_current() {
            return Err(ServeError::Revoked);
        }
        let authorities = RequestAuthorities::new(local, authority.generation(), advertised)
            .map_err(ServeError::Authority)?;
        let admission = authority.admission();
        let result = self
            .serve(io, &authorities, peer, Some(&admission), || {
                admission.is_current()
            })
            .await;
        drop(authority);
        result
    }

    async fn serve<I, F>(
        &self,
        io: I,
        authorities: &RequestAuthorities,
        peer: IpAddr,
        admission: Option<&ConnectionAdmission>,
        active: F,
    ) -> Result<(), ServeError>
    where
        I: AsyncRead + AsyncWrite + Unpin + Send,
        F: Fn() -> bool + Unpin + Send,
    {
        let mut io = AuthorizedIo { io, active };
        let result = self
            .serve_requests(
                &mut io,
                PeerContext {
                    authorities,
                    peer,
                    admission,
                },
            )
            .await;
        if !(io.active)() {
            return Err(ServeError::Revoked);
        }
        result
    }

    async fn serve_requests<I>(
        &self,
        io: &mut I,
        context: PeerContext<'_>,
    ) -> Result<(), ServeError>
    where
        I: AsyncRead + AsyncWrite + Unpin + Send,
    {
        let mut buffer = vec![0; framing::MAX_HEADER_BYTES];
        let mut used = 0;
        let authorities = context.authorities;
        for request_index in 1..=MAX_REQUESTS {
            if request_index > 1 && used == 0 {
                match timeout(IDLE_TIMEOUT, io.read(&mut buffer)).await {
                    Err(_) | Ok(Ok(0)) => return Ok(()),
                    Ok(Ok(bytes)) => used = bytes,
                    Ok(Err(error)) => return Err(ServeError::Socket(error.kind())),
                }
            }
            let now = Instant::now();
            let deadline = now + REQUEST_TIMEOUT;
            let head = timeout_at(now + HEADER_TIMEOUT, read_head(io, &mut buffer, &mut used));
            let end = match head.await {
                Err(_) => {
                    send_at(
                        io,
                        deadline,
                        reject(authorities, StatusCode::REQUEST_TIMEOUT),
                    )
                    .await?;
                    return Err(ServeError::Deadline);
                }
                Ok(Ok(None)) => return Ok(()),
                Ok(Ok(Some(end))) => end,
                Ok(Err(HeaderReadError::Rejected(status))) => {
                    return send_at(io, deadline, reject(authorities, status)).await;
                }
                Ok(Err(HeaderReadError::Socket(kind))) => return Err(ServeError::Socket(kind)),
            };

            let frame = match parse_head(&buffer[..end]) {
                Ok(frame) => frame,
                Err(status) => return send_at(io, deadline, reject(authorities, status)).await,
            };
            let control = frame.request.uri().path() == control::PATH
                && frame.request.headers().contains_key(UPGRADE);
            let required_origin = control || frame.request.method() == http::Method::POST;
            let Some(policy) = authorities.policy(frame.request.headers(), required_origin) else {
                return send_at(io, deadline, reject(authorities, StatusCode::FORBIDDEN)).await;
            };
            if !self.rate.admit(context.peer, Instant::now().into_std()) {
                return send_at(
                    io,
                    deadline,
                    reject(authorities, StatusCode::TOO_MANY_REQUESTS),
                )
                .await;
            }
            if control {
                return self
                    .serve_control(io, &frame, &context, policy, &buffer[end..used], deadline)
                    .await;
            }
            if let Some(route) = Route::from_head(&frame.request) {
                return self
                    .serve_pairing(io, &context, &frame, route, &buffer[end..used], deadline)
                    .await;
            }
            let mut response = self.assets.respond(&frame.request, policy);
            *response.version_mut() = frame.request.version();
            let close = frame.close
                || request_index == MAX_REQUESTS
                || response.status() == StatusCode::BAD_REQUEST;
            if close {
                response
                    .headers_mut()
                    .insert(CONNECTION, HeaderValue::from_static("close"));
            }
            send_at(io, deadline, response).await?;
            if close {
                return Ok(());
            }
            buffer.copy_within(end..used, 0);
            used -= end;
        }
        Ok(())
    }

    async fn serve_control<I: AsyncRead + AsyncWrite + Unpin + Send>(
        &self,
        io: &mut I,
        frame: &Frame,
        context: &PeerContext<'_>,
        policy: &StaticResponsePolicy,
        prefix: &[u8],
        deadline: Instant,
    ) -> Result<(), ServeError> {
        let upgrade = match control::preflight(&frame.request) {
            Ok(upgrade) => upgrade,
            Err(status) => return send_at(io, deadline, pairing::empty(policy, status)).await,
        };
        let (Some(service), Some(admission)) = (&self.pairing, context.admission) else {
            return send_at(
                io,
                deadline,
                pairing::empty(policy, StatusCode::SERVICE_UNAVAILABLE),
            )
            .await;
        };
        if !admission.is_current() {
            return Err(ServeError::Revoked);
        }
        if !self
            .control_rate
            .admit(context.peer, Instant::now().into_std())
        {
            return send_at(
                io,
                deadline,
                pairing::empty(policy, StatusCode::TOO_MANY_REQUESTS),
            )
            .await;
        }
        let host = context
            .authorities
            .pairing_host(frame.request.headers())
            .ok_or(ServeError::Revoked)?;
        let authorization = timeout_at(
            deadline,
            service.authorize_control(ControlRequest::new(
                admission,
                host,
                context.peer,
                upgrade.cookie,
            )),
        )
        .await
        .map_err(|_| ServeError::Deadline)?;
        if !admission.is_current() {
            return Err(ServeError::Revoked);
        }
        let (proof, renewal) = match permit(authorization) {
            Ok(proof) => proof,
            Err(status) => return send_at(io, deadline, pairing::empty(policy, status)).await,
        };
        let reservation = timeout_at(deadline, service.reserve_control(proof))
            .await
            .map_err(|_| ServeError::Deadline)?;
        if !admission.is_current() {
            return Err(ServeError::Revoked);
        }
        let mut session = match permit(reservation) {
            Ok(session) => session,
            Err(status) => return send_at(io, deadline, pairing::empty(policy, status)).await,
        };
        if !session.is_current() {
            return send_at(
                io,
                deadline,
                pairing::empty(policy, StatusCode::UNAUTHORIZED),
            )
            .await;
        }
        let mut response = upgrade.response(policy);
        if let Some(cookie) = renewal {
            let mut header = HeaderValue::from_str(cookie.as_set_cookie())
                .expect("validated host-scoped credential cookie");
            header.set_sensitive(true);
            response.headers_mut().insert(SET_COOKIE, header);
        }
        send_at(io, deadline, response).await?;
        let mut socket = ControlSocket::new(io, prefix, &self.raw_quota, admission);
        session.run(&mut socket).await;
        drop(session);
        socket.close().await;
        Ok(())
    }

    async fn serve_pairing<I: AsyncRead + AsyncWrite + Unpin>(
        &self,
        io: &mut I,
        context: &PeerContext<'_>,
        frame: &Frame,
        route: Route,
        received: &[u8],
        deadline: Instant,
    ) -> Result<(), ServeError> {
        let policy = context
            .authorities
            .policy(frame.request.headers(), true)
            .ok_or(ServeError::Revoked)?;
        let (Some(service), Some(admission)) = (&self.pairing, context.admission) else {
            return send_at(
                io,
                deadline,
                pairing::empty(policy, StatusCode::SERVICE_UNAVAILABLE),
            )
            .await;
        };
        if let Err(status) = Route::validate_header(&frame.request) {
            return send_at(io, deadline, pairing::empty(policy, status)).await;
        }
        if matches!(route, Route::Request)
            && !self
                .pairing_rate
                .admit(context.peer, Instant::now().into_std())
        {
            return send_at(
                io,
                deadline,
                pairing::empty(policy, StatusCode::TOO_MANY_REQUESTS),
            )
            .await;
        }
        let mut bytes = vec![0; frame.body_bytes];
        let already_read = received.len().min(bytes.len());
        bytes[..already_read].copy_from_slice(&received[..already_read]);
        match timeout_at(deadline, io.read_exact(&mut bytes[already_read..])).await {
            Err(_) => return Err(ServeError::Deadline),
            Ok(Err(error)) if error.kind() == io::ErrorKind::UnexpectedEof => {
                return send_at(
                    io,
                    deadline,
                    pairing::empty(policy, StatusCode::BAD_REQUEST),
                )
                .await;
            }
            Ok(Err(error)) => return Err(ServeError::Socket(error.kind())),
            Ok(Ok(_)) => {}
        }
        let operation = match route.decode(&bytes) {
            Ok(value) => value,
            Err(status) => return send_at(io, deadline, pairing::empty(policy, status)).await,
        };
        let request = pairing::admit(
            &frame.request,
            context.authorities,
            admission,
            context.peer,
            operation,
        )
        .ok_or(ServeError::Revoked)?;
        let reply = timeout_at(deadline, service.handle(request))
            .await
            .map_err(|_| ServeError::Deadline)?;
        if !admission.is_current() {
            return Err(ServeError::Revoked);
        }
        send_at(io, deadline, pairing::response(policy, reply)).await
    }
}

fn permit<P>(result: ControlAuthorization<P>) -> Result<P, StatusCode> {
    match result {
        ControlAuthorization::Verified(value) => Ok(value),
        ControlAuthorization::Rejected => Err(StatusCode::UNAUTHORIZED),
        ControlAuthorization::RateLimited => Err(StatusCode::TOO_MANY_REQUESTS),
        ControlAuthorization::Unavailable => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

fn reject(authorities: &RequestAuthorities, status: StatusCode) -> Response<&'static [u8]> {
    let mut response = authorities.fallback().response(status, &[]);
    response
        .headers_mut()
        .insert(CONNECTION, HeaderValue::from_static("close"));
    response
}

async fn send_at<I: AsyncWrite + Unpin, B: AsRef<[u8]>>(
    io: &mut I,
    deadline: Instant,
    response: Response<B>,
) -> Result<(), ServeError> {
    timeout_at(deadline, write_response(io, response))
        .await
        .map_err(|_| ServeError::Deadline)?
        .map_err(|error| ServeError::Socket(error.kind()))
}

struct AuthorizedIo<I, F> {
    io: I,
    active: F,
}

fn revoked() -> io::Error {
    io::Error::from(io::ErrorKind::PermissionDenied)
}

impl<I: AsyncRead + Unpin, F: Fn() -> bool + Unpin> AsyncRead for AuthorizedIo<I, F> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if !(self.active)() {
            return Poll::Ready(Err(revoked()));
        }
        Pin::new(&mut self.io).poll_read(context, buffer)
    }
}

impl<I: AsyncWrite + Unpin, F: Fn() -> bool + Unpin> AsyncWrite for AuthorizedIo<I, F> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        if !(self.active)() {
            return Poll::Ready(Err(revoked()));
        }
        Pin::new(&mut self.io).poll_write(context, bytes)
    }

    fn poll_flush(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        if !(self.active)() {
            return Poll::Ready(Err(revoked()));
        }
        Pin::new(&mut self.io).poll_flush(context)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_shutdown(context)
    }
}

#[cfg(test)]
mod tests;
