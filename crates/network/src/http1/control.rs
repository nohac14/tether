//! Preflight for the only route eligible for a WebSocket upgrade.

mod socket;

pub(super) use socket::RawQuota;
pub use socket::{ControlPayload, ControlSocket, ControlSocketError};

use crate::assets::StaticResponsePolicy;
use base64::{Engine, engine::general_purpose::STANDARD};
use http::header::{
    CONNECTION, COOKIE, HeaderName, HeaderValue, SEC_WEBSOCKET_ACCEPT, SEC_WEBSOCKET_KEY,
    SEC_WEBSOCKET_PROTOCOL, SEC_WEBSOCKET_VERSION, UPGRADE,
};
use http::{HeaderMap, Method, Request, Response, StatusCode, Version};
use std::future::Future;
use std::net::IpAddr;
use tether_config::DeviceCookie;
use tether_pairing::HostScope;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::supervisor::ConnectionAdmission;

pub(super) const PATH: &str = "/control";
const SUBPROTOCOL: &str = "tether.control.v1";
const RFC6455_GUID: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// Credential presented on an admitted listener with validated Host and Origin.
pub struct ControlRequest {
    pub authority: ConnectionAdmission,
    pub host: HostScope,
    pub peer: IpAddr,
    cookie: String,
}

impl ControlRequest {
    pub(super) fn new(
        authority: &ConnectionAdmission,
        host: &HostScope,
        peer: IpAddr,
        cookie: &str,
    ) -> Self {
        Self {
            authority: authority.clone(),
            host: host.clone(),
            peer: peer.to_canonical(),
            cookie: cookie.to_owned(),
        }
    }

    #[must_use]
    pub fn cookie(&self) -> &str {
        &self.cookie
    }
}

pub enum ControlAuthorization<P> {
    Verified(P),
    Rejected,
    RateLimited,
    Unavailable,
}

/// A live reservation. Final drop must request the reserved session teardown,
/// including when the pending upgrade is cancelled or its response fails.
pub trait ControlSession: Send {
    fn is_current(&self) -> bool;

    fn run<I: AsyncRead + AsyncWrite + Unpin + Send>(
        &mut self,
        socket: &mut ControlSocket<'_, I>,
    ) -> impl Future<Output = ()> + Send;
}

impl ControlSession for std::convert::Infallible {
    fn is_current(&self) -> bool {
        match *self {}
    }

    fn run<I: AsyncRead + AsyncWrite + Unpin + Send>(
        &mut self,
        _socket: &mut ControlSocket<'_, I>,
    ) -> impl Future<Output = ()> + Send {
        std::future::ready(())
    }
}

pub(super) struct Upgrade<'a> {
    pub(super) cookie: &'a str,
    key: &'a HeaderValue,
}

impl Upgrade<'_> {
    pub(super) fn response(&self, policy: &StaticResponsePolicy) -> Response<&'static [u8]> {
        let mut digest = sha1_smol::Sha1::new();
        digest.update(self.key.as_bytes());
        digest.update(RFC6455_GUID);
        let accept = STANDARD.encode(digest.digest().bytes());
        let mut response = policy.response(StatusCode::SWITCHING_PROTOCOLS, &[]);
        let headers = response.headers_mut();
        headers.insert(CONNECTION, HeaderValue::from_static("Upgrade"));
        headers.insert(UPGRADE, HeaderValue::from_static("websocket"));
        headers.insert(
            SEC_WEBSOCKET_PROTOCOL,
            HeaderValue::from_static(SUBPROTOCOL),
        );
        headers.insert(
            SEC_WEBSOCKET_ACCEPT,
            HeaderValue::from_str(&accept).expect("valid Base64"),
        );
        response
    }
}

/// Disallow ambiguous headers before a transport library or credential
/// verifier sees them. Offers for extensions are ignored during negotiation.
pub(super) fn preflight<'a>(request: &'a Request<&[u8]>) -> Result<Upgrade<'a>, StatusCode> {
    let headers = request.headers();
    if request.method() != Method::GET
        || request.version() != Version::HTTP_11
        || !single(headers, CONNECTION).is_some_and(valid_connection)
        || !single(headers, UPGRADE)
            .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"websocket"))
        || single(headers, SEC_WEBSOCKET_VERSION).is_none_or(|v| v.as_bytes() != b"13")
        || single(headers, SEC_WEBSOCKET_PROTOCOL)
            .is_none_or(|v| v.as_bytes() != SUBPROTOCOL.as_bytes())
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let key = single(headers, SEC_WEBSOCKET_KEY)
        .filter(|value| valid_key(value))
        .ok_or(StatusCode::BAD_REQUEST)?;
    let cookie = credential(headers).ok_or(StatusCode::UNAUTHORIZED)?;
    Ok(Upgrade { cookie, key })
}

fn single(headers: &HeaderMap, name: HeaderName) -> Option<&HeaderValue> {
    let mut values = headers.get_all(name).iter();
    let first = values.next()?;
    values.next().is_none().then_some(first)
}

fn valid_connection(value: &HeaderValue) -> bool {
    let mut upgrade = false;
    for token in value.as_bytes().split(|&byte| byte == b',') {
        let token = token.trim_ascii();
        if token.is_empty() || token.eq_ignore_ascii_case(b"close") {
            return false;
        }
        if token.eq_ignore_ascii_case(b"upgrade") {
            if upgrade {
                return false;
            }
            upgrade = true;
        }
    }
    upgrade
}

fn valid_key(value: &HeaderValue) -> bool {
    // The standard Base64 form of exactly 16 bytes ends in two padding
    // characters. The final character before the padding has four zero bits.
    let Ok(bytes) = <&[u8; 24]>::try_from(value.as_bytes()) else {
        return false;
    };
    bytes[..21]
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || b"+/".contains(byte))
        && b"AQgw".contains(&bytes[21])
        && bytes[22..] == *b"=="
}

fn credential(headers: &HeaderMap) -> Option<&str> {
    let cookies = single(headers, COOKIE)?.to_str().ok()?;
    let mut credential = None;
    for pair in cookies.split(';') {
        let (name, value) = pair.trim_ascii().split_once('=')?;
        if name.trim_ascii() != DeviceCookie::NAME {
            continue;
        }
        if name != DeviceCookie::NAME
            || credential.is_some()
            || value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return None;
        }
        credential = Some(value);
    }
    credential
}
