//! Exact HTTP browser authorities derived from an admitted listener endpoint.

use crate::assets::StaticResponsePolicy;
use http::HeaderMap;
use http::header::{HOST, HeaderName, HeaderValue, ORIGIN};
use http::uri::Authority;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::num::{NonZeroU16, NonZeroU64};
use tether_pairing::HostScope;

const MAX_AUTHORITY_BYTES: usize = 128;
const DEFAULT_HTTP_PORT: NonZeroU16 = NonZeroU16::new(80).unwrap();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityError {
    InvalidHost,
    StaleGeneration,
}

/// A name the core has successfully advertised for one listener generation.
/// The core must revoke that generation when the advertisement changes.
#[derive(Clone)]
pub struct AdvertisedHost {
    generation: NonZeroU64,
    canonical: HostScope,
    dotted: Option<HostScope>,
}

impl AdvertisedHost {
    /// Constructs an authority from a successfully advertised installation name.
    ///
    /// # Errors
    /// Rejects names outside the installation's accepted hostname format.
    pub fn for_generation(
        generation: NonZeroU64,
        advertised: &str,
        allow_trailing_dot: bool,
    ) -> Result<Self, AuthorityError> {
        let name = advertised.strip_suffix('.').unwrap_or(advertised);
        let canonical = HostScope::local(name).map_err(|_| AuthorityError::InvalidHost)?;
        let dotted = if allow_trailing_dot {
            Some(HostScope::local(&format!("{name}.")).map_err(|_| AuthorityError::InvalidHost)?)
        } else {
            None
        };
        Ok(Self {
            generation,
            canonical,
            dotted,
        })
    }
}

#[derive(Eq, PartialEq)]
struct BrowserAuthority {
    host: HostScope,
    port: NonZeroU16,
}

impl BrowserAuthority {
    fn parse(value: &str) -> Option<Self> {
        if value.len() > MAX_AUTHORITY_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b".-:[]".contains(&byte))
        {
            return None;
        }
        let authority = value.parse::<Authority>().ok()?;
        let raw_host = authority.host();
        let suffix = value.strip_prefix(raw_host)?;
        let port = if suffix.is_empty() {
            DEFAULT_HTTP_PORT
        } else {
            let digits = suffix.strip_prefix(':')?;
            if digits.is_empty()
                || digits.len() > 5
                || digits.starts_with('0')
                || !digits.bytes().all(|byte| byte.is_ascii_digit())
            {
                return None;
            }
            NonZeroU16::new(digits.parse::<u16>().ok()?)?
        };
        let host = if let Some(v6) = raw_host
            .strip_prefix('[')
            .and_then(|ip| ip.strip_suffix(']'))
        {
            HostScope::ip(IpAddr::V6(v6.parse::<Ipv6Addr>().ok()?))
        } else {
            parse_unbracketed(raw_host)?
        };
        Some(Self { host, port })
    }
}

fn parse_unbracketed(raw_host: &str) -> Option<HostScope> {
    if let Ok(address) = raw_host.parse::<Ipv4Addr>() {
        Some(HostScope::ip(IpAddr::V4(address)))
    } else {
        HostScope::local(raw_host).ok()
    }
}

struct Allowed {
    authority: BrowserAuthority,
    policy: StaticResponsePolicy,
}

pub(super) struct RequestAuthorities {
    literal: Allowed,
    name: Option<Allowed>,
    dotted: Option<Allowed>,
}

impl RequestAuthorities {
    pub(super) fn new(
        local: SocketAddr,
        generation: NonZeroU64,
        advertised: Option<&AdvertisedHost>,
    ) -> Result<Self, AuthorityError> {
        let port = NonZeroU16::new(local.port()).ok_or(AuthorityError::InvalidHost)?;
        let literal = allowed(HostScope::ip(local.ip()), port, &local.to_string())?;
        let mut result = Self {
            literal,
            name: None,
            dotted: None,
        };
        if let Some(advertised) = advertised {
            if advertised.generation != generation {
                return Err(AuthorityError::StaleGeneration);
            }
            result.name = Some(allowed_name(&advertised.canonical, port)?);
            result.dotted = advertised
                .dotted
                .as_ref()
                .map(|host| allowed_name(host, port))
                .transpose()?;
        }
        Ok(result)
    }

    pub(super) const fn fallback(&self) -> &StaticResponsePolicy {
        &self.literal.policy
    }

    pub(super) fn policy(
        &self,
        headers: &HeaderMap,
        origin_required: bool,
    ) -> Option<&StaticResponsePolicy> {
        self.approved(headers, origin_required)
            .map(|allowed| &allowed.policy)
    }

    pub(super) fn pairing_host(&self, headers: &HeaderMap) -> Option<&HostScope> {
        self.approved(headers, true)
            .map(|allowed| &allowed.authority.host)
    }

    fn approved(&self, headers: &HeaderMap, origin_required: bool) -> Option<&Allowed> {
        let host = BrowserAuthority::parse(single_header(headers, HOST).ok()??.to_str().ok()?)?;
        match single_header(headers, ORIGIN).ok()? {
            Some(value) => {
                let origin = value.to_str().ok()?;
                let (scheme, authority) = origin.split_once("://")?;
                if !scheme.eq_ignore_ascii_case("http")
                    || BrowserAuthority::parse(authority)? != host
                {
                    return None;
                }
            }
            None if origin_required => return None,
            None => {}
        }
        std::iter::once(&self.literal)
            .chain(self.name.as_ref())
            .chain(self.dotted.as_ref())
            .find(|candidate| candidate.authority == host)
    }
}

fn allowed(host: HostScope, port: NonZeroU16, text: &str) -> Result<Allowed, AuthorityError> {
    let policy = StaticResponsePolicy::for_approved_authority(
        &text.parse().map_err(|_| AuthorityError::InvalidHost)?,
    )
    .map_err(|_| AuthorityError::InvalidHost)?;
    Ok(Allowed {
        authority: BrowserAuthority { host, port },
        policy,
    })
}

fn allowed_name(host: &HostScope, port: NonZeroU16) -> Result<Allowed, AuthorityError> {
    allowed(host.clone(), port, &format!("{}:{port}", host.host_name()))
}

fn single_header(headers: &HeaderMap, name: HeaderName) -> Result<Option<&HeaderValue>, ()> {
    let mut values = headers.get_all(name).iter();
    let value = values.next();
    values.next().is_none().then_some(value).ok_or(())
}
