use super::{Limits, ValidationError, mapping, required, runtime_header};
use crate::{preflight, wire};
use prost::Message;
use std::collections::BTreeSet;
use std::fmt;
use tether_input_core as input;

/// Caller-owned ordered held-state expectation, possibly an admission projection.
/// This is not proof of native execution. Validation only clones the snapshot.
#[derive(Clone, Default, Eq, PartialEq)]
pub struct HeldState {
    pub buttons: BTreeSet<input::MouseButton>,
    pub keys: BTreeSet<input::PhysicalKey>,
}

impl fmt::Debug for HeldState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HeldState(<redacted>)")
    }
}

/// Immutable facts supplied by the future authenticated, serialized session actor.
pub struct ClientContext<'a> {
    pub authenticated: bool,
    pub controls_input: bool,
    pub epoch: u64,
    pub next_sequence: u64,
    pub last_timestamp_micros: u64,
    pub last_heartbeat_identifier: Option<u64>,
    pub limits: Limits,
    pub backend: &'a input::BackendStatus,
    pub held: &'a HeldState,
}

/// Semantically distinct operations; ownership barriers cannot become native input.
#[derive(Clone, Eq, PartialEq)]
pub enum Operation {
    Input(input::InputAction),
    RequestControl,
    RelinquishControl,
    ReleaseAll,
}

/// Ordered action with a checked client-local timestamp, not a desktop deadline.
#[derive(Clone, Eq, PartialEq)]
pub struct TimedOperation {
    timestamp_micros: u64,
    operation: Operation,
}

impl TimedOperation {
    #[must_use]
    pub const fn timestamp_micros(&self) -> u64 {
        self.timestamp_micros
    }
    #[must_use]
    pub const fn operation(&self) -> &Operation {
        &self.operation
    }

    #[must_use]
    pub fn into_operation(self) -> Operation {
        self.operation
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum ClientMessage {
    Batch(Vec<TimedOperation>),
    Heartbeat(u64),
    Leaving(wire::ClientLeavingReason),
}

/// Only constructed after an entire runtime envelope succeeds.
#[derive(Clone, Eq, PartialEq)]
pub struct ValidatedClient {
    sequence: u64,
    epoch: u64,
    timestamp_micros: u64,
    message: ClientMessage,
}

impl ValidatedClient {
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }
    /// Greatest operation timestamp, or envelope timestamp for non-input messages.
    #[must_use]
    pub const fn timestamp_micros(&self) -> u64 {
        self.timestamp_micros
    }
    #[must_use]
    pub const fn message(&self) -> &ClientMessage {
        &self.message
    }
    #[must_use]
    pub fn into_message(self) -> ClientMessage {
        self.message
    }
}

/// Validated initial peer metadata. Display name remains untrusted UI content.
pub struct ValidatedHello {
    build: String,
    display_name: String,
    limits: Limits,
    compose_text: bool,
}

impl ValidatedHello {
    #[must_use]
    pub fn build_identifier(&self) -> &str {
        &self.build
    }
    #[must_use]
    pub fn untrusted_display_name(&self) -> &str {
        &self.display_name
    }
    #[must_use]
    pub const fn limits(&self) -> Limits {
        self.limits
    }
    #[must_use]
    pub const fn compose_text(&self) -> bool {
        self.compose_text
    }
}

macro_rules! redacted {
    ($($name:ident),+ $(,)?) => {$(
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($name), "(<redacted>)"))
            }
        }
    )+};
}
redacted!(
    Operation,
    TimedOperation,
    ClientMessage,
    ValidatedClient,
    ValidatedHello
);

/// Validates bootstrap negotiation after the caller has authenticated the upgrade.
///
/// # Errors
/// Returns closed errors for unauthenticated, malformed or incompatible hellos.
pub fn validate_client_hello(
    bytes: &[u8],
    authenticated: bool,
    server_limits: Limits,
) -> Result<ValidatedHello, ValidationError> {
    let envelope = decode(bytes, authenticated, server_limits)?;
    runtime_header(envelope.protocol_major, envelope.protocol_minor, 1, 1)?;
    if envelope.sequence != 0 || envelope.session_epoch != 0 {
        return Err(ValidationError::InvalidSequence);
    }
    let Some(wire::client_envelope::Payload::ClientHello(hello)) = envelope.payload else {
        return Err(ValidationError::MalformedMessage);
    };
    if hello.protocol_major != 1
        || hello.minimum_protocol_minor > hello.maximum_protocol_minor
        || hello.minimum_protocol_minor != 0
    {
        return Err(ValidationError::IncompatibleVersion);
    }
    if hello.build_identifier.is_empty()
        || !hello
            .build_identifier
            .bytes()
            .all(|b| (0x20..=0x7e).contains(&b))
        || hello.client_display_name.is_empty()
        || hello.client_display_name.chars().any(char::is_control)
    {
        return Err(ValidationError::MalformedMessage);
    }
    let capabilities = unique_capabilities(&hello.capabilities)?;
    if !capabilities.contains(&1) || !capabilities.contains(&2) {
        return Err(ValidationError::Unsupported);
    }
    let client = required(hello.limits)?;
    let limits = Limits {
        bytes: client.maximum_encoded_message_bytes,
        operations: client.maximum_operations_per_batch,
        text: client.maximum_text_operation_bytes,
        ..Limits::HARD
    };
    if !limits.within(Limits::HARD) {
        return Err(ValidationError::InvalidLimits);
    }
    Ok(ValidatedHello {
        build: hello.build_identifier,
        display_name: hello.client_display_name,
        limits: limits.intersect(server_limits),
        compose_text: capabilities.contains(&3),
    })
}

fn unique_capabilities(values: &[i32]) -> Result<BTreeSet<i32>, ValidationError> {
    let mut result = BTreeSet::new();
    for &value in values {
        if !(1..=3).contains(&value) || !result.insert(value) {
            return Err(ValidationError::Unsupported);
        }
    }
    Ok(result)
}

fn decode(
    bytes: &[u8],
    authenticated: bool,
    limits: Limits,
) -> Result<wire::ClientEnvelope, ValidationError> {
    if !authenticated {
        return Err(ValidationError::AuthenticationRequired);
    }
    preflight::check(bytes, wire::CLIENT_ROOT, limits)?;
    wire::ClientEnvelope::decode(bytes).map_err(|_| ValidationError::MalformedMessage)
}

/// Validates a runtime message without changing session state or executing input.
///
/// # Errors
/// Rejects the whole envelope on any framing, context, state or capability error.
pub fn validate_client(
    bytes: &[u8],
    context: &ClientContext<'_>,
) -> Result<ValidatedClient, ValidationError> {
    let envelope = decode(bytes, context.authenticated, context.limits)?;
    runtime_header(
        envelope.protocol_major,
        envelope.protocol_minor,
        envelope.sequence,
        context.next_sequence,
    )?;
    if context.controls_input && context.epoch == 0 {
        return Err(ValidationError::InvalidContext);
    }
    if envelope.session_epoch != context.epoch {
        return Err(ValidationError::StaleEpoch);
    }
    if envelope.client_timestamp_micros < context.last_timestamp_micros {
        return Err(ValidationError::InvalidTimestamp);
    }
    let mut timestamp = envelope.client_timestamp_micros;
    let message = match required(envelope.payload)? {
        wire::client_envelope::Payload::ClientHello(_) => {
            return Err(ValidationError::MalformedMessage);
        }
        wire::client_envelope::Payload::HeartbeatPing(ping) => {
            if context
                .last_heartbeat_identifier
                .is_some_and(|last| ping.heartbeat_identifier <= last)
            {
                return Err(ValidationError::InvalidSequence);
            }
            ClientMessage::Heartbeat(ping.heartbeat_identifier)
        }
        wire::client_envelope::Payload::ClientLeaving(leaving) => {
            let reason = wire::ClientLeavingReason::try_from(leaving.reason)
                .map_err(|_| ValidationError::Unsupported)?;
            if reason == wire::ClientLeavingReason::Unspecified {
                return Err(ValidationError::Unsupported);
            }
            ClientMessage::Leaving(reason)
        }
        wire::client_envelope::Payload::OperationBatch(batch) => {
            let operations = validate_batch(batch, timestamp, context)?;
            timestamp = operations
                .last()
                .ok_or(ValidationError::MalformedMessage)?
                .timestamp_micros;
            ClientMessage::Batch(operations)
        }
    };
    Ok(ValidatedClient {
        sequence: envelope.sequence,
        epoch: envelope.session_epoch,
        timestamp_micros: timestamp,
        message,
    })
}

fn validate_batch(
    batch: wire::ClientOperationBatch,
    base: u64,
    context: &ClientContext<'_>,
) -> Result<Vec<TimedOperation>, ValidationError> {
    if batch.operations.is_empty() || batch.operations.len() > context.limits.operations() {
        return Err(ValidationError::LimitExceeded);
    }
    let count = batch.operations.len();
    let mut result = Vec::with_capacity(count);
    let mut held = context.held.clone();
    let mut text_bytes = 0_usize;
    let mut previous = base;
    for (index, value) in batch.operations.into_iter().enumerate() {
        let timestamp = base
            .checked_add(u64::from(value.timestamp_offset_micros))
            .ok_or(ValidationError::InvalidTimestamp)?;
        if timestamp < previous {
            return Err(ValidationError::InvalidTimestamp);
        }
        previous = timestamp;
        let (operation, repeat) =
            operation(required(value.action)?, context, &mut held, &mut text_bytes)?;
        if !matches!(operation, Operation::Input(_)) && index + 1 != count {
            return Err(ValidationError::InvalidInputState);
        }
        if result.len() + repeat > context.limits.operations() {
            return Err(ValidationError::LimitExceeded);
        }
        for _ in 0..repeat {
            result.push(TimedOperation {
                timestamp_micros: timestamp,
                operation: operation.clone(),
            });
        }
    }
    Ok(result)
}

fn operation(
    action: wire::client_operation::Action,
    context: &ClientContext<'_>,
    held: &mut HeldState,
    text_bytes: &mut usize,
) -> Result<(Operation, usize), ValidationError> {
    use wire::client_operation::Action;
    match action {
        Action::RequestControl(_) => return Ok((Operation::RequestControl, 1)),
        Action::RelinquishControl(_) | Action::ReleaseAll(_) if !context.controls_input => {
            return Err(ValidationError::NotController);
        }
        Action::RelinquishControl(_) => return Ok((Operation::RelinquishControl, 1)),
        Action::ReleaseAll(_) => return Ok((Operation::ReleaseAll, 1)),
        _ => {}
    }
    if !context.controls_input {
        return Err(ValidationError::NotController);
    }
    let (input, repeat) = native_action(action, context.limits, text_bytes)?;
    context
        .backend
        .validate_action(&input)
        .map_err(|error| match error {
            input::CapabilityRejection::CapabilityUnavailable => {
                ValidationError::CapabilityUnavailable
            }
            _ => ValidationError::BackendUnavailable,
        })?;
    project(&input, held)?;
    Ok((Operation::Input(input), repeat))
}

fn native_action(
    action: wire::client_operation::Action,
    limits: Limits,
    text_bytes: &mut usize,
) -> Result<(input::InputAction, usize), ValidationError> {
    use input::InputAction as I;
    use wire::client_operation::Action;
    let mut repeat = 1;
    let action = match action {
        Action::PointerMove(value) => {
            delta(value.delta_x_1024, limits)?;
            delta(value.delta_y_1024, limits)?;
            I::PointerMove(
                input::PointerDelta::new(value.delta_x_1024, value.delta_y_1024)
                    .map_err(|_| ValidationError::LimitExceeded)?,
            )
        }
        Action::Scroll(value) => {
            delta(value.delta_1024, limits)?;
            let axis = match wire::ScrollAxis::try_from(value.axis) {
                Ok(wire::ScrollAxis::Vertical) => input::ScrollAxis::Vertical,
                Ok(wire::ScrollAxis::Horizontal) => input::ScrollAxis::Horizontal,
                _ => return Err(ValidationError::Unsupported),
            };
            I::Scroll(
                input::ScrollDelta::new(axis, value.delta_1024)
                    .map_err(|_| ValidationError::LimitExceeded)?,
            )
        }
        Action::MouseButtonTransition(value) => {
            let button = mapping::button(value.button)?;
            I::MouseButton(if down(value.state)? {
                input::ButtonAction::Down(button)
            } else {
                input::ButtonAction::Up(button)
            })
        }
        Action::MouseButtonClick(value) => {
            if value.click_count == 0 || value.click_count > limits.clicks {
                return Err(ValidationError::LimitExceeded);
            }
            let count =
                u8::try_from(value.click_count).map_err(|_| ValidationError::LimitExceeded)?;
            I::MouseButton(
                input::ButtonAction::click(mapping::button(value.button)?, count)
                    .map_err(|_| ValidationError::LimitExceeded)?,
            )
        }
        Action::PhysicalKeyTransition(value) => {
            let key = required(value.key)?;
            if key.usage_page != u32::from(input::PhysicalKey::USAGE_PAGE) {
                return Err(ValidationError::Unsupported);
            }
            let usage = u16::try_from(key.usage_id).map_err(|_| ValidationError::Unsupported)?;
            let key = input::PhysicalKey::new(usage).map_err(|_| ValidationError::Unsupported)?;
            I::PhysicalKey(if down(value.state)? {
                input::KeyAction::Down(key)
            } else {
                input::KeyAction::Up(key)
            })
        }
        Action::TextInsertion(value) => {
            *text_bytes = text_bytes
                .checked_add(value.text.len())
                .ok_or(ValidationError::LimitExceeded)?;
            if value.text.len() > limits.text_bytes() || *text_bytes > limits.aggregate_text_bytes()
            {
                return Err(ValidationError::LimitExceeded);
            }
            I::TextInsert(
                input::SensitiveText::new(value.text)
                    .map_err(|_| ValidationError::MalformedMessage)?,
            )
        }
        Action::TextEdit(value) => {
            repeat =
                usize::try_from(value.repeat_count).map_err(|_| ValidationError::LimitExceeded)?;
            if repeat == 0 || repeat > limits.operations() {
                return Err(ValidationError::LimitExceeded);
            }
            I::TextEdit(mapping::edit(value.command)?)
        }
        Action::LogicalCommand(value) => I::LogicalCommand(mapping::logical(value.command)?),
        Action::MediaCommand(value) => I::MediaCommand(mapping::media(value.command)?),
        _ => return Err(ValidationError::InvalidContext),
    };
    Ok((action, repeat))
}

fn delta(value: i32, limits: Limits) -> Result<(), ValidationError> {
    if value.unsigned_abs() > limits.delta {
        Err(ValidationError::LimitExceeded)
    } else {
        Ok(())
    }
}

fn down(value: i32) -> Result<bool, ValidationError> {
    match wire::TransitionState::try_from(value) {
        Ok(wire::TransitionState::Down) => Ok(true),
        Ok(wire::TransitionState::Up) => Ok(false),
        _ => Err(ValidationError::Unsupported),
    }
}

fn project(action: &input::InputAction, held: &mut HeldState) -> Result<(), ValidationError> {
    let valid = match action {
        input::InputAction::MouseButton(input::ButtonAction::Down(button)) => {
            held.buttons.insert(*button)
        }
        input::InputAction::MouseButton(input::ButtonAction::Up(button)) => {
            held.buttons.remove(button)
        }
        input::InputAction::MouseButton(input::ButtonAction::Click { button, .. }) => {
            !held.buttons.contains(button)
        }
        input::InputAction::PhysicalKey(input::KeyAction::Down(key)) => held.keys.insert(*key),
        input::InputAction::PhysicalKey(input::KeyAction::Up(key)) => held.keys.remove(key),
        _ => true,
    };
    if valid {
        Ok(())
    } else {
        Err(ValidationError::InvalidInputState)
    }
}
