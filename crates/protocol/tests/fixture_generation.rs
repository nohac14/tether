//! Synthetic common goldens: never feed real user data into this fixture writer.
mod corpus;
#[path = "validation/support.rs"]
mod support;

use corpus::{Case, Envelope};
use prost::Message;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use tether_protocol::{validation::*, wire};

fn path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/protocol/v1-golden.json")
}

#[test]
fn frozen_corpus_has_exact_case_set_bytes_and_safe_admission_results() {
    let path = path();
    let metadata = fs::symlink_metadata(&path).unwrap();
    assert!(metadata.is_file() && metadata.len() <= 1_048_576);
    let actual = fs::read_to_string(path).unwrap();
    let cases = corpus::cases::cases();
    assert!(
        actual == corpus::document(&cases),
        "synthetic corpus drift; review, then cargo xtask fixtures"
    );
    let unique: BTreeSet<_> = cases.iter().map(|value| &value.id).collect();
    assert_eq!(unique.len(), cases.len());
    assert!(cases.len() >= 70);
    for case in &cases {
        verify(case);
    }
}

fn verify(case: &Case) {
    let admission = match &case.expected {
        Some(Envelope::Client(expected)) => {
            assert_eq!(
                wire::ClientEnvelope::decode(case.bytes.as_slice()).unwrap(),
                *expected,
                "{}",
                case.id
            );
            if case.hello {
                validate_client_hello(&case.bytes, true, Limits::HARD).map(|_| ())
            } else {
                let backend = support::backend();
                let held = HeldState::default();
                let mut context = support::context(&backend, &held);
                if case.id == "full-width-counters" {
                    context.next_sequence = u64::MAX;
                    context.epoch = u64::MAX;
                }
                validate_client(&case.bytes, &context).map(|_| ())
            }
        }
        Some(Envelope::Server(expected)) => {
            assert_eq!(
                wire::ServerEnvelope::decode(case.bytes.as_slice()).unwrap(),
                *expected,
                "{}",
                case.id
            );
            if case.hello {
                validate_server_hello(&case.bytes, &support::offer()).map(|_| ())
            } else {
                validate_server(&case.bytes, &support::server_context()).map(|_| ())
            }
        }
        None => {
            assert!(wire::ClientEnvelope::decode(case.bytes.as_slice()).is_err());
            validate_client(
                &case.bytes,
                &support::context(&support::backend(), &HeldState::default()),
            )
            .map(|_| ())
        }
    };
    assert_eq!(admission, case.admission, "{}", case.id);
    if let Some(value) = &case.expected {
        let decoded = match value {
            Envelope::Client(_) => {
                Envelope::Client(wire::ClientEnvelope::decode(case.bytes.as_slice()).unwrap())
            }
            Envelope::Server(_) => {
                Envelope::Server(wire::ServerEnvelope::decode(case.bytes.as_slice()).unwrap())
            }
        };
        assert_eq!(decoded.bytes(), value.bytes());
    }
}

/// Explicit generation command, not a skipped runtime verification test.
#[test]
#[ignore = "writes only reviewed synthetic goldens; run cargo xtask fixtures explicitly"]
fn generate_fixtures() {
    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/manifest.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        manifest,
        serde_json::json!({"version": 1, "synthetic_only": true, "protocol": "protocol/v1-golden.json"})
    );
    let path = path();
    assert!(
        fs::symlink_metadata(path.parent().unwrap())
            .unwrap()
            .is_dir()
    );
    if let Ok(metadata) = fs::symlink_metadata(&path) {
        assert!(metadata.is_file());
    }
    let cases = corpus::cases::cases();
    for case in &cases {
        verify(case);
    }
    let text = corpus::document(&cases);
    assert!(text.len() <= 1_048_576);
    fs::write(path, text).unwrap();
}
