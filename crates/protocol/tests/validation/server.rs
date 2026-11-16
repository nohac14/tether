use super::support::*;
use prost::Message;
use tether_input_core as input;
use tether_protocol::{validation::*, wire};

#[test]
fn hello_negotiates_limits_and_preserves_untrusted_metadata_without_debug_leaks() {
    let value = offer();
    assert_eq!(value.build_identifier(), "test-1");
    assert_eq!(value.untrusted_display_name(), "Untrusted phone");
    assert!(value.compose_text());
    assert_eq!(value.limits(), Limits::HARD);
    assert_eq!(format!("{value:?}"), "ValidatedHello(<redacted>)");
    let bytes = server_hello().encode_to_vec();
    let accepted = validate_server_hello(&bytes, &value).unwrap();
    assert_eq!(format!("{accepted:?}"), "ValidatedServer(<redacted>)");
    assert_eq!(accepted.envelope(), &server_hello());
    assert_eq!(accepted.into_envelope(), server_hello());
    let mut lower = Limits::HARD.to_wire();
    lower.maximum_operations_per_batch = 8;
    let result = validate_client_hello(
        &hello().encode_to_vec(),
        true,
        Limits::from_wire(&lower).unwrap(),
    )
    .unwrap();
    assert_eq!(result.limits().operations(), 8);
    assert!(validate_server_hello(&bytes, &result).is_err());
    let mut printable = hello();
    let Some(wire::client_envelope::Payload::ClientHello(inner)) = printable.payload.as_mut()
    else {
        panic!()
    };
    inner.build_identifier = " build ~".into();
    assert_eq!(
        validate_client_hello(&printable.encode_to_vec(), true, Limits::HARD)
            .unwrap()
            .build_identifier(),
        " build ~"
    );
}

#[test]
fn hello_rejects_incompatible_or_malformed_negotiation() {
    let original = hello();
    assert_eq!(
        validate_client_hello(&[], false, Limits::HARD).unwrap_err(),
        ValidationError::AuthenticationRequired
    );
    for change in 0..12 {
        let mut value = original.clone();
        let Some(wire::client_envelope::Payload::ClientHello(hello)) = value.payload.as_mut()
        else {
            panic!()
        };
        match change {
            0 => hello.protocol_major = 2,
            1 => hello.minimum_protocol_minor = 1,
            2 => hello.build_identifier.clear(),
            3 => hello.build_identifier = "not printable\n".into(),
            4 => hello.client_display_name.clear(),
            5 => hello.client_display_name = "control\0".into(),
            6 => hello.capabilities = vec![1],
            7 => hello.capabilities = vec![1, 2, 2],
            8 => hello.limits = None,
            9 => hello.limits.as_mut().unwrap().maximum_operations_per_batch = 0,
            10 => value.sequence = 1,
            _ => value.protocol_minor = 1,
        }
        assert!(validate_client_hello(&value.encode_to_vec(), true, Limits::HARD).is_err());
    }
    for change in 0..10 {
        let mut value = server_hello();
        let Some(wire::server_envelope::Payload::ServerHello(hello)) = value.payload.as_mut()
        else {
            panic!()
        };
        match change {
            0 => hello.selected_protocol_minor = 1,
            1 => hello.connection_identifier.pop().map(|_| ()).unwrap(),
            2 => hello.heartbeat_interval_millis = 0,
            3 => hello.heartbeat_timeout_millis = 4001,
            4 => hello.heartbeat_timeout_millis = 1000,
            5 => hello.capabilities = None,
            6 => hello.control_state = None,
            7 => hello.limits = None,
            8 => value.sequence = 1,
            _ => value.session_epoch = 7,
        }
        assert!(validate_server_hello(&value.encode_to_vec(), &offer()).is_err());
    }
}

#[test]
fn capability_conversion_roundtrips_partial_support_and_fails_closed_on_health() {
    let mut status = backend();
    let snapshot = capability_snapshot(&status, Limits::HARD).unwrap();
    assert_eq!(
        validate_capabilities(&snapshot, Limits::HARD, 1).unwrap(),
        status.capabilities
    );
    status.capabilities.atomic_click = input::CapabilityState::Unavailable;
    let snapshot = capability_snapshot(&status, Limits::HARD).unwrap();
    assert!(!snapshot.atomic_click_available);
    assert_eq!(snapshot.limits.unwrap().maximum_atomic_click_count, 2);
    for permission in [
        input::PermissionState::Denied,
        input::PermissionState::PromptRequired,
        input::PermissionState::TransientlyUnknown,
        input::PermissionState::Unavailable,
    ] {
        status.permission = permission;
        let snapshot = capability_snapshot(&status, Limits::HARD).unwrap();
        assert!(
            validate_capabilities(&snapshot, Limits::HARD, 1)
                .unwrap()
                .is_empty()
        );
    }
    status.permission = input::PermissionState::Granted;
    status.available = false;
    assert!(
        !capability_snapshot(&status, Limits::HARD)
            .unwrap()
            .pointer_available
    );
    status.available = true;
    status.fault = Some(input::BackendErrorCategory::NativeCallFailed);
    assert!(
        !capability_snapshot(&status, Limits::HARD)
            .unwrap()
            .physical_keys_available
    );
    status.revision = 0;
    assert!(capability_snapshot(&status, Limits::HARD).is_err());
}

#[test]
fn invalid_capability_lists_permission_claims_and_revision_regressions_fail() {
    let original = capability_snapshot(&backend(), Limits::HARD).unwrap();
    for change in 0..10 {
        let mut caps = original.clone();
        match change {
            0 => caps.revision = 0,
            1 => caps.mouse_buttons.push(1),
            2 => caps.text_modes.push(0),
            3 => caps.logical_commands.push(999),
            4 => caps.media_commands.push(1),
            5 => caps.backend_initialized = false,
            6 => caps.permission_state = wire::PermissionState::Denied.into(),
            7 => caps.user_authorization_required = true,
            8 => caps.mouse_buttons.clear(),
            _ => caps.limits = None,
        }
        assert!(validate_capabilities(&caps, Limits::HARD, 1).is_err());
    }
    assert!(validate_capabilities(&original, Limits::HARD, 2).is_err());
}

#[test]
fn server_runtime_messages_obey_sequence_epoch_revision_and_control_coherence() {
    let ctx = server_context();
    for payload in [
        wire::server_envelope::Payload::HeartbeatPong(wire::HeartbeatPong {
            heartbeat_identifier: 42,
        }),
        wire::server_envelope::Payload::CapabilitySnapshot(
            capability_snapshot(&backend(), Limits::HARD).unwrap(),
        ),
        wire::server_envelope::Payload::ControlState(control()),
        wire::server_envelope::Payload::ProtocolError(ValidationError::MalformedMessage.to_wire()),
    ] {
        let original = server(payload);
        assert!(validate_server(&original.encode_to_vec(), &ctx).is_ok());
        let mut value = original.clone();
        value.sequence = 2;
        assert_eq!(
            validate_server(&value.encode_to_vec(), &ctx),
            Err(ValidationError::InvalidSequence)
        );
        value = original;
        value.session_epoch = 6;
        assert_eq!(
            validate_server(&value.encode_to_vec(), &ctx),
            Err(ValidationError::StaleEpoch)
        );
    }
    let mut newer = control();
    newer.revision = 2;
    newer.session_epoch = 8;
    let mut envelope = server(wire::server_envelope::Payload::ControlState(newer));
    envelope.session_epoch = 8;
    assert!(validate_server(&envelope.encode_to_vec(), &ctx).is_ok());
    newer.this_client_controls = false;
    envelope.payload = Some(wire::server_envelope::Payload::ControlState(newer));
    assert!(validate_server(&envelope.encode_to_vec(), &ctx).is_err());
    let mut initial = control();
    initial.state = wire::ControlStateKind::NoController.into();
    initial.this_client_controls = false;
    initial.session_epoch = 0;
    let mut envelope = server(wire::server_envelope::Payload::ControlState(initial));
    envelope.session_epoch = 0;
    let mut ctx = ctx;
    ctx.epoch = 0;
    assert!(validate_server(&envelope.encode_to_vec(), &ctx).is_ok());
    initial.state = wire::ControlStateKind::Standby.into();
    envelope.payload = Some(wire::server_envelope::Payload::ControlState(initial));
    assert!(validate_server(&envelope.encode_to_vec(), &ctx).is_err());
}

#[test]
fn errors_are_closed_and_optional_numeric_context_has_no_ambiguous_enum() {
    let ctx = server_context();
    for code in 1..=14 {
        let message = wire::ProtocolError {
            code,
            ..Default::default()
        };
        assert!(
            validate_server(
                &server(wire::server_envelope::Payload::ProtocolError(message)).encode_to_vec(),
                &ctx
            )
            .is_ok()
        );
    }
    for error in [
        wire::ProtocolError::default(),
        wire::ProtocolError {
            code: 1,
            supported_minimum_minor: Some(0),
            ..Default::default()
        },
        wire::ProtocolError {
            code: 3,
            supported_minimum_minor: Some(2),
            supported_maximum_minor: Some(1),
            ..Default::default()
        },
        wire::ProtocolError {
            code: 1,
            numeric_context: Some(wire::SafeNumericContext::default()),
            ..Default::default()
        },
    ] {
        assert!(
            validate_server(
                &server(wire::server_envelope::Payload::ProtocolError(error)).encode_to_vec(),
                &ctx
            )
            .is_err()
        );
    }
    let error = wire::ProtocolError {
        code: 3,
        supported_minimum_minor: Some(0),
        supported_maximum_minor: Some(0),
        numeric_context: Some(wire::SafeNumericContext {
            kind: 1,
            observed: 99,
            limit: 50,
        }),
        ..Default::default()
    };
    assert!(
        validate_server(
            &server(wire::server_envelope::Payload::ProtocolError(error)).encode_to_vec(),
            &ctx
        )
        .is_ok()
    );
    for error in [
        ValidationError::MalformedMessage,
        ValidationError::Unsupported,
        ValidationError::AuthenticationRequired,
        ValidationError::InvalidTimestamp,
        ValidationError::StaleEpoch,
        ValidationError::InvalidContext,
        ValidationError::MessageTooLarge,
    ] {
        assert!(!format!("{error}").contains("test-only-secret"));
        assert!(!error.to_wire().retryable);
        assert!([1008, 1009, 4001, 4002, 4003, 1011].contains(&error.close_code()));
    }
}

#[test]
fn limits_are_positive_bounded_consistent_and_roundtrip() {
    assert_eq!(
        Limits::from_wire(&Limits::HARD.to_wire()).unwrap(),
        Limits::HARD
    );
    for value in [0, u32::MAX] {
        for index in 0..6 {
            let mut limits = Limits::HARD.to_wire();
            match index {
                0 => limits.maximum_encoded_message_bytes = value,
                1 => limits.maximum_operations_per_batch = value,
                2 => limits.maximum_text_operation_bytes = value,
                3 => limits.maximum_text_bytes_per_batch = value,
                4 => limits.maximum_absolute_fixed_delta = value,
                _ => limits.maximum_atomic_click_count = value,
            }
            assert_eq!(
                Limits::from_wire(&limits),
                Err(ValidationError::InvalidLimits)
            );
        }
    }
    let mut limits = Limits::HARD.to_wire();
    limits.maximum_text_bytes_per_batch = 1;
    assert_eq!(
        Limits::from_wire(&limits),
        Err(ValidationError::InvalidLimits)
    );
}
