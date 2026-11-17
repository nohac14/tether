use super::{Case, Envelope};
use crate::support::*;
use prost::Message;
use tether_protocol::{validation::*, wire};
use wire::client_operation::Action as A;

pub fn cases() -> Vec<Case> {
    let mut cases = vec![
        Case::new("client-hello", true, Envelope::Client(hello()), Ok(())),
        Case::new(
            "server-hello",
            true,
            Envelope::Server(server_hello()),
            Ok(()),
        ),
    ];
    add_actions(&mut cases);
    add_server(&mut cases);
    add_boundaries(&mut cases);
    add_unknown(&mut cases);
    cases
}

fn add_actions(out: &mut Vec<Case>) {
    let mut actions = vec![
        ("pointer".into(), vec![pointer()]),
        (
            "button-transitions".into(),
            vec![button(1), pointer(), button(2)],
        ),
        ("key-transitions".into(), vec![key(1), key(2)]),
        (
            "text-unicode".into(),
            vec![text("synthetic 🙂e\u{301}\n日本")],
        ),
        (
            "request-control".into(),
            vec![operation(A::RequestControl(wire::RequestControl {}))],
        ),
        (
            "relinquish-control".into(),
            vec![operation(A::RelinquishControl(wire::RelinquishControl {}))],
        ),
        (
            "release-all".into(),
            vec![operation(A::ReleaseAll(wire::ReleaseAll {}))],
        ),
    ];
    for axis in 1..=2 {
        actions.push((
            format!("scroll-{axis}"),
            vec![operation(A::Scroll(wire::Scroll {
                axis,
                delta_1024: -1025,
            }))],
        ));
    }
    for button in 1..=5 {
        actions.push((
            format!("click-{button}"),
            vec![operation(A::MouseButtonClick(wire::MouseButtonClick {
                button,
                click_count: 2,
            }))],
        ));
    }
    for command in 1..=3 {
        actions.push((
            format!("edit-{command}"),
            vec![operation(A::TextEdit(wire::TextEdit {
                command,
                repeat_count: 2,
            }))],
        ));
    }
    for command in 1..=11 {
        actions.push((
            format!("logical-{command}"),
            vec![operation(A::LogicalCommand(wire::LogicalCommandAction {
                command,
            }))],
        ));
    }
    for command in 1..=6 {
        actions.push((
            format!("media-{command}"),
            vec![operation(A::MediaCommand(wire::MediaCommandAction {
                command,
            }))],
        ));
    }
    for (id, actions) in actions {
        out.push(Case::new(
            id,
            false,
            Envelope::Client(envelope(actions)),
            Ok(()),
        ));
    }
    add_liveness(out);
}

fn add_liveness(out: &mut Vec<Case>) {
    let mut ping = envelope(vec![]);
    ping.payload = Some(wire::client_envelope::Payload::HeartbeatPing(
        wire::HeartbeatPing {
            heartbeat_identifier: 42,
        },
    ));
    out.push(Case::new(
        "heartbeat-ping",
        false,
        Envelope::Client(ping.clone()),
        Ok(()),
    ));
    ping.sequence = u64::MAX;
    ping.session_epoch = u64::MAX;
    ping.client_timestamp_micros = u64::MAX;
    ping.payload = Some(wire::client_envelope::Payload::HeartbeatPing(
        wire::HeartbeatPing {
            heartbeat_identifier: u64::MAX,
        },
    ));
    out.push(Case::new(
        "full-width-counters",
        false,
        Envelope::Client(ping),
        Ok(()),
    ));
    for reason in 1..=3 {
        let mut leaving = envelope(vec![]);
        leaving.payload = Some(wire::client_envelope::Payload::ClientLeaving(
            wire::ClientLeaving { reason },
        ));
        out.push(Case::new(
            format!("leaving-{reason}"),
            false,
            Envelope::Client(leaving),
            Ok(()),
        ));
    }
}

fn add_server(out: &mut Vec<Case>) {
    out.push(Case::new(
        "heartbeat-pong",
        false,
        Envelope::Server(server(wire::server_envelope::Payload::HeartbeatPong(
            wire::HeartbeatPong {
                heartbeat_identifier: 42,
            },
        ))),
        Ok(()),
    ));
    out.push(Case::new(
        "capability-snapshot",
        false,
        Envelope::Server(server(wire::server_envelope::Payload::CapabilitySnapshot(
            capability_snapshot(&backend(), Limits::HARD).unwrap(),
        ))),
        Ok(()),
    ));
    let mut partial = capability_snapshot(&backend(), Limits::HARD).unwrap();
    partial.atomic_click_available = false;
    out.push(Case::new(
        "capability-without-atomic-click",
        false,
        Envelope::Server(server(wire::server_envelope::Payload::CapabilitySnapshot(
            partial,
        ))),
        Ok(()),
    ));
    out.push(Case::new(
        "control-state",
        false,
        Envelope::Server(server(wire::server_envelope::Payload::ControlState(
            control(),
        ))),
        Ok(()),
    ));
    for code in 1..=14 {
        let error = wire::ProtocolError {
            code,
            retryable: false,
            ..Default::default()
        };
        out.push(Case::new(
            format!("error-{code}"),
            false,
            Envelope::Server(server(wire::server_envelope::Payload::ProtocolError(error))),
            Ok(()),
        ));
    }
    let error = wire::ProtocolError {
        code: 3,
        supported_minimum_minor: Some(0),
        supported_maximum_minor: Some(0),
        numeric_context: Some(wire::SafeNumericContext {
            kind: 1,
            observed: 65537,
            limit: 65536,
        }),
        ..Default::default()
    };
    out.push(Case::new(
        "error-optional-presence",
        false,
        Envelope::Server(server(wire::server_envelope::Payload::ProtocolError(error))),
        Ok(()),
    ));
}

fn add_boundaries(out: &mut Vec<Case>) {
    for (id, operations, admission) in [
        ("text-maximum", vec![text(&"é".repeat(2048))], Ok(())),
        (
            "text-oversize",
            vec![text(&"x".repeat(4097))],
            Err(ValidationError::LimitExceeded),
        ),
        ("batch-maximum", vec![pointer(); 256], Ok(())),
        (
            "batch-oversize",
            vec![pointer(); 257],
            Err(ValidationError::LimitExceeded),
        ),
        (
            "aggregate-text-maximum",
            vec![text(&"x".repeat(4096)); 4],
            Ok(()),
        ),
        (
            "aggregate-text-oversize",
            vec![text(&"x".repeat(4096)); 5],
            Err(ValidationError::LimitExceeded),
        ),
    ] {
        out.push(Case::new(
            id,
            false,
            Envelope::Client(envelope(operations)),
            admission,
        ));
    }
    for (id, major, minor) in [("incompatible-major", 2, 0), ("incompatible-minor", 1, 1)] {
        let mut value = envelope(vec![pointer()]);
        value.protocol_major = major;
        value.protocol_minor = minor;
        out.push(Case::new(
            id,
            false,
            Envelope::Client(value),
            Err(ValidationError::IncompatibleVersion),
        ));
    }
    for size in [65536, 65537] {
        out.push(padded(size));
    }
}

fn padded(size: usize) -> Case {
    let mut value = envelope(vec![]);
    value.payload = Some(wire::client_envelope::Payload::ClientLeaving(
        wire::ClientLeaving { reason: 1 },
    ));
    let expected = Envelope::Client(value.clone());
    value.payload = None;
    for length in size - 64..size {
        let body = [
            wire::ClientLeaving { reason: 1 }.encode_to_vec(),
            nested(100, &vec![0; length]),
        ]
        .concat();
        let bytes = [value.encode_to_vec(), nested(13, &body)].concat();
        if bytes.len() == size {
            return Case {
                id: format!("message-{size}"),
                hello: false,
                bytes,
                expected: Some(expected),
                admission: if size == 65536 {
                    Ok(())
                } else {
                    Err(ValidationError::MessageTooLarge)
                },
            };
        }
    }
    panic!("could not construct exact-size synthetic fixture");
}

fn add_unknown(out: &mut Vec<Case>) {
    let unknown = envelope(vec![operation(A::MediaCommand(wire::MediaCommandAction {
        command: 999,
    }))]);
    out.push(Case::new(
        "unknown-enum",
        false,
        Envelope::Client(unknown),
        Err(ValidationError::Unsupported),
    ));
    let mut unknown = Case::new(
        "unknown-nested-field",
        false,
        Envelope::Client(envelope(vec![pointer()])),
        Ok(()),
    );
    let move_body = [
        wire::PointerMove {
            delta_x_1024: 1024,
            delta_y_1024: -512,
        }
        .encode_to_vec(),
        scalar(100, 7),
    ]
    .concat();
    let mut header = envelope(vec![]);
    header.payload = None;
    unknown.bytes = [
        header.encode_to_vec(),
        nested(11, &nested(1, &nested(10, &move_body))),
    ]
    .concat();
    out.push(unknown);
    let mut unknown = Case::new(
        "unknown-union-field",
        false,
        Envelope::Client(envelope(vec![pointer()])),
        Err(ValidationError::Unsupported),
    );
    unknown.bytes.extend(nested(99, &[]));
    out.push(unknown);
    let mut duplicate = Case::new(
        "duplicate-sequence",
        false,
        Envelope::Client(envelope(vec![pointer()])),
        Err(ValidationError::MalformedMessage),
    );
    duplicate.bytes.extend(scalar(3, 1));
    out.push(duplicate);
    let mut expected = header.clone();
    expected.payload = Some(wire::client_envelope::Payload::HeartbeatPing(
        wire::HeartbeatPing {
            heartbeat_identifier: 42,
        },
    ));
    let mut duplicate = Case::new(
        "duplicate-payload",
        false,
        Envelope::Client(expected),
        Err(ValidationError::MalformedMessage),
    );
    duplicate.bytes = envelope(vec![pointer()]).encode_to_vec();
    duplicate.bytes.extend(nested(
        12,
        &wire::HeartbeatPing {
            heartbeat_identifier: 42,
        }
        .encode_to_vec(),
    ));
    out.push(duplicate);
    out.push(Case {
        id: "truncated".into(),
        hello: false,
        bytes: vec![0x80],
        expected: None,
        admission: Err(ValidationError::MalformedMessage),
    });
}
