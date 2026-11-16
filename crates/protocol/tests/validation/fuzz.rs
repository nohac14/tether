use super::support::*;
use prost::Message;
use tether_protocol::{validation::*, wire};
use tether_test_support::SeededRandom;

#[test]
fn deterministic_bounded_arbitrary_and_mutated_frames_never_panic_or_mutate_context() {
    let backend = backend();
    let before = backend.clone();
    let held = HeldState::default();
    let ctx = context(&backend, &held);
    let server_ctx = server_context();
    let offer = offer();
    let seeds = [
        hello().encode_to_vec(),
        envelope(vec![
            pointer(),
            button(1),
            button(2),
            text("test-only-private🙂"),
        ])
        .encode_to_vec(),
        server_hello().encode_to_vec(),
        server(wire::server_envelope::Payload::ControlState(control())).encode_to_vec(),
    ];
    let mut random = SeededRandom::new(0xa84f_19c2_774e_02d1);
    for iteration in 0..20_000 {
        let mut bytes = if iteration % 2 == 0 {
            let length = usize::try_from(random.next_u64() % 1025).unwrap();
            let mut bytes = vec![0; length];
            random.fill_bytes(&mut bytes);
            bytes
        } else {
            seeds[(iteration / 2) % seeds.len()].clone()
        };
        if !bytes.is_empty() {
            let slot =
                usize::try_from(random.next_u64() % u64::try_from(bytes.len()).unwrap()).unwrap();
            bytes[slot] ^= random.next_u64().to_le_bytes()[0];
            if iteration % 7 == 0 {
                bytes.truncate(slot);
            }
        }
        if let Ok(value) = validate_client(&bytes, &ctx) {
            assert_eq!(value.sequence(), 1);
            assert_eq!(value.epoch(), 7);
            assert!(value.timestamp_micros() >= 100);
            assert!(!format!("{value:?}").contains("test-only-private"));
            if let ClientMessage::Batch(actions) = value.message() {
                assert!(!actions.is_empty() && actions.len() <= 256);
            }
        }
        let _ = validate_client_hello(&bytes, true, Limits::HARD);
        let _ = validate_server(&bytes, &server_ctx);
        let _ = validate_server_hello(&bytes, &offer);
    }
    assert_eq!(backend, before);
    assert_eq!(held, HeldState::default());
}
