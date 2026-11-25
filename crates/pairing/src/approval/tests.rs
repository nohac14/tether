use super::*;
use crate::{CredentialDigest, PairingIdentityError};
use std::net::Ipv4Addr;
use std::sync::{Barrier, Mutex};

fn host() -> HostScope {
    HostScope::local("tether-a1b2c3d4e5f6.local").expect("fixture host")
}

fn peer(last_octet: u8) -> IpAddr {
    Ipv4Addr::new(192, 0, 2, last_octet).into()
}

fn label() -> DeviceLabel {
    DeviceLabel::new("Fixture phone").expect("fixture label")
}

fn issue(state: &mut Pairing, now: Instant, scope: HostScope, entropy: u8) -> String {
    state
        .issue_with(now, scope, || Ok([entropy; SECRET_BYTES]))
        .expect("issue fixture QR")
        .encode_for_fragment()
}

fn submit(
    state: &mut Pairing,
    now: Instant,
    scope: &HostScope,
    source: IpAddr,
    token: &str,
    entropy: u8,
) -> Result<CompletionCapability, PairingError> {
    state.submit_with(now, scope, source, token, label(), || {
        Ok([entropy; SECRET_BYTES])
    })
}

fn first_id(state: &mut Pairing, now: Instant) -> ApprovalId {
    state
        .approvals(now)
        .unwrap()
        .next()
        .expect("approval exists")
        .id()
}

#[test]
fn qr_and_approval_are_independently_single_use_and_scoped() {
    let now = Instant::now();
    let scope = host();
    let alias = HostScope::local("tether-a1b2c3d4e5f6.local.").unwrap();
    let mut state = Pairing::default();
    let token = issue(&mut state, now, scope.clone(), 0xab);
    for (wrong_host, wrong_token) in [(&alias, token.clone()), (&scope, token.to_uppercase())] {
        assert_eq!(
            submit(&mut state, now, wrong_host, peer(1), &wrong_token, 2).unwrap_err(),
            PairingError::Unavailable
        );
    }
    assert_eq!(state.approvals(now).unwrap().len(), 0);

    let completion = submit(&mut state, now, &scope, peer(1), &token, 2)
        .unwrap()
        .encode_for_response_body();
    assert_ne!(token, completion);
    assert_eq!(
        submit(&mut state, now, &scope, peer(1), &token, 3).unwrap_err(),
        PairingError::Unavailable
    );
    for (wrong_host, wrong_peer, wrong_capability) in [
        (&alias, peer(1), completion.as_str()),
        (&scope, peer(2), completion.as_str()),
        (&scope, peer(1), token.as_str()),
    ] {
        assert_eq!(
            state
                .poll_completion(now, wrong_host, wrong_peer, wrong_capability)
                .unwrap_err(),
            PairingError::Unavailable
        );
    }
    assert!(matches!(
        state.poll_completion(now, &scope, peer(1), &completion),
        Ok(CompletionPoll::Pending)
    ));
    let approval = first_id(&mut state, now);
    state
        .decide(now, approval, ApprovalDecision::Approve)
        .unwrap();
    assert_eq!(
        state
            .poll_completion(now, &scope, peer(1), &completion)
            .unwrap_err(),
        PairingError::PollTooSoon
    );
    let now = now + POLL_INTERVAL;
    let CompletionPoll::Approved(approved) = state
        .poll_completion(now, &scope, peer(1), &completion)
        .unwrap()
    else {
        panic!("expected an approved claim");
    };
    assert_eq!(approved.approval().id(), approval);
    assert_eq!(approved.approval().host(), &scope);
    assert_eq!(approved.approval().peer(), peer(1));
    assert_eq!(approved.approval().label(), &label());
    assert_eq!(
        state
            .poll_completion(now, &scope, peer(1), &completion)
            .unwrap_err(),
        PairingError::Unavailable
    );
}

#[test]
fn exact_expiry_and_poll_bound_include_pending_and_approved_requests() {
    let start = Instant::now();
    let scope = host();
    let mut state = Pairing::default();
    let expired = issue(&mut state, start, scope.clone(), 1);
    let now = start + LIFETIME;
    assert_eq!(
        submit(&mut state, now, &scope, peer(1), &expired, 2).unwrap_err(),
        PairingError::Unavailable
    );
    let token = issue(&mut state, now, scope.clone(), 3);
    let completion = submit(&mut state, now, &scope, peer(1), &token, 4)
        .unwrap()
        .encode_for_response_body();
    let approval = first_id(&mut state, now);
    for poll in 0..60 {
        let at = now + POLL_INTERVAL * poll;
        assert!(matches!(
            state.poll_completion(at, &scope, peer(1), &completion),
            Ok(CompletionPoll::Pending)
        ));
        assert_eq!(
            state
                .poll_completion(at, &scope, peer(1), &completion)
                .unwrap_err(),
            PairingError::PollTooSoon
        );
    }
    let expiry = now + LIFETIME;
    state
        .decide(
            expiry.checked_sub(Duration::from_millis(1)).unwrap(),
            approval,
            ApprovalDecision::Approve,
        )
        .unwrap();
    assert_eq!(
        state
            .poll_completion(expiry, &scope, peer(1), &completion)
            .unwrap_err(),
        PairingError::Unavailable
    );
    assert_eq!(state.approvals(expiry).unwrap().len(), 0);
}

#[test]
fn approval_caps_share_the_canonical_peer_across_approved_hosts() {
    let now = Instant::now();
    let scope = host();
    let fallback = HostScope::ip(peer(42));
    let mut state = Pairing::default();
    for number in 0..8 {
        let address = peer(number / 2 + 1);
        let scope = if number % 2 == 0 { &scope } else { &fallback };
        let token = issue(&mut state, now, scope.clone(), 1 + number);
        submit(&mut state, now, scope, address, &token, 11 + number).unwrap();
        if number == 1 {
            let mapped = IpAddr::V6(Ipv4Addr::new(192, 0, 2, 1).to_ipv6_mapped());
            let third = issue(&mut state, now, scope.clone(), 50);
            assert_eq!(
                submit(&mut state, now, scope, mapped, &third, 51).unwrap_err(),
                PairingError::ApprovalLimit
            );
        }
    }
    let extra = issue(&mut state, now, scope.clone(), 60);
    assert_eq!(
        submit(&mut state, now, &scope, peer(5), &extra, 61).unwrap_err(),
        PairingError::ApprovalLimit
    );
    let id = first_id(&mut state, now);
    state.decide(now, id, ApprovalDecision::Deny).unwrap();
    submit(&mut state, now, &scope, peer(5), &extra, 62).unwrap();
    assert_eq!(state.approvals(now).unwrap().len(), MAX_APPROVALS);
}

#[test]
fn replacement_denial_invalidation_and_restart_retire_old_capabilities() {
    let now = Instant::now();
    let scope = host();
    let mut state = Pairing::default();
    let old = issue(&mut state, now, scope.clone(), 1);
    let new = issue(&mut state, now, scope.clone(), 2);
    assert_eq!(
        submit(&mut state, now, &scope, peer(1), &old, 3).unwrap_err(),
        PairingError::Unavailable
    );
    let denied = submit(&mut state, now, &scope, peer(1), &new, 4)
        .unwrap()
        .encode_for_response_body();
    let denied_id = first_id(&mut state, now);
    state
        .decide(now, denied_id, ApprovalDecision::Deny)
        .unwrap();
    assert_eq!(
        state
            .poll_completion(now, &scope, peer(1), &denied)
            .unwrap_err(),
        PairingError::Unavailable
    );
    let token = issue(&mut state, now, scope.clone(), 5);
    let completion = submit(&mut state, now, &scope, peer(1), &token, 6)
        .unwrap()
        .encode_for_response_body();
    let revoked = first_id(&mut state, now);
    state
        .decide(now, revoked, ApprovalDecision::Approve)
        .unwrap();
    state.invalidate();
    for fresh in [&mut state, &mut Pairing::default()] {
        assert_eq!(
            fresh
                .poll_completion(now, &scope, peer(1), &completion)
                .unwrap_err(),
            PairingError::Unavailable
        );
    }
    let next = issue(&mut state, now, scope.clone(), 7);
    submit(&mut state, now, &scope, peer(1), &next, 8).unwrap();
    assert!(first_id(&mut state, now).get() > revoked.get());
    assert_eq!(
        state.decide(now, revoked, ApprovalDecision::Approve),
        Err(PairingError::Unavailable)
    );
}

#[test]
fn entropy_failure_and_counter_exhaustion_leave_no_partial_approval() {
    let now = Instant::now();
    let scope = host();
    let mut state = Pairing::default();
    let qr = issue(&mut state, now, scope.clone(), 1);
    assert_eq!(
        submit(&mut state, now, &scope, peer(1), &qr, 1).unwrap_err(),
        PairingError::EntropyUnavailable
    );
    let err = state.submit_with(now, &scope, peer(1), &qr, label(), || {
        Err(PairingError::EntropyUnavailable)
    });
    assert_eq!(err.unwrap_err(), PairingError::EntropyUnavailable);
    assert_eq!(state.approvals(now).unwrap().len(), 0);
    assert_eq!(state.last_id, 0);
    state.last_id = u64::MAX;
    assert_eq!(
        submit(&mut state, now, &scope, peer(1), &qr, 2).unwrap_err(),
        PairingError::ApprovalIdExhausted
    );
    assert_eq!(state.approvals(now).unwrap().len(), 0);
    assert_eq!(
        state
            .issue_with(now, scope.clone(), || Err(PairingError::EntropyUnavailable))
            .unwrap_err(),
        PairingError::EntropyUnavailable
    );
    assert_eq!(
        submit(&mut state, now, &scope, peer(1), &qr, 2).unwrap_err(),
        PairingError::Unavailable
    );
}

#[test]
fn clock_regression_closes_volatile_pairing_state() {
    let early = Instant::now();
    let late = early + Duration::from_secs(1);
    let scope = host();
    let mut state = Pairing::default();
    let qr = issue(&mut state, late, scope.clone(), 1);
    let completion = submit(&mut state, late, &scope, peer(1), &qr, 2)
        .unwrap()
        .encode_for_response_body();
    assert_eq!(
        state
            .poll_completion(early, &scope, peer(1), &completion)
            .unwrap_err(),
        PairingError::InvalidClock
    );
    assert_eq!(state.approvals(late).unwrap().len(), 0);
    assert_eq!(
        state
            .poll_completion(late, &scope, peer(1), &completion)
            .unwrap_err(),
        PairingError::Unavailable
    );
}

#[test]
fn concurrent_completion_consumers_get_only_one_approved_claim() {
    let now = Instant::now();
    let scope = host();
    let mut state = Pairing::default();
    let token = issue(&mut state, now, scope.clone(), 1);
    let capability = submit(&mut state, now, &scope, peer(1), &token, 2)
        .unwrap()
        .encode_for_response_body();
    let id = first_id(&mut state, now);
    state.decide(now, id, ApprovalDecision::Approve).unwrap();
    let state = Mutex::new(state);
    let ready = Barrier::new(2);
    std::thread::scope(|threads| {
        let claim = || {
            ready.wait();
            state
                .lock()
                .unwrap()
                .poll_completion(now, &scope, peer(1), &capability)
        };
        let first = threads.spawn(claim);
        let second = threads.spawn(claim);
        let results = [first.join().unwrap(), second.join().unwrap()];
        assert_eq!(
            results
                .iter()
                .filter(|outcome| matches!(outcome, Ok(CompletionPoll::Approved(_))))
                .count(),
            1
        );
        assert_eq!(
            results
                .iter()
                .filter(|outcome| matches!(outcome, Err(PairingError::Unavailable)))
                .count(),
            1
        );
    });
}

#[test]
fn credential_digest_uses_sha256_and_exact_bounded_cookie_encoding() {
    let digest = CredentialDigest::from_stored_sha256([
        0x66, 0x68, 0x7a, 0xad, 0xf8, 0x62, 0xbd, 0x77, 0x6c, 0x8f, 0xc1, 0x8b, 0x8e, 0x9f, 0x8e,
        0x20, 0x08, 0x97, 0x14, 0x85, 0x6e, 0xe2, 0x33, 0xb3, 0x90, 0x2a, 0x59, 0x1d, 0x0d, 0x5f,
        0x29, 0x25,
    ]);
    assert!(digest.matches_cookie_value(&"0".repeat(64)));
    for invalid in [
        "0".repeat(63),
        "0".repeat(65),
        "1".repeat(64),
        "🧪".repeat(16),
    ] {
        assert!(!digest.matches_cookie_value(&invalid));
    }
    assert_eq!(format!("{digest:#?}"), "CredentialDigest(<redacted>)");
}

#[test]
fn public_issue_and_submission_use_platform_entropy() {
    let now = Instant::now();
    let scope = host();
    let mut state = Pairing::default();
    let qr = state
        .issue_qr(now, scope.clone())
        .unwrap()
        .encode_for_fragment();
    let completion = state
        .submit(now, &scope, peer(1), &qr, label())
        .unwrap()
        .encode_for_response_body();
    assert_eq!(qr.len(), 64);
    assert_eq!(completion.len(), 64);
    assert!(matches!(
        state.poll_completion(now, &scope, peer(1), &completion),
        Ok(CompletionPoll::Pending)
    ));
}

#[test]
fn scope_and_label_are_strict_bounded_and_diagnostics_redact() {
    assert_eq!(
        HostScope::local("TETHER-A1b2C3D4E5F6.LOCAL").unwrap(),
        host()
    );
    let dotted = HostScope::local("TETHER-A1B2C3D4E5F6-10000.LOCAL.").unwrap();
    assert_eq!(dotted.host_name(), "tether-a1b2c3d4e5f6-10000.local.");
    for invalid in [
        "tether-a1b2c3d4e5f6.local:1234",
        "tether-a1b2c3d4e5f6.local..",
        "example.local",
        "tether-a1b2c3d4e5f6-1.local",
        "tether-a1b2c3d4e5f6-02.local",
        "tether-a1b2c3d4e5f6-+2.local",
    ] {
        assert_eq!(
            HostScope::local(invalid),
            Err(PairingIdentityError::InvalidLocalName)
        );
    }
    assert!(DeviceLabel::new(&"é".repeat(40)).is_ok());
    assert!(DeviceLabel::new(&"é".repeat(41)).is_err());
    for invalid in [
        "",
        " padded ",
        "a\nb",
        "a\u{2028}b",
        "a\u{202e}b",
        "a\u{2069}b",
    ] {
        assert_eq!(
            DeviceLabel::new(invalid),
            Err(PairingIdentityError::InvalidDeviceLabel)
        );
    }
    let qr = QrToken([0xfe; SECRET_BYTES]);
    let completion = CompletionCapability([0xac; SECRET_BYTES]);
    assert_eq!(
        format!("{qr:?} {completion:?}"),
        "QrToken(<redacted>) CompletionCapability(<redacted>)"
    );
    assert_eq!(
        format!("{dotted:?} {:?}", label()),
        "HostScope(<redacted>) DeviceLabel(<redacted>)"
    );
}
