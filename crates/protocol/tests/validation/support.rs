use prost::Message;
use tether_input_core as input;
use tether_protocol::{validation::*, wire};

pub fn backend() -> input::BackendStatus {
    let mut caps = input::BackendCapabilities::unavailable();
    caps.relative_pointer = input::CapabilityState::Available;
    caps.atomic_click = input::CapabilityState::Available;
    caps.vertical_scroll = input::CapabilityState::Available;
    caps.horizontal_scroll = input::CapabilityState::Available;
    caps.physical_keys = input::CapabilityState::Available;
    caps.mouse_buttons.extend([
        input::MouseButton::Left,
        input::MouseButton::Right,
        input::MouseButton::Middle,
        input::MouseButton::Back,
        input::MouseButton::Forward,
    ]);
    caps.text_modes.extend([
        input::TextMode::ActiveLayoutMappable,
        input::TextMode::NativeUnicodeBestEffort,
        input::TextMode::EiUtf8,
    ]);
    caps.logical_commands.extend([
        input::LogicalCommand::Copy,
        input::LogicalCommand::Cut,
        input::LogicalCommand::Paste,
        input::LogicalCommand::Undo,
        input::LogicalCommand::Redo,
        input::LogicalCommand::SelectAll,
        input::LogicalCommand::Save,
        input::LogicalCommand::Find,
        input::LogicalCommand::CloseTab,
        input::LogicalCommand::SwitchApplication,
        input::LogicalCommand::TaskManagerEquivalent,
    ]);
    caps.media_commands.extend([
        input::MediaCommand::PlayPause,
        input::MediaCommand::NextTrack,
        input::MediaCommand::PreviousTrack,
        input::MediaCommand::VolumeUp,
        input::MediaCommand::VolumeDown,
        input::MediaCommand::Mute,
    ]);
    input::BackendStatus {
        revision: 1,
        kind: input::BackendKind::Mock,
        initialized: true,
        available: true,
        permission: input::PermissionState::NotRequired,
        capabilities: caps,
        fault: None,
    }
}

pub fn context<'a>(backend: &'a input::BackendStatus, held: &'a HeldState) -> ClientContext<'a> {
    ClientContext {
        authenticated: true,
        controls_input: true,
        epoch: 7,
        next_sequence: 1,
        last_timestamp_micros: 100,
        last_heartbeat_identifier: None,
        limits: Limits::HARD,
        backend,
        held,
    }
}

pub fn operation(action: wire::client_operation::Action) -> wire::ClientOperation {
    wire::ClientOperation {
        timestamp_offset_micros: 0,
        action: Some(action),
    }
}

pub fn envelope(operations: Vec<wire::ClientOperation>) -> wire::ClientEnvelope {
    wire::ClientEnvelope {
        protocol_major: 1,
        sequence: 1,
        session_epoch: 7,
        client_timestamp_micros: 100,
        payload: Some(wire::client_envelope::Payload::OperationBatch(
            wire::ClientOperationBatch { operations },
        )),
        ..Default::default()
    }
}

pub fn pointer() -> wire::ClientOperation {
    operation(wire::client_operation::Action::PointerMove(
        wire::PointerMove {
            delta_x_1024: 1024,
            delta_y_1024: -512,
        },
    ))
}

pub fn button(state: i32) -> wire::ClientOperation {
    operation(wire::client_operation::Action::MouseButtonTransition(
        wire::MouseButtonTransition { button: 1, state },
    ))
}

pub fn key(state: i32) -> wire::ClientOperation {
    operation(wire::client_operation::Action::PhysicalKeyTransition(
        wire::PhysicalKeyTransition {
            key: Some(wire::PhysicalKey {
                usage_page: 7,
                usage_id: 4,
            }),
            state,
        },
    ))
}

pub fn text(value: &str) -> wire::ClientOperation {
    operation(wire::client_operation::Action::TextInsertion(
        wire::TextInsertion { text: value.into() },
    ))
}

pub fn hello() -> wire::ClientEnvelope {
    wire::ClientEnvelope {
        protocol_major: 1,
        payload: Some(wire::client_envelope::Payload::ClientHello(
            wire::ClientHello {
                protocol_major: 1,
                minimum_protocol_minor: 0,
                maximum_protocol_minor: 5,
                build_identifier: "test-1".into(),
                client_display_name: "Untrusted phone".into(),
                limits: Some(wire::ClientLimits {
                    maximum_encoded_message_bytes: 65536,
                    maximum_operations_per_batch: 256,
                    maximum_text_operation_bytes: 4096,
                }),
                capabilities: vec![1, 2, 3],
            },
        )),
        ..Default::default()
    }
}

pub fn offer() -> ValidatedHello {
    validate_client_hello(&hello().encode_to_vec(), true, Limits::HARD).unwrap()
}

pub fn control() -> wire::ControlState {
    wire::ControlState {
        revision: 1,
        state: wire::ControlStateKind::Controlling.into(),
        session_epoch: 7,
        this_client_controls: true,
    }
}

pub fn server(payload: wire::server_envelope::Payload) -> wire::ServerEnvelope {
    wire::ServerEnvelope {
        protocol_major: 1,
        sequence: 1,
        session_epoch: 7,
        payload: Some(payload),
        ..Default::default()
    }
}

pub fn server_context() -> ServerContext {
    ServerContext {
        next_sequence: 1,
        epoch: 7,
        capability_revision: 1,
        control_revision: 1,
        limits: Limits::HARD,
    }
}

pub fn server_hello() -> wire::ServerEnvelope {
    wire::ServerEnvelope {
        protocol_major: 1,
        payload: Some(wire::server_envelope::Payload::ServerHello(
            wire::ServerHello {
                selected_protocol_major: 1,
                connection_identifier: vec![0x42; 16],
                heartbeat_interval_millis: 1000,
                heartbeat_timeout_millis: 4000,
                limits: Some(Limits::HARD.to_wire()),
                capabilities: Some(capability_snapshot(&backend(), Limits::HARD).unwrap()),
                control_state: Some(control()),
                ..Default::default()
            },
        )),
        ..Default::default()
    }
}

/// Test-only protobuf constructors; not a second production schema or codec.
pub fn varint(mut value: u64) -> Vec<u8> {
    let mut result = Vec::new();
    while value > 0x7f {
        result.push(u8::try_from(value & 0x7f).unwrap() | 0x80);
        value >>= 7;
    }
    result.push(u8::try_from(value).unwrap());
    result
}

pub fn scalar(field: u32, value: u64) -> Vec<u8> {
    let mut result = varint(u64::from(field) << 3);
    result.extend(varint(value));
    result
}

pub fn nested(field: u32, bytes: &[u8]) -> Vec<u8> {
    let mut result = varint((u64::from(field) << 3) | 2);
    result.extend(varint(bytes.len().try_into().unwrap()));
    result.extend(bytes);
    result
}
