use std::fmt;
use std::net::IpAddr;

const MAX_LOCAL_HOST_BYTES: usize = 32;
const MAX_LABEL_BYTES: usize = 80;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairingIdentityError {
    InvalidLocalName,
    InvalidDeviceLabel,
}

impl fmt::Display for PairingIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLocalName => "invalid local pairing hostname",
            Self::InvalidDeviceLabel => "invalid device display label",
        })
    }
}

impl std::error::Error for PairingIdentityError {}

/// Cookie host identity. Only transport can establish that a host is approved.
#[derive(Clone, Eq, PartialEq)]
pub struct HostScope(Host);

#[derive(Clone, Eq, PartialEq)]
enum Host {
    Ip(IpAddr),
    Local(Box<str>),
}

impl HostScope {
    #[must_use]
    pub const fn ip(address: IpAddr) -> Self {
        Self(Host::Ip(address))
    }

    /// Forms a scope from the actual browser host selected by the transport.
    /// DNS case is folded and a trailing dot is retained as a separate scope.
    ///
    /// # Errors
    /// Rejects names outside the generated installation/conflict name format.
    pub fn local(name: &str) -> Result<Self, PairingIdentityError> {
        if name.len() > MAX_LOCAL_HOST_BYTES || !name.is_ascii() {
            return Err(PairingIdentityError::InvalidLocalName);
        }
        let name = name.to_ascii_lowercase();
        let unrooted = name.strip_suffix('.').unwrap_or(&name);
        let instance = unrooted
            .strip_prefix("tether-")
            .and_then(|name| name.strip_suffix(".local"))
            .ok_or(PairingIdentityError::InvalidLocalName)?;
        let seed = if let Some((seed, suffix)) = instance.split_once('-') {
            if suffix.starts_with('0')
                || !suffix.bytes().all(|byte| byte.is_ascii_digit())
                || !suffix
                    .parse::<u16>()
                    .is_ok_and(|value| (2..=10_000).contains(&value))
            {
                return Err(PairingIdentityError::InvalidLocalName);
            }
            seed
        } else {
            instance
        };
        if seed.len() != 12 || !seed.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(PairingIdentityError::InvalidLocalName);
        }
        Ok(Self(Host::Local(name.into_boxed_str())))
    }

    /// The exact host without a port, for the device store or local approval UI.
    #[must_use]
    pub fn host_name(&self) -> String {
        match &self.0 {
            Host::Ip(address) => address.to_string(),
            Host::Local(name) => name.to_string(),
        }
    }
}

impl fmt::Debug for HostScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HostScope(<redacted>)")
    }
}

/// Untrusted, bounded display text from the phone. Never interpret as markup.
#[derive(Clone, Eq, PartialEq)]
pub struct DeviceLabel(Box<str>);

impl DeviceLabel {
    /// # Errors
    /// Rejects empty, oversized, padded, control or directional override text.
    pub fn new(label: &str) -> Result<Self, PairingIdentityError> {
        if label.is_empty()
            || label.len() > MAX_LABEL_BYTES
            || label.trim() != label
            || label.chars().any(|character| {
                character.is_control()
                    || matches!(
                        character,
                        '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
                    )
            })
        {
            return Err(PairingIdentityError::InvalidDeviceLabel);
        }
        Ok(Self(label.into()))
    }

    #[must_use]
    pub fn for_display(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for DeviceLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DeviceLabel(<redacted>)")
    }
}
