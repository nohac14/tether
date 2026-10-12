use std::fmt;

/// Cryptographic bytes in one persistent installation identifier.
pub const INSTALLATION_ID_BYTES: usize = 32;

/// Cryptographic bytes retained for deterministic hostname derivation.
pub const HOSTNAME_SEED_BYTES: usize = 16;

/// Seed bytes exposed in the preferred hostname suffix (48 bits).
pub const HOSTNAME_SUFFIX_BYTES: usize = 6;

/// Maximum bounded conflict-renaming attempt accepted from discovery.
pub const MAX_HOSTNAME_CONFLICT_ATTEMPT: u16 = 9_999;

/// Validation failure for an installation identity or derived hostname.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityValidationError {
    InvalidInstallationId,
    InvalidHostnameSeed,
    InvalidConflictAttempt,
}

impl fmt::Display for IdentityValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInstallationId => "installation identifier is invalid",
            Self::InvalidHostnameSeed => "hostname seed is invalid",
            Self::InvalidConflictAttempt => "hostname conflict attempt is outside the limit",
        })
    }
}

impl std::error::Error for IdentityValidationError {}

/// Persistent random installation identity and independent hostname seed.
///
/// Standard diagnostics are redacted even though the preferred hostname is
/// intentionally advertised later by the discovery component.
#[derive(Clone, Eq, PartialEq)]
pub struct InstallationIdentity {
    installation_id: [u8; INSTALLATION_ID_BYTES],
    hostname_seed: [u8; HOSTNAME_SEED_BYTES],
}

impl InstallationIdentity {
    pub(crate) const fn from_parts(
        installation_id: [u8; INSTALLATION_ID_BYTES],
        hostname_seed: [u8; HOSTNAME_SEED_BYTES],
    ) -> Self {
        Self {
            installation_id,
            hostname_seed,
        }
    }

    pub(crate) fn from_encoded(
        installation_id: &str,
        hostname_seed: &str,
    ) -> Result<Self, IdentityValidationError> {
        Ok(Self {
            installation_id: decode_lower_hex(installation_id)
                .ok_or(IdentityValidationError::InvalidInstallationId)?,
            hostname_seed: decode_lower_hex(hostname_seed)
                .ok_or(IdentityValidationError::InvalidHostnameSeed)?,
        })
    }

    pub(crate) fn encoded_installation_id(&self) -> String {
        encode_lower_hex(&self.installation_id)
    }

    pub(crate) fn encoded_hostname_seed(&self) -> String {
        encode_lower_hex(&self.hostname_seed)
    }

    /// Returns the stable opaque bytes for non-secret local identifiers.
    ///
    /// Callers must not place them in ordinary logs or diagnostics.
    #[must_use]
    pub const fn installation_id_bytes(&self) -> &[u8; INSTALLATION_ID_BYTES] {
        &self.installation_id
    }

    /// Returns the stable preferred `.local` hostname.
    #[must_use]
    pub fn preferred_hostname(&self) -> LocalHostname {
        let suffix = encode_lower_hex(&self.hostname_seed[..HOSTNAME_SUFFIX_BYTES]);
        LocalHostname(format!("tether-{suffix}.local"))
    }

    /// Derives a deterministic bounded alternative after an mDNS conflict.
    ///
    /// Attempt one produces the `-2` alternative because the unsuffixed name
    /// is the first candidate.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityValidationError::InvalidConflictAttempt`] for zero or
    /// an attempt above [`MAX_HOSTNAME_CONFLICT_ATTEMPT`].
    pub fn conflict_hostname(
        &self,
        attempt: u16,
    ) -> Result<LocalHostname, IdentityValidationError> {
        if attempt == 0 || attempt > MAX_HOSTNAME_CONFLICT_ATTEMPT {
            return Err(IdentityValidationError::InvalidConflictAttempt);
        }
        let suffix = encode_lower_hex(&self.hostname_seed[..HOSTNAME_SUFFIX_BYTES]);
        let candidate = u32::from(attempt) + 1;
        Ok(LocalHostname(format!("tether-{suffix}-{candidate}.local")))
    }
}

impl fmt::Debug for InstallationIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("InstallationIdentity(<redacted>)")
    }
}

/// Valid deterministic local hostname produced from the installation seed.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LocalHostname(String);

impl LocalHostname {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for LocalHostname {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("LocalHostname")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for LocalHostname {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn encode_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn decode_lower_hex<const N: usize>(encoded: &str) -> Option<[u8; N]> {
    if encoded.len() != N * 2 {
        return None;
    }
    let mut decoded = [0_u8; N];
    let (pairs, _) = encoded.as_bytes().as_chunks::<2>();
    for (index, pair) in pairs.iter().enumerate() {
        let high = lower_hex_value(pair[0])?;
        let low = lower_hex_value(pair[1])?;
        decoded[index] = (high << 4) | low;
    }
    Some(decoded)
}

const fn lower_hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostname_derivation_is_stable_bounded_and_ip_independent() {
        let identity = InstallationIdentity::from_parts([0x55; 32], [0xab; 16]);
        assert_eq!(
            identity.preferred_hostname().as_str(),
            "tether-abababababab.local"
        );
        assert_eq!(
            identity
                .conflict_hostname(1)
                .expect("first conflict")
                .as_str(),
            "tether-abababababab-2.local"
        );
        assert_eq!(
            identity
                .conflict_hostname(MAX_HOSTNAME_CONFLICT_ATTEMPT)
                .expect("maximum conflict")
                .as_str(),
            "tether-abababababab-10000.local"
        );
        assert!(matches!(
            identity.conflict_hostname(0),
            Err(IdentityValidationError::InvalidConflictAttempt)
        ));
    }

    #[test]
    fn encoded_identity_is_exact_lowercase_hex() {
        let identity = InstallationIdentity::from_parts([0x1f; 32], [0xa0; 16]);
        let installation_id = identity.encoded_installation_id();
        let hostname_seed = identity.encoded_hostname_seed();
        assert_eq!(
            InstallationIdentity::from_encoded(&installation_id, &hostname_seed),
            Ok(identity)
        );
        assert!(matches!(
            InstallationIdentity::from_encoded(&installation_id.to_uppercase(), &hostname_seed),
            Err(IdentityValidationError::InvalidInstallationId)
        ));
        assert!(matches!(
            InstallationIdentity::from_encoded(&installation_id, "00"),
            Err(IdentityValidationError::InvalidHostnameSeed)
        ));
    }

    #[test]
    fn standard_identity_diagnostics_are_redacted() {
        let identity = InstallationIdentity::from_parts([0x42; 32], [0x24; 16]);
        let debug = format!("{identity:?}");
        assert_eq!(debug, "InstallationIdentity(<redacted>)");
        assert!(!debug.contains("4242"));
    }
}
