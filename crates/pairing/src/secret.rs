use ctutils::CtEq;
use sha2::{Digest, Sha256};
use std::fmt;

pub(crate) const SECRET_BYTES: usize = 32;
const ENCODED_BYTES: usize = SECRET_BYTES * 2;

/// An issued one-time secret destined exclusively for the QR fragment.
pub struct QrToken(pub(crate) [u8; SECRET_BYTES]);

impl QrToken {
    #[must_use]
    pub fn encode_for_fragment(&self) -> String {
        encode(&self.0)
    }
}

/// A separate short-lived proof for the one accepted pending approval.
pub struct CompletionCapability(pub(crate) [u8; SECRET_BYTES]);

impl CompletionCapability {
    #[must_use]
    pub fn encode_for_response_body(&self) -> String {
        encode(&self.0)
    }
}

/// SHA-256 credential digest. The caller also checks host, expiry and revocation.
#[derive(Clone)]
pub struct CredentialDigest(SecretDigest);

impl CredentialDigest {
    /// Constructs a digest read from the owner-protected device store.
    #[must_use]
    pub const fn from_stored_sha256(value: [u8; SECRET_BYTES]) -> Self {
        Self(SecretDigest(value))
    }

    /// Hashes a separate 256-bit OS draw for durable credential enrollment.
    #[must_use]
    pub fn for_enrollment(value: &[u8; SECRET_BYTES]) -> Self {
        Self(SecretDigest::of(value))
    }

    /// Returns the digest for the owner-protected device store only.
    #[must_use]
    pub const fn as_stored_sha256(&self) -> &[u8; SECRET_BYTES] {
        &self.0.0
    }

    /// Parses and hashes the exact cookie value once for a bounded device lookup.
    #[must_use]
    pub fn from_cookie_value(value: &str) -> Option<Self> {
        SecretDigest::from_encoded(value).map(Self)
    }

    /// Compares the complete fixed-size digests using the same constant-time primitive.
    #[must_use]
    pub fn matches_digest(&self, candidate: &Self) -> bool {
        self.0.matches(&candidate.0)
    }

    /// Accepts the exact canonical credential encoding and compares digests.
    #[must_use]
    pub fn matches_cookie_value(&self, value: &str) -> bool {
        Self::from_cookie_value(value).is_some_and(|candidate| self.matches_digest(&candidate))
    }
}

macro_rules! redacted_debug {
    ($($name:ident),+ $(,)?) => {$(
        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($name), "(<redacted>)"))
            }
        }
    )+};
}

redacted_debug!(QrToken, CompletionCapability, CredentialDigest);

#[derive(Clone)]
pub(crate) struct SecretDigest([u8; SECRET_BYTES]);

impl SecretDigest {
    pub(crate) fn of(value: &[u8; SECRET_BYTES]) -> Self {
        Self(Sha256::digest(value).into())
    }

    pub(crate) fn from_encoded(value: &str) -> Option<Self> {
        if value.len() != ENCODED_BYTES {
            return None;
        }
        let mut decoded = [0; SECRET_BYTES];
        let (pairs, _) = value.as_bytes().as_chunks::<2>();
        for (byte, pair) in decoded.iter_mut().zip(pairs) {
            *byte = nibble(pair[0])? << 4 | nibble(pair[1])?;
        }
        Some(Self::of(&decoded))
    }

    pub(crate) fn matches(&self, candidate: &Self) -> bool {
        bool::from(self.0.ct_eq(&candidate.0))
    }
}

fn encode(bytes: &[u8; SECRET_BYTES]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(ENCODED_BYTES);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
