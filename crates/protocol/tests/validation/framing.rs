use super::support::*;
use prost::Message;
use tether_protocol::{validation::*, wire};

#[test]
fn ambiguous_payloads_unknown_actions_and_duplicate_scalars_are_not_last_one_wins() {
    let backend = backend();
    let held = HeldState::default();
    let ctx = context(&backend, &held);
    let original = envelope(vec![pointer()]).encode_to_vec();
    let mut duplicate = original.clone();
    duplicate.extend(nested(
        12,
        &wire::HeartbeatPing {
            heartbeat_identifier: 1,
        }
        .encode_to_vec(),
    ));
    // Prost alone silently accepts this complete duplicate-oneof encoding.
    assert!(wire::ClientEnvelope::decode(duplicate.as_slice()).is_ok());
    assert_eq!(
        validate_client(&duplicate, &ctx),
        Err(ValidationError::MalformedMessage)
    );
    let mut scalar_duplicate = original.clone();
    scalar_duplicate.extend(scalar(3, 1));
    assert_eq!(
        validate_client(&scalar_duplicate, &ctx),
        Err(ValidationError::MalformedMessage)
    );
    let mut unknown = original.clone();
    unknown.extend(nested(50, &[]));
    assert_eq!(
        validate_client(&unknown, &ctx),
        Err(ValidationError::Unsupported)
    );
    let mut encoded_operation = pointer().encode_to_vec();
    encoded_operation.extend(nested(99, &[]));
    let mut headers = envelope(vec![]);
    headers.payload = None;
    let mut bytes = headers.encode_to_vec();
    bytes.extend(nested(11, &nested(1, &encoded_operation)));
    assert_eq!(
        validate_client(&bytes, &ctx),
        Err(ValidationError::Unsupported)
    );
    let duplicate_action = [pointer().encode_to_vec(), key(1).encode_to_vec()].concat();
    let mut bytes = headers.encode_to_vec();
    bytes.extend(nested(11, &nested(1, &duplicate_action)));
    assert_eq!(
        validate_client(&bytes, &ctx),
        Err(ValidationError::MalformedMessage)
    );
}

#[test]
fn width_overflow_bool_utf8_truncation_wire_type_and_group_attacks_fail() {
    let backend = backend();
    let held = HeldState::default();
    let ctx = context(&backend, &held);
    let mut header = envelope(vec![]);
    header.payload = None;
    for invalid in [
        nested(15, &nested(1, &[0xff])),
        nested(10, &scalar(1, u64::from(u32::MAX) + 1)),
        nested(11, &scalar(1, (1_u64 << 32) + 1)),
        vec![0x53],       // deprecated group inside operation
        vec![0x52, 0x80], // truncated message length
    ] {
        let mut bytes = header.encode_to_vec();
        bytes.extend(nested(11, &nested(1, &invalid)));
        assert!(validate_client(&bytes, &ctx).is_err());
    }
    for bytes in [
        vec![0],
        vec![0xff; 11],
        vec![0x0a, 0],
        vec![0x08, 0x80],
        vec![0x08, 0xff, 0xff, 0xff, 0xff, 0x10],
    ] {
        assert!(validate_client(&bytes, &ctx).is_err());
    }
    let mut caps = capability_snapshot(&backend, Limits::HARD).unwrap();
    caps.pointer_available = false;
    let mut snapshot = caps.encode_to_vec();
    // Duplicate or noncanonical boolean must not be coerced into `true`.
    snapshot.extend(scalar(5, 2));
    let mut value = server(wire::server_envelope::Payload::HeartbeatPong(
        wire::HeartbeatPong::default(),
    ));
    value.payload = None;
    let mut bytes = value.encode_to_vec();
    bytes.extend(nested(12, &snapshot));
    assert!(validate_server(&bytes, &server_context()).is_err());
}

#[test]
fn preallocation_limits_reject_tiny_repeated_messages_and_large_text_or_metadata() {
    let backend = backend();
    let held = HeldState::default();
    let ctx = context(&backend, &held);
    let mut header = envelope(vec![]);
    header.payload = None;
    let mut bytes = header.encode_to_vec();
    bytes.extend(nested(11, &nested(1, &[]).repeat(257)));
    assert_eq!(
        validate_client(&bytes, &ctx),
        Err(ValidationError::LimitExceeded)
    );
    assert_eq!(
        validate_client(
            &envelope(vec![text(&"x".repeat(4097))]).encode_to_vec(),
            &ctx
        ),
        Err(ValidationError::LimitExceeded)
    );
    assert_eq!(
        validate_client(&vec![0; 65537], &ctx),
        Err(ValidationError::MessageTooLarge)
    );
    let mut value = hello();
    let Some(wire::client_envelope::Payload::ClientHello(inner)) = value.payload.as_mut() else {
        panic!()
    };
    inner.client_display_name = "x".repeat(129);
    assert_eq!(
        validate_client_hello(&value.encode_to_vec(), true, Limits::HARD).unwrap_err(),
        ValidationError::LimitExceeded
    );
    let mut snapshot = capability_snapshot(&backend, Limits::HARD)
        .unwrap()
        .encode_to_vec();
    snapshot.extend(nested(10, &vec![1; 5000]));
    let mut value = server(wire::server_envelope::Payload::HeartbeatPong(
        wire::HeartbeatPong::default(),
    ));
    value.payload = None;
    let mut bytes = value.encode_to_vec();
    bytes.extend(nested(12, &snapshot));
    assert_eq!(
        validate_server(&bytes, &server_context()),
        Err(ValidationError::LimitExceeded)
    );
}

#[test]
fn packed_unpacked_split_enum_fields_and_arbitrary_field_order_are_accepted() {
    let mut envelope = hello();
    let Some(wire::client_envelope::Payload::ClientHello(mut inner)) = envelope.payload.take()
    else {
        panic!()
    };
    inner.capabilities.clear();
    for extra in [
        [scalar(7, 1), scalar(7, 2), scalar(7, 3)].concat(),
        [nested(7, &[1]), nested(7, &[2, 3])].concat(),
        [scalar(7, 1), nested(7, &[2, 3])].concat(),
    ] {
        let mut body = extra;
        body.extend(inner.encode_to_vec());
        let mut bytes = nested(10, &body);
        bytes.extend(envelope.encode_to_vec());
        assert!(validate_client_hello(&bytes, true, Limits::HARD).is_ok());
    }
}

#[test]
fn bounded_unknown_nested_fields_are_compatible_but_visit_budget_is_enforced() {
    let backend = backend();
    let held = HeldState::default();
    let ctx = context(&backend, &held);
    let mut header = envelope(vec![]);
    header.payload = None;
    let mut move_body = wire::PointerMove {
        delta_x_1024: 1,
        delta_y_1024: 0,
    }
    .encode_to_vec();
    for unknown in [
        scalar(100, 1),
        nested(101, b"opaque"),
        [varint((102 << 3) | 1), vec![0; 8]].concat(),
        [varint((103 << 3) | 5), vec![0; 4]].concat(),
    ] {
        move_body.extend(unknown);
    }
    let mut bytes = header.encode_to_vec();
    bytes.extend(nested(11, &nested(1, &nested(10, &move_body))));
    assert!(validate_client(&bytes, &ctx).is_ok());
    move_body.extend(scalar(100, 1).repeat(4096));
    let mut bytes = header.encode_to_vec();
    bytes.extend(nested(11, &nested(1, &nested(10, &move_body))));
    assert_eq!(
        validate_client(&bytes, &ctx),
        Err(ValidationError::LimitExceeded)
    );
}
