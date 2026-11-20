use super::*;
use prost::Message;
use tether_input_core as input;
use tether_protocol::wire;
use tether_test_support::SeededRandom;
use wire::client_operation::Action;

fn backend() -> BackendStatus {
    let mut caps = input::BackendCapabilities::unavailable();
    caps.relative_pointer = input::CapabilityState::Available;
    caps.physical_keys = input::CapabilityState::Available;
    caps.atomic_click = input::CapabilityState::Available;
    caps.mouse_buttons.insert(input::MouseButton::Left);
    caps.text_modes
        .insert(input::TextMode::NativeUnicodeBestEffort);
    BackendStatus {
        revision: 1,
        kind: input::BackendKind::Mock,
        initialized: true,
        available: true,
        permission: input::PermissionState::NotRequired,
        capabilities: caps,
        fault: None,
    }
}
fn trusted_facts(backend: &BackendStatus, epoch: u64) -> AdmissionFacts<'_> {
    AdmissionFacts {
        authenticated: true,
        controls_input: true,
        authorization_revision: 1,
        epoch,
        backend,
    }
}
fn frame(sequence: u64, epoch: u64, actions: Vec<Action>) -> wire::ClientEnvelope {
    wire::ClientEnvelope {
        protocol_major: 1,
        sequence,
        session_epoch: epoch,
        client_timestamp_micros: 100,
        payload: Some(wire::client_envelope::Payload::OperationBatch(
            wire::ClientOperationBatch {
                operations: actions
                    .into_iter()
                    .map(|action| wire::ClientOperation {
                        action: Some(action),
                        ..Default::default()
                    })
                    .collect(),
            },
        )),
        ..Default::default()
    }
}
fn button(state: i32) -> Action {
    Action::MouseButtonTransition(wire::MouseButtonTransition { button: 1, state })
}
fn ping(sequence: u64, epoch: u64, identifier: u64) -> wire::ClientEnvelope {
    wire::ClientEnvelope {
        payload: Some(wire::client_envelope::Payload::HeartbeatPing(
            wire::HeartbeatPing {
                heartbeat_identifier: identifier,
            },
        )),
        ..frame(sequence, epoch, vec![])
    }
}
fn accept(cursor: &mut AdmissionCursor, value: &wire::ClientEnvelope, facts: &AdmissionFacts<'_>) {
    let prepared = cursor.prepare(&value.encode_to_vec(), facts).unwrap();
    cursor.commit(prepared, facts).unwrap();
}

#[test]
fn prepare_drop_and_bad_batch_leave_cursor_unchanged_but_queued_down_up_work() {
    let status = backend();
    let facts = trusted_facts(&status, 7);
    let mut cursor = AdmissionCursor::new(7, Limits::HARD).unwrap();
    let before = cursor.snapshot();
    let prepared = cursor
        .prepare(&frame(1, 7, vec![button(1)]).encode_to_vec(), &facts)
        .unwrap();
    assert_eq!(prepared.operations(), 1);
    assert_eq!(prepared.barrier(), None);
    assert_eq!(cursor.snapshot(), before);
    drop(prepared);
    assert_eq!(cursor.snapshot(), before);
    assert!(
        cursor
            .prepare(
                &frame(1, 7, vec![button(1), button(1)]).encode_to_vec(),
                &facts
            )
            .is_err()
    );
    assert_eq!(cursor.snapshot(), before);
    accept(&mut cursor, &frame(1, 7, vec![button(1)]), &facts);
    assert_eq!(cursor.snapshot().projected_buttons, 1);
    // No dispatcher exists here: up validates against admitted, not completed, down.
    accept(&mut cursor, &frame(2, 7, vec![button(2)]), &facts);
    assert_eq!(cursor.snapshot().projected_buttons, 0);
    assert_eq!(cursor.snapshot().next_sequence, Some(3));
}

#[test]
fn cross_cursor_and_obsolete_preparations_cannot_commit() {
    let status = backend();
    let facts = trusted_facts(&status, 7);
    let mut first = AdmissionCursor::new(7, Limits::HARD).unwrap();
    let mut second = AdmissionCursor::new(7, Limits::HARD).unwrap();
    let bytes = frame(1, 7, vec![button(1)]).encode_to_vec();
    let foreign = first.prepare(&bytes, &facts).unwrap();
    let before = second.snapshot();
    assert_eq!(
        second.commit(foreign, &facts),
        Err(AdmissionError::StalePreparation)
    );
    assert_eq!(second.snapshot(), before);
    let accepted = first.prepare(&bytes, &facts).unwrap();
    let stale = first.prepare(&bytes, &facts).unwrap();
    first.commit(accepted, &facts).unwrap();
    let before = first.snapshot();
    assert_eq!(
        first.commit(stale, &facts),
        Err(AdmissionError::StalePreparation)
    );
    assert_eq!(first.snapshot(), before);
}

#[test]
fn epoch_changes_invalidate_old_preparation_and_key_projection_rolls_back_atomically() {
    let status = backend();
    let mut cursor = AdmissionCursor::new(7, Limits::HARD).unwrap();
    let old = cursor
        .prepare(
            &frame(1, 7, vec![button(1)]).encode_to_vec(),
            &trusted_facts(&status, 7),
        )
        .unwrap();
    cursor.advance_epoch(8).unwrap();
    let facts = trusted_facts(&status, 8);
    let before = cursor.snapshot();
    assert_eq!(
        cursor.commit(old, &facts),
        Err(AdmissionError::StalePreparation)
    );
    assert_eq!(cursor.snapshot(), before);
    let key = |usage_id, state| {
        Action::PhysicalKeyTransition(wire::PhysicalKeyTransition {
            key: Some(wire::PhysicalKey {
                usage_page: 7,
                usage_id,
            }),
            state,
        })
    };
    accept(
        &mut cursor,
        &frame(1, 8, vec![key(4, 1), key(0xe0, 1), button(1)]),
        &facts,
    );
    assert_eq!(cursor.snapshot().projected_keys, 2);
    let before = cursor.snapshot();
    let revision = cursor.revision;
    let bad = frame(
        2,
        8,
        vec![
            key(4, 2),
            Action::MouseButtonClick(wire::MouseButtonClick {
                button: 1,
                click_count: 1,
            }),
        ],
    );
    assert!(cursor.prepare(&bad.encode_to_vec(), &facts).is_err());
    assert_eq!(cursor.snapshot(), before);
    assert_eq!(cursor.revision, revision);
    accept(
        &mut cursor,
        &frame(2, 8, vec![key(4, 2), key(0xe0, 2), button(2)]),
        &facts,
    );
    assert_eq!(
        (
            cursor.snapshot().projected_keys,
            cursor.snapshot().projected_buttons
        ),
        (0, 0)
    );
}

#[test]
fn changed_authorization_role_epoch_or_backend_facts_invalidate_preparation() {
    let status = backend();
    for change in 0..6 {
        let mut cursor = AdmissionCursor::new(7, Limits::HARD).unwrap();
        let original = trusted_facts(&status, 7);
        let prepared = cursor
            .prepare(&frame(1, 7, vec![button(1)]).encode_to_vec(), &original)
            .unwrap();
        let before = cursor.snapshot();
        let mut changed_status = status.clone();
        if change == 4 {
            changed_status.available = false;
        }
        if change == 5 {
            changed_status.capabilities.mouse_buttons.clear();
        }
        let mut current = trusted_facts(&changed_status, 7);
        match change {
            0 => current.authenticated = false,
            1 => current.controls_input = false,
            2 => current.authorization_revision = 2,
            3 => current.epoch = 8,
            _ => {}
        }
        assert_eq!(
            cursor.commit(prepared, &current),
            Err(AdmissionError::StalePreparation)
        );
        assert_eq!(cursor.snapshot(), before);
    }
    let cursor = AdmissionCursor::new(7, Limits::HARD).unwrap();
    let mut denied = trusted_facts(&status, 7);
    denied.authenticated = false;
    assert_eq!(
        cursor
            .prepare(&frame(1, 7, vec![button(1)]).encode_to_vec(), &denied)
            .unwrap_err(),
        AdmissionError::Validation(ValidationError::AuthenticationRequired)
    );
    denied.authenticated = true;
    denied.authorization_revision = 0;
    assert_eq!(
        cursor
            .prepare(&frame(1, 7, vec![button(1)]).encode_to_vec(), &denied)
            .unwrap_err(),
        AdmissionError::InvalidFacts
    );
}

#[test]
fn release_barrier_blocks_input_but_not_heartbeat_and_never_replays_held_state() {
    let status = backend();
    let original = trusted_facts(&status, 7);
    let mut cursor = AdmissionCursor::new(7, Limits::HARD).unwrap();
    accept(
        &mut cursor,
        &frame(
            1,
            7,
            vec![button(1), Action::ReleaseAll(wire::ReleaseAll {})],
        ),
        &original,
    );
    let before = cursor.snapshot();
    assert_eq!(before.pending_barrier, Some(BarrierKind::ReleaseAll));
    assert_eq!(before.projected_buttons, 1); // not a native cleanup acknowledgement
    assert_eq!(
        cursor
            .prepare(&frame(2, 7, vec![button(2)]).encode_to_vec(), &original)
            .unwrap_err(),
        AdmissionError::PendingBarrier
    );
    assert_eq!(cursor.snapshot(), before);
    accept(&mut cursor, &ping(2, 7, 1), &original);
    assert_eq!(
        cursor.resolve_request_without_transfer(),
        Err(AdmissionError::InvalidTransition)
    );
    assert_eq!(
        cursor.advance_epoch(7),
        Err(AdmissionError::InvalidTransition)
    );
    cursor.advance_epoch(8).unwrap();
    assert_eq!(cursor.snapshot().projected_buttons, 0);
    assert_eq!(cursor.snapshot().pending_barrier, None);
    assert_eq!(cursor.snapshot().next_sequence, Some(3));
    assert_eq!(cursor.snapshot().last_heartbeat_identifier, Some(1));
    let current = trusted_facts(&status, 8);
    assert_eq!(
        cursor
            .prepare(&frame(3, 7, vec![button(1)]).encode_to_vec(), &current)
            .unwrap_err(),
        AdmissionError::Validation(ValidationError::StaleEpoch)
    );
    accept(&mut cursor, &frame(3, 8, vec![button(1)]), &current);
}

#[test]
fn denied_request_clears_only_request_barrier_and_does_not_grant_authority() {
    let status = backend();
    let mut standby = trusted_facts(&status, 0);
    standby.controls_input = false;
    let mut cursor = AdmissionCursor::new(0, Limits::HARD).unwrap();
    accept(
        &mut cursor,
        &frame(1, 0, vec![Action::RequestControl(wire::RequestControl {})]),
        &standby,
    );
    assert_eq!(
        cursor.snapshot().pending_barrier,
        Some(BarrierKind::RequestControl)
    );
    cursor.resolve_request_without_transfer().unwrap();
    assert_eq!(cursor.snapshot().pending_barrier, None);
    assert_eq!(
        cursor
            .prepare(&frame(2, 0, vec![button(1)]).encode_to_vec(), &standby)
            .unwrap_err(),
        AdmissionError::Validation(ValidationError::NotController)
    );
    assert_eq!(cursor.snapshot().next_sequence, Some(2));
    let mut owner = AdmissionCursor::new(7, Limits::HARD).unwrap();
    accept(
        &mut owner,
        &frame(
            1,
            7,
            vec![Action::RelinquishControl(wire::RelinquishControl {})],
        ),
        &trusted_facts(&status, 7),
    );
    assert_eq!(
        owner.resolve_request_without_transfer(),
        Err(AdmissionError::InvalidTransition)
    );
}

#[test]
fn heartbeat_and_time_state_advance_only_on_commit_without_wrapping() {
    let status = backend();
    let facts = trusted_facts(&status, 7);
    let mut cursor = AdmissionCursor::new(7, Limits::HARD).unwrap();
    let pending = cursor
        .prepare(&ping(1, 7, 3).encode_to_vec(), &facts)
        .unwrap();
    drop(pending);
    assert_eq!(cursor.snapshot().last_heartbeat_identifier, None);
    accept(&mut cursor, &ping(1, 7, 3), &facts);
    assert_eq!(cursor.snapshot().last_timestamp_micros, 100);
    assert_eq!(
        cursor
            .prepare(&ping(2, 7, 3).encode_to_vec(), &facts)
            .unwrap_err(),
        AdmissionError::Validation(ValidationError::InvalidSequence)
    );
    let mut regressed = ping(2, 7, 4);
    regressed.client_timestamp_micros = 99;
    assert_eq!(
        cursor
            .prepare(&regressed.encode_to_vec(), &facts)
            .unwrap_err(),
        AdmissionError::Validation(ValidationError::InvalidTimestamp)
    );
    cursor.next_sequence = Some(u64::MAX);
    accept(&mut cursor, &ping(u64::MAX, 7, u64::MAX), &facts);
    assert_eq!(cursor.snapshot().next_sequence, None);
    cursor.advance_epoch(8).unwrap();
    assert_eq!(cursor.snapshot().next_sequence, None);
    assert_eq!(
        cursor
            .prepare(&ping(1, 8, 0).encode_to_vec(), &trusted_facts(&status, 8))
            .unwrap_err(),
        AdmissionError::CounterExhausted
    );
}

#[test]
fn private_instance_and_revision_exhaustion_fail_closed_without_global_test_mutation() {
    let local = AtomicU64::new(u64::MAX);
    assert_eq!(nonce(&local), Err(AdmissionError::CounterExhausted));
    assert_eq!(local.load(Ordering::Relaxed), u64::MAX);
    let status = backend();
    let facts = trusted_facts(&status, 7);
    let mut cursor = AdmissionCursor::new(7, Limits::HARD).unwrap();
    cursor.revision = u64::MAX;
    let before = cursor.snapshot();
    assert_eq!(
        cursor
            .prepare(&frame(1, 7, vec![button(1)]).encode_to_vec(), &facts)
            .unwrap_err(),
        AdmissionError::CounterExhausted
    );
    assert_eq!(
        cursor.advance_epoch(8),
        Err(AdmissionError::CounterExhausted)
    );
    assert_eq!(cursor.snapshot(), before);
}

#[test]
fn cursor_instance_numbers_are_unique_across_concurrent_construction() {
    let local = AtomicU64::new(1);
    let all = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| scope.spawn(|| (0..64).map(|_| nonce(&local).unwrap()).collect::<Vec<_>>()))
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap())
            .collect::<std::collections::BTreeSet<_>>()
    });
    assert_eq!(all.len(), 256);
    assert_eq!(local.load(Ordering::Relaxed), 257);
}

#[test]
fn cursor_preparation_and_fact_diagnostics_never_reveal_input_or_metadata() {
    let status = backend();
    let facts = trusted_facts(&status, 7);
    let cursor = AdmissionCursor::new(7, Limits::HARD).unwrap();
    let value = frame(
        1,
        7,
        vec![Action::TextInsertion(wire::TextInsertion {
            text: "synthetic-private-🙂".into(),
        })],
    );
    let prepared = cursor.prepare(&value.encode_to_vec(), &facts).unwrap();
    assert_eq!(format!("{prepared:?}"), "PreparedEnvelope(<redacted>)");
    assert_eq!(format!("{cursor:?}"), "AdmissionCursor(<redacted>)");
    assert_eq!(format!("{facts:?}"), "AdmissionFacts(<redacted>)");
    assert_eq!(
        format!("{:?}", cursor.snapshot()),
        "AdmissionSnapshot(<redacted>)"
    );
}

#[test]
fn seeded_projection_matches_button_model_under_dropped_preparations() {
    let status = backend();
    let facts = trusted_facts(&status, 7);
    let mut cursor = AdmissionCursor::new(7, Limits::HARD).unwrap();
    let mut random = SeededRandom::new(0x1c25_6bf3_a201_6d71);
    let mut held = false;
    let mut sequence = 1;
    for _ in 0..10_000 {
        let before = cursor.snapshot();
        let value = frame(sequence, 7, vec![button(if held { 2 } else { 1 })]);
        let pending = cursor.prepare(&value.encode_to_vec(), &facts).unwrap();
        assert_eq!(cursor.snapshot(), before);
        if random.next_u64() & 1 == 0 {
            drop(pending);
            assert_eq!(cursor.snapshot(), before);
        } else {
            cursor.commit(pending, &facts).unwrap();
            held = !held;
            sequence += 1;
            assert_eq!(cursor.snapshot().projected_buttons, usize::from(held));
            assert_eq!(cursor.snapshot().next_sequence, Some(sequence));
        }
    }
}
