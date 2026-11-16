use super::support::*;
use prost::Message;
use tether_input_core as input;
use tether_protocol::{validation::*, wire};
use wire::client_operation::Action;

#[test]
fn every_input_family_maps_in_order_with_edit_expansion() {
    let backend = backend();
    let held = HeldState::default();
    let mut operations = vec![pointer(), button(1), pointer(), button(2), key(1), key(2)];
    for axis in [1, 2] {
        operations.push(operation(Action::Scroll(wire::Scroll {
            axis,
            delta_1024: -1,
        })));
    }
    for button in 1..=5 {
        operations.push(operation(Action::MouseButtonClick(
            wire::MouseButtonClick {
                button,
                click_count: 2,
            },
        )));
    }
    operations.push(text("🙂\0e\u{301}日本"));
    for command in 1..=3 {
        operations.push(operation(Action::TextEdit(wire::TextEdit {
            command,
            repeat_count: 2,
        })));
    }
    for command in 1..=11 {
        operations.push(operation(Action::LogicalCommand(
            wire::LogicalCommandAction { command },
        )));
    }
    for command in 1..=6 {
        operations.push(operation(Action::MediaCommand(wire::MediaCommandAction {
            command,
        })));
    }
    for (index, value) in operations.iter_mut().enumerate() {
        value.timestamp_offset_micros = u32::try_from(index).unwrap();
    }
    let validated = validate_client(
        &envelope(operations).encode_to_vec(),
        &context(&backend, &held),
    )
    .unwrap();
    assert_eq!(validated.sequence(), 1);
    assert_eq!(validated.epoch(), 7);
    let ClientMessage::Batch(actions) = validated.message() else {
        panic!("batch required")
    };
    assert_eq!(actions.len(), 37);
    assert_eq!(
        actions[0].operation(),
        &Operation::Input(input::InputAction::PointerMove(
            input::PointerDelta::new(1024, -512).unwrap()
        ))
    );
    assert_eq!(
        actions[1].operation(),
        &Operation::Input(input::InputAction::MouseButton(input::ButtonAction::Down(
            input::MouseButton::Left
        )))
    );
    assert_eq!(
        actions[4].operation(),
        &Operation::Input(input::InputAction::PhysicalKey(input::KeyAction::Down(
            input::PhysicalKey::new(4).unwrap()
        )))
    );
    assert_eq!(
        actions[6].operation(),
        &Operation::Input(input::InputAction::Scroll(
            input::ScrollDelta::new(input::ScrollAxis::Vertical, -1).unwrap()
        ))
    );
    assert_eq!(
        actions[7].operation(),
        &Operation::Input(input::InputAction::Scroll(
            input::ScrollDelta::new(input::ScrollAxis::Horizontal, -1).unwrap()
        ))
    );
    assert_eq!(actions[14].operation(), actions[15].operation());
    assert!(
        actions
            .windows(2)
            .all(|pair| pair[0].timestamp_micros() <= pair[1].timestamp_micros())
    );
    assert_eq!(held, HeldState::default());
    assert!(matches!(validated.into_message(), ClientMessage::Batch(_)));
}

#[test]
fn held_state_checks_are_atomic_and_do_not_mutate_the_authoritative_ledger() {
    let backend = backend();
    let held = HeldState::default();
    let ctx = context(&backend, &held);
    let click = operation(Action::MouseButtonClick(wire::MouseButtonClick {
        button: 1,
        click_count: 1,
    }));
    for invalid in [
        vec![button(1), button(1)],
        vec![button(2)],
        vec![key(1), key(1)],
        vec![key(2)],
        vec![button(1), click.clone()],
        vec![button(1), text("")],
    ] {
        assert!(validate_client(&envelope(invalid).encode_to_vec(), &ctx).is_err());
        assert_eq!(held, HeldState::default());
    }
    assert!(
        validate_client(
            &envelope(vec![
                button(1),
                pointer(),
                button(2),
                click.clone(),
                key(1),
                key(2)
            ])
            .encode_to_vec(),
            &ctx
        )
        .is_ok()
    );
    let mut already_held = held.clone();
    already_held.buttons.insert(input::MouseButton::Left);
    let before = already_held.clone();
    assert_eq!(
        validate_client(
            &envelope(vec![click]).encode_to_vec(),
            &context(&backend, &already_held)
        ),
        Err(ValidationError::InvalidInputState)
    );
    assert_eq!(already_held, before);
    assert!(
        validate_client(
            &envelope(vec![button(2)]).encode_to_vec(),
            &context(&backend, &already_held)
        )
        .is_ok()
    );
    assert_eq!(already_held, before);
}

#[test]
fn ownership_barriers_are_terminal_and_standby_cannot_inject_or_release_another_owner() {
    let backend = backend();
    let held = HeldState::default();
    let mut ctx = context(&backend, &held);
    for action in [
        Action::RequestControl(wire::RequestControl {}),
        Action::RelinquishControl(wire::RelinquishControl {}),
        Action::ReleaseAll(wire::ReleaseAll {}),
    ] {
        assert!(
            validate_client(
                &envelope(vec![pointer(), operation(action.clone())]).encode_to_vec(),
                &ctx
            )
            .is_ok()
        );
        assert_eq!(
            validate_client(
                &envelope(vec![operation(action), pointer()]).encode_to_vec(),
                &ctx
            ),
            Err(ValidationError::InvalidInputState)
        );
    }
    ctx.controls_input = false;
    for op in [
        pointer(),
        operation(Action::ReleaseAll(wire::ReleaseAll {})),
        operation(Action::RelinquishControl(wire::RelinquishControl {})),
    ] {
        assert_eq!(
            validate_client(&envelope(vec![op]).encode_to_vec(), &ctx),
            Err(ValidationError::NotController)
        );
    }
    let mut request = envelope(vec![operation(Action::RequestControl(
        wire::RequestControl {},
    ))]);
    request.session_epoch = 0;
    ctx.epoch = 0;
    assert!(validate_client(&request.encode_to_vec(), &ctx).is_ok());
    ctx.controls_input = true;
    assert_eq!(
        validate_client(&request.encode_to_vec(), &ctx),
        Err(ValidationError::InvalidContext)
    );
}

#[test]
fn revoked_capabilities_backend_health_and_authentication_fail_closed() {
    let mut backend = backend();
    let held = HeldState::default();
    let bytes = envelope(vec![pointer()]).encode_to_vec();
    let mut ctx = context(&backend, &held);
    ctx.authenticated = false;
    assert_eq!(
        validate_client(&bytes, &ctx),
        Err(ValidationError::AuthenticationRequired)
    );
    backend.capabilities.relative_pointer = input::CapabilityState::Unavailable;
    assert_eq!(
        validate_client(&bytes, &context(&backend, &held)),
        Err(ValidationError::CapabilityUnavailable)
    );
    backend.capabilities.relative_pointer = input::CapabilityState::Available;
    for permission in [
        input::PermissionState::Denied,
        input::PermissionState::PromptRequired,
        input::PermissionState::TransientlyUnknown,
        input::PermissionState::Unavailable,
    ] {
        backend.permission = permission;
        assert_eq!(
            validate_client(&bytes, &context(&backend, &held)),
            Err(ValidationError::BackendUnavailable)
        );
    }
    backend.permission = input::PermissionState::NotRequired;
    backend.available = false;
    assert_eq!(
        validate_client(&bytes, &context(&backend, &held)),
        Err(ValidationError::BackendUnavailable)
    );
    backend.available = true;
    backend.fault = Some(input::BackendErrorCategory::NativeCallFailed);
    assert_eq!(
        validate_client(&bytes, &context(&backend, &held)),
        Err(ValidationError::BackendUnavailable)
    );
}

#[test]
fn sequence_epoch_and_timestamp_boundaries_do_not_wrap() {
    let backend = backend();
    let held = HeldState::default();
    let mut ctx = context(&backend, &held);
    let mut message = envelope(vec![pointer()]);
    for sequence in [0, 2, u64::MAX] {
        message.sequence = sequence;
        assert_eq!(
            validate_client(&message.encode_to_vec(), &ctx),
            Err(ValidationError::InvalidSequence)
        );
    }
    ctx.next_sequence = u64::MAX;
    assert!(validate_client(&message.encode_to_vec(), &ctx).is_ok());
    assert_eq!(next_sequence(u64::MAX), None);
    assert_eq!(next_sequence(0), Some(1));
    ctx.next_sequence = 1;
    message.sequence = 1;
    message.session_epoch = 6;
    assert_eq!(
        validate_client(&message.encode_to_vec(), &ctx),
        Err(ValidationError::StaleEpoch)
    );
    message.session_epoch = 7;
    message.client_timestamp_micros = 99;
    assert_eq!(
        validate_client(&message.encode_to_vec(), &ctx),
        Err(ValidationError::InvalidTimestamp)
    );
    message.client_timestamp_micros = u64::MAX;
    message.payload = Some(wire::client_envelope::Payload::OperationBatch(
        wire::ClientOperationBatch {
            operations: vec![wire::ClientOperation {
                timestamp_offset_micros: 1,
                ..pointer()
            }],
        },
    ));
    assert_eq!(
        validate_client(&message.encode_to_vec(), &ctx),
        Err(ValidationError::InvalidTimestamp)
    );
    let decreasing = envelope(vec![
        wire::ClientOperation {
            timestamp_offset_micros: 2,
            ..pointer()
        },
        pointer(),
    ]);
    assert_eq!(
        validate_client(&decreasing.encode_to_vec(), &ctx),
        Err(ValidationError::InvalidTimestamp)
    );
}

#[test]
fn numerical_text_and_expanded_batch_limits_apply_before_admission() {
    let backend = backend();
    let held = HeldState::default();
    let ctx = context(&backend, &held);
    for delta in [-8_388_608, 8_388_608] {
        let op = operation(Action::PointerMove(wire::PointerMove {
            delta_x_1024: delta,
            delta_y_1024: delta,
        }));
        assert!(validate_client(&envelope(vec![op]).encode_to_vec(), &ctx).is_ok());
    }
    for delta in [i32::MIN, -8_388_609, 8_388_609, i32::MAX] {
        assert_eq!(
            validate_client(
                &envelope(vec![operation(Action::Scroll(wire::Scroll {
                    axis: 1,
                    delta_1024: delta
                }))])
                .encode_to_vec(),
                &ctx
            ),
            Err(ValidationError::LimitExceeded)
        );
    }
    assert!(
        validate_client(
            &envelope(vec![text(&"🙂".repeat(1024)); 4]).encode_to_vec(),
            &ctx
        )
        .is_ok()
    );
    assert_eq!(
        validate_client(
            &envelope(vec![text(&"a".repeat(4096)); 5]).encode_to_vec(),
            &ctx
        ),
        Err(ValidationError::LimitExceeded)
    );
    assert!(validate_client(&envelope(vec![pointer(); 256]).encode_to_vec(), &ctx).is_ok());
    for repeat_count in [0, 257, u32::MAX] {
        assert_eq!(
            validate_client(
                &envelope(vec![operation(Action::TextEdit(wire::TextEdit {
                    command: 1,
                    repeat_count
                }))])
                .encode_to_vec(),
                &ctx
            ),
            Err(ValidationError::LimitExceeded)
        );
    }
    let edit = operation(Action::TextEdit(wire::TextEdit {
        command: 1,
        repeat_count: 256,
    }));
    assert!(validate_client(&envelope(vec![edit.clone()]).encode_to_vec(), &ctx).is_ok());
    assert_eq!(
        validate_client(&envelope(vec![pointer(), edit]).encode_to_vec(), &ctx),
        Err(ValidationError::LimitExceeded)
    );
    let mut wire_limits = Limits::HARD.to_wire();
    wire_limits.maximum_operations_per_batch = 1;
    wire_limits.maximum_text_operation_bytes = 4;
    wire_limits.maximum_atomic_click_count = 1;
    wire_limits.maximum_absolute_fixed_delta = 1024;
    let mut lower = context(&backend, &held);
    lower.limits = Limits::from_wire(&wire_limits).unwrap();
    assert_eq!(
        validate_client(&envelope(vec![text("12345")]).encode_to_vec(), &lower),
        Err(ValidationError::LimitExceeded)
    );
    assert_eq!(
        validate_client(
            &envelope(vec![pointer(), pointer()]).encode_to_vec(),
            &lower
        ),
        Err(ValidationError::LimitExceeded)
    );
    assert_eq!(
        validate_client(
            &envelope(vec![operation(Action::MouseButtonClick(
                wire::MouseButtonClick {
                    button: 1,
                    click_count: 2
                }
            ))])
            .encode_to_vec(),
            &lower
        ),
        Err(ValidationError::LimitExceeded)
    );
}

#[test]
fn invalid_hid_and_unspecified_or_unknown_commands_are_never_mapped_to_input() {
    let backend = backend();
    let held = HeldState::default();
    let ctx = context(&backend, &held);
    for key in [
        wire::PhysicalKey {
            usage_page: 8,
            usage_id: 4,
        },
        wire::PhysicalKey {
            usage_page: 7,
            usage_id: 0,
        },
        wire::PhysicalKey {
            usage_page: 7,
            usage_id: u32::MAX,
        },
    ] {
        assert_eq!(
            validate_client(
                &envelope(vec![operation(Action::PhysicalKeyTransition(
                    wire::PhysicalKeyTransition {
                        key: Some(key),
                        state: 1
                    }
                ))])
                .encode_to_vec(),
                &ctx
            ),
            Err(ValidationError::Unsupported)
        );
    }
    for command in [0, 999, -1] {
        assert!(
            validate_client(
                &envelope(vec![operation(Action::LogicalCommand(
                    wire::LogicalCommandAction { command }
                ))])
                .encode_to_vec(),
                &ctx
            )
            .is_err()
        );
        assert!(
            validate_client(
                &envelope(vec![operation(Action::MediaCommand(
                    wire::MediaCommandAction { command }
                ))])
                .encode_to_vec(),
                &ctx
            )
            .is_err()
        );
        assert!(
            validate_client(
                &envelope(vec![operation(Action::TextEdit(wire::TextEdit {
                    command,
                    repeat_count: 1
                }))])
                .encode_to_vec(),
                &ctx
            )
            .is_err()
        );
    }
    for invalid in [
        button(0),
        wire::ClientOperation::default(),
        operation(Action::PhysicalKeyTransition(wire::PhysicalKeyTransition {
            key: None,
            state: 1,
        })),
    ] {
        assert!(validate_client(&envelope(vec![invalid]).encode_to_vec(), &ctx).is_err());
    }
}

#[test]
fn heartbeat_and_leaving_are_not_native_actions_and_work_for_standby() {
    let backend = backend();
    let held = HeldState::default();
    let mut ctx = context(&backend, &held);
    ctx.controls_input = false;
    ctx.epoch = 0;
    let mut value = envelope(vec![]);
    value.session_epoch = 0;
    value.payload = Some(wire::client_envelope::Payload::HeartbeatPing(
        wire::HeartbeatPing {
            heartbeat_identifier: u64::MAX,
        },
    ));
    assert_eq!(
        validate_client(&value.encode_to_vec(), &ctx)
            .unwrap()
            .message(),
        &ClientMessage::Heartbeat(u64::MAX)
    );
    ctx.last_heartbeat_identifier = Some(u64::MAX);
    assert_eq!(
        validate_client(&value.encode_to_vec(), &ctx),
        Err(ValidationError::InvalidSequence)
    );
    value.payload = Some(wire::client_envelope::Payload::HeartbeatPing(
        wire::HeartbeatPing {
            heartbeat_identifier: 0,
        },
    ));
    assert_eq!(
        validate_client(&value.encode_to_vec(), &ctx),
        Err(ValidationError::InvalidSequence)
    );
    ctx.last_heartbeat_identifier = None;
    for reason in 1..=3 {
        value.payload = Some(wire::client_envelope::Payload::ClientLeaving(
            wire::ClientLeaving { reason },
        ));
        assert!(validate_client(&value.encode_to_vec(), &ctx).is_ok());
    }
    value.payload = None;
    assert_eq!(
        validate_client(&value.encode_to_vec(), &ctx),
        Err(ValidationError::MalformedMessage)
    );
}
