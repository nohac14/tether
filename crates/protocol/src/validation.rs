//! Bounded, side-effect-free conversion between wire data and validated values.

use crate::wire;
use std::fmt;

mod capabilities;
mod client;
mod mapping;
mod server;
pub use capabilities::{capability_snapshot, validate_capabilities};
pub use client::*;
pub use server::*;

/// Closed failures that never contain offending data or codec error strings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationError {
    MalformedMessage,
    MessageTooLarge,
    LimitExceeded,
    Unsupported,
    IncompatibleVersion,
    InvalidLimits,
    AuthenticationRequired,
    NotController,
    InvalidSequence,
    StaleEpoch,
    InvalidTimestamp,
    InvalidInputState,
    CapabilityUnavailable,
    BackendUnavailable,
    InvalidContext,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "protocol validation: {self:?}")
    }
}
impl std::error::Error for ValidationError {}

impl ValidationError {
    /// Fixed safe wire error, never a reflected decoder or input error.
    #[must_use]
    pub fn to_wire(self) -> wire::ProtocolError {
        use wire::ProtocolErrorCode as Code;
        let code = match self {
            Self::MessageTooLarge => Code::MessageTooLarge,
            Self::Unsupported => Code::UnknownOrUnsupportedOperation,
            Self::IncompatibleVersion => Code::IncompatibleVersion,
            Self::AuthenticationRequired => Code::AuthenticationRequired,
            Self::NotController => Code::NotController,
            Self::InvalidSequence | Self::InvalidTimestamp => Code::InvalidSequence,
            Self::StaleEpoch => Code::StaleEpoch,
            Self::CapabilityUnavailable => Code::CapabilityUnavailable,
            Self::BackendUnavailable => Code::BackendUnavailable,
            Self::InvalidContext => Code::InternalFailure,
            Self::MalformedMessage
            | Self::LimitExceeded
            | Self::InvalidLimits
            | Self::InvalidInputState => Code::MalformedMessage,
        };
        wire::ProtocolError {
            code: code.into(),
            retryable: false,
            ..Default::default()
        }
    }

    /// Fixed WebSocket closure code. Scoped release must not wait for delivery.
    #[must_use]
    pub const fn close_code(self) -> u16 {
        match self {
            Self::MessageTooLarge => 1009,
            Self::AuthenticationRequired => 4001,
            Self::IncompatibleVersion => 4002,
            Self::InvalidSequence | Self::InvalidTimestamp | Self::StaleEpoch => 4003,
            Self::BackendUnavailable | Self::InvalidContext => 1011,
            _ => 1008,
        }
    }
}

/// Validated positive negotiated limits, always within version-one ceilings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    bytes: u32,
    operations: u32,
    text: u32,
    aggregate_text: u32,
    delta: u32,
    clicks: u32,
}

impl Limits {
    pub const HARD: Self = Self {
        bytes: 65_536,
        operations: 256,
        text: 4096,
        aggregate_text: 16_384,
        delta: 8_388_608,
        clicks: 2,
    };

    /// Validates advertised limits rather than clamping a peer's invalid claim.
    ///
    /// # Errors
    /// Returns a safe limit error for zero, excessive or inconsistent limits.
    pub fn from_wire(value: &wire::ProtocolLimits) -> Result<Self, ValidationError> {
        let result = Self {
            bytes: value.maximum_encoded_message_bytes,
            operations: value.maximum_operations_per_batch,
            text: value.maximum_text_operation_bytes,
            aggregate_text: value.maximum_text_bytes_per_batch,
            delta: value.maximum_absolute_fixed_delta,
            clicks: value.maximum_atomic_click_count,
        };
        if !result.within(Self::HARD) || result.aggregate_text < result.text {
            return Err(ValidationError::InvalidLimits);
        }
        Ok(result)
    }

    #[must_use]
    pub fn to_wire(self) -> wire::ProtocolLimits {
        wire::ProtocolLimits {
            maximum_encoded_message_bytes: self.bytes,
            maximum_operations_per_batch: self.operations,
            maximum_text_operation_bytes: self.text,
            maximum_text_bytes_per_batch: self.aggregate_text,
            maximum_absolute_fixed_delta: self.delta,
            maximum_atomic_click_count: self.clicks,
        }
    }

    #[must_use]
    pub const fn message_bytes(self) -> usize {
        self.bytes as usize
    }
    #[must_use]
    pub const fn operations(self) -> usize {
        self.operations as usize
    }
    #[must_use]
    pub const fn text_bytes(self) -> usize {
        self.text as usize
    }
    #[must_use]
    pub const fn aggregate_text_bytes(self) -> usize {
        self.aggregate_text as usize
    }

    pub(crate) fn within(self, ceiling: Self) -> bool {
        [
            (self.bytes, ceiling.bytes),
            (self.operations, ceiling.operations),
            (self.text, ceiling.text),
            (self.aggregate_text, ceiling.aggregate_text),
            (self.delta, ceiling.delta),
            (self.clicks, ceiling.clicks),
        ]
        .iter()
        .all(|&(value, max)| value > 0 && value <= max)
    }

    pub(crate) fn intersect(self, other: Self) -> Self {
        Self {
            bytes: self.bytes.min(other.bytes),
            operations: self.operations.min(other.operations),
            text: self.text.min(other.text),
            aggregate_text: self.aggregate_text.min(other.aggregate_text),
            delta: self.delta.min(other.delta),
            clicks: self.clicks.min(other.clicks),
        }
    }
}

pub(crate) fn required<T>(value: Option<T>) -> Result<T, ValidationError> {
    value.ok_or(ValidationError::MalformedMessage)
}

pub(crate) fn runtime_header(
    major: u32,
    minor: u32,
    sequence: u64,
    expected: u64,
) -> Result<(), ValidationError> {
    if major != 1 || minor != 0 {
        return Err(ValidationError::IncompatibleVersion);
    }
    if expected == 0 {
        return Err(ValidationError::InvalidContext);
    }
    if sequence != expected {
        return Err(ValidationError::InvalidSequence);
    }
    Ok(())
}

/// Returns no successor at exhaustion; callers must close, never wrap counters.
#[must_use]
pub const fn next_sequence(sequence: u64) -> Option<u64> {
    sequence.checked_add(1)
}
