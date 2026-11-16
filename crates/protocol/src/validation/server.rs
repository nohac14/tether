use super::{
    Limits, ValidatedHello, ValidationError, required, runtime_header, validate_capabilities,
};
use crate::{preflight, wire};
use prost::Message;
use std::fmt;

/// Trusted client-side tracking, updated only after accepting a full message.
#[derive(Clone, Copy, Debug)]
pub struct ServerContext {
    pub next_sequence: u64,
    pub epoch: u64,
    pub capability_revision: u64,
    pub control_revision: u64,
    pub limits: Limits,
}

/// Complete server envelope that passed structural and semantic checks.
#[derive(Clone, PartialEq)]
pub struct ValidatedServer(wire::ServerEnvelope);

impl fmt::Debug for ValidatedServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ValidatedServer(<redacted>)")
    }
}

impl ValidatedServer {
    #[must_use]
    pub const fn envelope(&self) -> &wire::ServerEnvelope {
        &self.0
    }
    #[must_use]
    pub fn into_envelope(self) -> wire::ServerEnvelope {
        self.0
    }
}

fn decode(bytes: &[u8], limits: Limits) -> Result<wire::ServerEnvelope, ValidationError> {
    preflight::check(bytes, wire::SERVER_ROOT, limits)?;
    wire::ServerEnvelope::decode(bytes).map_err(|_| ValidationError::MalformedMessage)
}

/// Validates server negotiation against the previously validated client offer.
///
/// # Errors
/// Rejects malformed hello, incompatible selection or policy exceeding the offer.
pub fn validate_server_hello(
    bytes: &[u8],
    offer: &ValidatedHello,
) -> Result<ValidatedServer, ValidationError> {
    let envelope = decode(bytes, offer.limits())?;
    runtime_header(envelope.protocol_major, envelope.protocol_minor, 1, 1)?;
    if envelope.sequence != 0 || envelope.session_epoch != 0 {
        return Err(ValidationError::InvalidSequence);
    }
    let Some(wire::server_envelope::Payload::ServerHello(hello)) = envelope.payload.as_ref() else {
        return Err(ValidationError::MalformedMessage);
    };
    if hello.selected_protocol_major != 1 || hello.selected_protocol_minor != 0 {
        return Err(ValidationError::IncompatibleVersion);
    }
    let limits = Limits::from_wire(required(hello.limits.as_ref())?)?;
    if !limits.within(offer.limits()) {
        return Err(ValidationError::InvalidLimits);
    }
    if hello.connection_identifier.len() != 16
        || !(250..=1000).contains(&hello.heartbeat_interval_millis)
        || !(1000..=4000).contains(&hello.heartbeat_timeout_millis)
        || hello.heartbeat_timeout_millis < 2 * hello.heartbeat_interval_millis
    {
        return Err(ValidationError::MalformedMessage);
    }
    validate_capabilities(required(hello.capabilities.as_ref())?, limits, 1)?;
    validate_control(required(hello.control_state.as_ref())?, 0, 1)?;
    Ok(ValidatedServer(envelope))
}

/// Validates runtime server messages without changing the caller's tracking state.
///
/// # Errors
/// Returns a closed error on malformed messages or stale tracking/negotiation.
pub fn validate_server(
    bytes: &[u8],
    context: &ServerContext,
) -> Result<ValidatedServer, ValidationError> {
    let envelope = decode(bytes, context.limits)?;
    runtime_header(
        envelope.protocol_major,
        envelope.protocol_minor,
        envelope.sequence,
        context.next_sequence,
    )?;
    let payload = required(envelope.payload.as_ref())?;
    match payload {
        wire::server_envelope::Payload::ServerHello(_) => {
            return Err(ValidationError::MalformedMessage);
        }
        wire::server_envelope::Payload::ControlState(control) => {
            validate_control(control, context.epoch, context.control_revision)?;
            if envelope.session_epoch != control.session_epoch {
                return Err(ValidationError::StaleEpoch);
            }
        }
        _ if envelope.session_epoch != context.epoch => return Err(ValidationError::StaleEpoch),
        wire::server_envelope::Payload::HeartbeatPong(_) => {}
        wire::server_envelope::Payload::CapabilitySnapshot(caps) => {
            validate_capabilities(caps, context.limits, context.capability_revision)?;
        }
        wire::server_envelope::Payload::ProtocolError(error) => validate_error(error)?,
    }
    Ok(ValidatedServer(envelope))
}

fn validate_control(
    control: &wire::ControlState,
    epoch: u64,
    revision: u64,
) -> Result<(), ValidationError> {
    let state = wire::ControlStateKind::try_from(control.state)
        .map_err(|_| ValidationError::Unsupported)?;
    if state == wire::ControlStateKind::Unspecified {
        return Err(ValidationError::Unsupported);
    }
    if control.revision == 0 || control.revision < revision {
        return Err(ValidationError::InvalidContext);
    }
    if control.session_epoch < epoch
        || (control.session_epoch == 0
            && matches!(
                state,
                wire::ControlStateKind::Controlling | wire::ControlStateKind::Standby
            ))
    {
        return Err(ValidationError::StaleEpoch);
    }
    if control.this_client_controls != (state == wire::ControlStateKind::Controlling) {
        return Err(ValidationError::MalformedMessage);
    }
    Ok(())
}

fn validate_error(error: &wire::ProtocolError) -> Result<(), ValidationError> {
    let code =
        wire::ProtocolErrorCode::try_from(error.code).map_err(|_| ValidationError::Unsupported)?;
    if code == wire::ProtocolErrorCode::Unspecified {
        return Err(ValidationError::Unsupported);
    }
    match (error.supported_minimum_minor, error.supported_maximum_minor) {
        (None, None) => {}
        (Some(min), Some(max))
            if min <= max && code == wire::ProtocolErrorCode::IncompatibleVersion => {}
        _ => return Err(ValidationError::MalformedMessage),
    }
    if let Some(context) = &error.numeric_context {
        let kind = wire::NumericContextKind::try_from(context.kind)
            .map_err(|_| ValidationError::Unsupported)?;
        if kind == wire::NumericContextKind::Unspecified {
            return Err(ValidationError::Unsupported);
        }
    }
    Ok(())
}
