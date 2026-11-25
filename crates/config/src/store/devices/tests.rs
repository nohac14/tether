use super::super::tests::write_fixture;
use super::super::{CommitHook, CommitStage, NoopCommitHook};
use super::*;
use serde_json::json;
use std::io;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use tether_input_core::{
    ButtonAction, DispatchOutcome, DispatcherConfig, EmergencyReason, InputAction, InputDispatcher,
    KeyAction, MockInputBackend, MouseButton, PhysicalKey,
};
use tether_pairing::{ApprovalDecision, CompletionPoll, Pairing};
use tether_test_support::PrivateTempDir;

const ISSUED: u64 = 1_700_000_000;
const EXPIRY: u64 = ISSUED + CREDENTIAL_LIFETIME_SECONDS;

fn host() -> HostScope {
    HostScope::local("tether-a1b2c3d4e5f6.local").unwrap()
}

fn wall(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

fn claimed_approval(scope: &HostScope, now: Instant) -> ApprovedPairing {
    let label = DeviceLabel::new("Fixture phone").unwrap();
    let mut actor = Pairing::default();
    let proof = actor
        .issue_qr(now, scope.clone())
        .unwrap()
        .encode_for_fragment();
    let peer = IpAddr::from([192, 0, 2, 7]);
    let completion = actor.submit(now, scope, peer, &proof, label).unwrap();
    let id = actor.approvals(now).unwrap().next().unwrap().id();
    actor.decide(now, id, ApprovalDecision::Approve).unwrap();
    match actor
        .poll_completion(now, scope, peer, &completion.encode_for_response_body())
        .unwrap()
    {
        CompletionPoll::Approved(approval) => approval,
        CompletionPoll::Pending => panic!("fixture requires an approved claim"),
    }
}

struct Fixture {
    store: SettingsStore,
    root: std::path::PathBuf,
    instant: Instant,
    _temp: PrivateTempDir,
}

impl Fixture {
    fn new() -> Self {
        let temp = PrivateTempDir::new().unwrap();
        let root = temp.path().join("config");
        let (store, _) = SettingsStore::open(&root).unwrap();
        Self {
            _temp: temp,
            root,
            store,
            instant: Instant::now(),
        }
    }

    fn enroll(&mut self, scope: &HostScope) -> Result<DeviceEnrollment, StorageError> {
        self.store.enroll_device(
            self.revision(),
            claimed_approval(scope, self.instant),
            self.instant,
            wall(ISSUED),
        )
    }

    fn enroll_cookie(&mut self, scope: &HostScope) -> (DeviceId, DeviceCookie) {
        let receipt = self.enroll(scope).unwrap();
        let id = receipt.device_id();
        let cookie = self
            .store
            .deliver_device_cookie(receipt, self.instant, wall(ISSUED))
            .unwrap();
        (id, cookie)
    }

    fn revision(&self) -> DeviceRevision {
        self.store.devices_snapshot().unwrap().revision()
    }
}

fn cookie_value(cookie: &DeviceCookie) -> &str {
    cookie
        .as_set_cookie()
        .strip_prefix("tether_device=")
        .unwrap()
        .split_once(';')
        .unwrap()
        .0
}

#[test]
fn durable_enrollment_contains_only_digest_and_verifies_after_restart() {
    let mut fixture = Fixture::new();
    let scope = host();
    let path = fixture.root.join(DOCUMENT.name);
    assert!(!path.exists());
    let (id, cookie) = fixture.enroll_cookie(&scope);
    let encoded = cookie.as_set_cookie();
    assert!(encoded.ends_with("; HttpOnly; SameSite=Strict; Path=/control; Max-Age=2592000"));
    let value = cookie_value(&cookie);
    assert_eq!(value.len(), 64);
    assert_eq!(format!("{cookie:#?}"), "DeviceCookie(<redacted>)");
    let disk = fs::read_to_string(&path).unwrap();
    assert!(!disk.contains(value));
    assert!(disk.contains("credential_sha256"));
    assert!(!disk.contains("tether_device="));

    drop(fixture.store);
    let (reopened, _) = SettingsStore::open(&fixture.root).unwrap();
    assert_eq!(reopened.device_store_status(), DeviceStoreStatus::Ready);
    assert_eq!(
        reopened
            .verify_device_cookie(&scope, value, wall(ISSUED))
            .unwrap()
            .id(),
        id
    );
    assert!(
        reopened
            .verify_device_cookie(&scope, value, wall(EXPIRY - 1))
            .is_some()
    );
    for (scope, now) in [
        (host(), wall(ISSUED - 1)),
        (host(), wall(EXPIRY)),
        (
            HostScope::local("tether-a1b2c3d4e5f6.local.").unwrap(),
            wall(ISSUED),
        ),
        (HostScope::ip(IpAddr::from([192, 0, 2, 7])), wall(ISSUED)),
    ] {
        assert!(reopened.verify_device_cookie(&scope, value, now).is_none());
    }
    assert!(
        reopened
            .verify_device_cookie(&host(), "malformed", wall(ISSUED))
            .is_none()
    );
}

#[test]
fn approval_expiry_and_revocation_prevent_cookie_publication() {
    let mut fixture = Fixture::new();
    let scope = host();
    let expiry = fixture.instant + Duration::from_secs(120);
    let result = fixture.store.enroll_device(
        fixture.revision(),
        claimed_approval(&scope, fixture.instant),
        expiry,
        wall(ISSUED),
    );
    assert_eq!(result.unwrap_err(), StorageError::ApprovalExpired);
    assert!(!fixture.root.join(DOCUMENT.name).exists());

    let receipt = fixture.enroll(&scope).unwrap();
    assert_eq!(
        fixture
            .store
            .deliver_device_cookie(receipt, expiry, wall(ISSUED))
            .unwrap_err(),
        StorageError::ApprovalExpired
    );
    let receipt = fixture.enroll(&scope).unwrap();
    fixture
        .store
        .begin_device_revocation(
            Some(fixture.revision()),
            DeviceRevocation::Device(receipt.device_id()),
        )
        .unwrap();
    assert_eq!(
        fixture
            .store
            .deliver_device_cookie(receipt, fixture.instant, wall(ISSUED))
            .unwrap_err(),
        StorageError::DeviceStoreUnavailable
    );
}

#[test]
fn verified_use_renews_a_legacy_pairing_and_expired_pairings_stay_expired() {
    let mut fixture = Fixture::new();
    let scope = host();
    let (_, cookie) = fixture.enroll_cookie(&scope);
    let path = fixture.root.join(DOCUMENT.name);
    let mut legacy: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    legacy["schema_version"] = json!(1);
    write_fixture(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
    fixture.store.reload().unwrap();
    let value = cookie_value(&cookie);

    assert!(
        fixture
            .store
            .renew_device_cookie(&scope, value, wall(EXPIRY))
            .unwrap()
            .is_none()
    );
    let renewed = fixture
        .store
        .renew_device_cookie(&scope, value, wall(EXPIRY - 1))
        .unwrap()
        .unwrap();
    assert!(renewed.as_set_cookie().ends_with("; Max-Age=2592000"));
    drop(fixture.store);

    let (mut store, _) = SettingsStore::open(&fixture.root).unwrap();
    let next_expiry = EXPIRY - 1 + CREDENTIAL_LIFETIME_SECONDS;
    assert_eq!(
        store
            .verify_device_cookie(&scope, value, wall(EXPIRY))
            .unwrap()
            .expires_at_unix_seconds(),
        next_expiry
    );
    assert!(
        store
            .renew_device_cookie(&scope, value, wall(next_expiry))
            .unwrap()
            .is_none()
    );
}

#[test]
fn metadata_is_revision_checked_and_last_seen_writes_are_bounded() {
    let mut fixture = Fixture::new();
    let scope = host();
    let (id, cookie) = fixture.enroll_cookie(&scope);
    let old = fixture.revision();
    let verified = fixture
        .store
        .verify_device_cookie(&scope, cookie_value(&cookie), wall(ISSUED))
        .unwrap();
    let revision = fixture
        .store
        .rename_device(old, id, DeviceLabel::new("Renamed phone").unwrap())
        .unwrap();
    assert!(matches!(
        fixture.store.record_device_seen(verified, wall(ISSUED)),
        Err(StorageError::DeviceRevisionConflict { .. })
    ));
    assert_eq!(
        fixture.store.devices_snapshot().unwrap().devices()[0]
            .label()
            .for_display(),
        "Renamed phone"
    );

    let verified = fixture
        .store
        .verify_device_cookie(&scope, cookie_value(&cookie), wall(ISSUED + 1))
        .unwrap();
    let seen = fixture
        .store
        .record_device_seen(verified, wall(ISSUED + 1))
        .unwrap();
    assert_ne!(revision, seen);
    assert_eq!(
        fixture.store.devices_snapshot().unwrap().devices()[0].last_seen_unix_seconds(),
        Some(ISSUED + 1)
    );
    let verified = fixture
        .store
        .verify_device_cookie(&scope, cookie_value(&cookie), wall(ISSUED + 60))
        .unwrap();
    assert_eq!(
        fixture
            .store
            .record_device_seen(verified, wall(ISSUED + 60))
            .unwrap(),
        seen
    );
    assert_ne!(
        fixture
            .store
            .record_device_seen(verified, wall(ISSUED + 61))
            .unwrap(),
        seen
    );
    assert_eq!(
        fixture.store.devices_snapshot().unwrap().devices()[0].expires_at_unix_seconds(),
        EXPIRY + 61
    );
    assert!(
        fixture
            .store
            .verify_device_cookie(&scope, cookie_value(&cookie), wall(ISSUED + 60))
            .is_none()
    );
}

struct Fail(CommitStage);

impl CommitHook for Fail {
    fn check(&self, stage: CommitStage) -> io::Result<()> {
        if self.0 == stage {
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        } else {
            Ok(())
        }
    }
}

#[test]
fn failed_enrollment_never_releases_a_cookie() {
    for stage in [
        CommitStage::CreateTemporary,
        CommitStage::WriteTemporary,
        CommitStage::SyncTemporary,
        CommitStage::ValidateTemporary,
        CommitStage::Replace,
        CommitStage::SyncDirectory,
    ] {
        let scope = host();
        let mut fixture = Fixture::new();
        fixture.store.set_commit_hook(Arc::new(Fail(stage)));
        assert!(fixture.enroll(&scope).is_err());
        let status = fixture.store.device_store_status();
        if stage == CommitStage::SyncDirectory {
            assert_eq!(status, DeviceStoreStatus::ReloadRequired);
            assert!(fixture.store.devices_snapshot().is_none());
        } else {
            assert_eq!(status, DeviceStoreStatus::Ready);
            assert_eq!(fixture.store.devices_snapshot().unwrap().devices().len(), 0);
            assert!(!fixture.root.join(DOCUMENT.name).exists());
        }
        let entries = fs::read_dir(&fixture.root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name());
        assert!(
            !entries
                .filter_map(|name| name.into_string().ok())
                .any(|name| name.starts_with(DOCUMENT.temporary_prefix))
        );
    }
}

#[test]
fn accepted_revocation_closes_access_until_durable_retry() {
    for stage in [CommitStage::Replace, CommitStage::SyncDirectory] {
        let mut fixture = Fixture::new();
        let scope = host();
        let (id, cookie) = fixture.enroll_cookie(&scope);
        let (_, other) = fixture.enroll_cookie(&scope);
        let before = fixture.revision();
        let request = fixture
            .store
            .begin_device_revocation(Some(before), DeviceRevocation::Device(id))
            .unwrap();
        assert_eq!(request.scope(), DeviceRevocation::Device(id));
        assert_eq!(
            fixture.store.device_store_status(),
            DeviceStoreStatus::Revoking
        );
        assert!(
            fixture
                .store
                .verify_device_cookie(&scope, cookie_value(&cookie), wall(ISSUED))
                .is_none()
        );

        fixture.store.set_commit_hook(Arc::new(Fail(stage)));
        assert!(fixture.store.commit_device_revocation(request).is_err());
        fixture.store.set_commit_hook(Arc::new(NoopCommitHook));
        fixture.store.reload().unwrap();
        assert_eq!(
            fixture.store.device_store_status(),
            DeviceStoreStatus::Revoking
        );
        assert!(
            fixture
                .store
                .verify_device_cookie(&scope, cookie_value(&cookie), wall(ISSUED))
                .is_none()
        );
        fixture.store.commit_device_revocation(request).unwrap();
        assert!(
            fixture
                .store
                .verify_device_cookie(&scope, cookie_value(&other), wall(ISSUED))
                .is_some()
        );
        assert_eq!(
            fixture.store.commit_device_revocation(request),
            Err(StorageError::DeviceRevocationNotStarted)
        );

        drop(fixture.store);
        let (reopened, _) = SettingsStore::open(&fixture.root).unwrap();
        assert!(
            reopened
                .verify_device_cookie(&scope, cookie_value(&cookie), wall(ISSUED))
                .is_none()
        );
        assert_eq!(reopened.devices_snapshot().unwrap().devices().len(), 1);
    }
}

#[test]
fn revocation_notice_can_release_held_input_before_device_commit() {
    let mut fixture = Fixture::new();
    let scope = host();
    let (id, cookie) = fixture.enroll_cookie(&scope);
    let dispatcher =
        InputDispatcher::start(MockInputBackend::new(), DispatcherConfig::default()).unwrap();
    let timeout = Duration::from_secs(2);
    for action in [
        InputAction::MouseButton(ButtonAction::Down(MouseButton::Left)),
        InputAction::PhysicalKey(KeyAction::Down(PhysicalKey::new(0x04).unwrap())),
    ] {
        let outcome = dispatcher.submit(action).unwrap().wait_timeout(timeout);
        assert!(matches!(outcome, Ok(DispatchOutcome::Executed { .. })));
    }

    let path = fixture.root.join(DOCUMENT.name);
    let before = fs::read(&path).unwrap();
    let request = fixture
        .store
        .begin_device_revocation(Some(fixture.revision()), DeviceRevocation::Device(id))
        .unwrap();
    assert_eq!(request.scope(), DeviceRevocation::Device(id));
    assert!(
        fixture
            .store
            .verify_device_cookie(&scope, cookie_value(&cookie), wall(ISSUED))
            .is_none()
    );
    let released = dispatcher
        .request_release(EmergencyReason::ConnectionLost)
        .unwrap()
        .wait_timeout(timeout)
        .unwrap();
    assert_eq!(released.report.attempted, 2);
    assert!(released.report.is_confirmed_neutral());
    assert!(fs::read(&path).unwrap().eq(&before));
    fixture.store.commit_device_revocation(request).unwrap();
    assert!(
        fixture
            .store
            .devices_snapshot()
            .unwrap()
            .devices()
            .is_empty()
    );
    assert!(dispatcher.shutdown().backend.is_neutral());
}

#[test]
fn revoke_all_supersedes_old_request_and_factory_rotation_clears_old_scope() {
    let mut fixture = Fixture::new();
    let scope = host();
    let (id, cookie) = fixture.enroll_cookie(&scope);
    let revision = fixture.revision();
    let old_identity = fixture.store.identity_snapshot().unwrap().clone();
    let old_request = fixture
        .store
        .begin_device_revocation(Some(revision), DeviceRevocation::Device(id))
        .unwrap();
    let all = fixture
        .store
        .begin_device_revocation(Some(revision), DeviceRevocation::All)
        .unwrap();
    assert_eq!(
        fixture.store.commit_device_revocation(old_request),
        Err(StorageError::StaleDeviceRevocation)
    );
    assert_eq!(
        fixture.store.rotate_identity(old_identity.revision()),
        Err(StorageError::IdentityResetBlockedByPairedDevices)
    );
    fixture
        .store
        .remove_device_store_for_identity_reset(all)
        .unwrap();
    let rotated = fixture
        .store
        .rotate_identity(old_identity.revision())
        .unwrap();
    assert_ne!(rotated.identity(), old_identity.identity());
    assert_eq!(
        fixture.store.device_store_status(),
        DeviceStoreStatus::Revoking
    );
    fixture.store.commit_device_revocation(all).unwrap();
    assert!(
        fixture
            .store
            .verify_device_cookie(&scope, cookie_value(&cookie), wall(ISSUED))
            .is_none()
    );
    assert_eq!(fixture.store.devices_snapshot().unwrap().devices().len(), 0);
}

#[test]
fn failed_factory_removal_keeps_admission_closed_and_requires_reconciliation() {
    let mut fixture = Fixture::new();
    let scope = host();
    let (_, cookie) = fixture.enroll_cookie(&scope);
    let request = fixture
        .store
        .begin_device_revocation(Some(fixture.revision()), DeviceRevocation::All)
        .unwrap();
    let path = fixture.root.join(DOCUMENT.name);
    fixture
        .store
        .set_commit_hook(Arc::new(Fail(CommitStage::Replace)));
    assert!(
        fixture
            .store
            .remove_device_store_for_identity_reset(request)
            .is_err()
    );
    assert!(path.exists());
    fixture
        .store
        .set_commit_hook(Arc::new(Fail(CommitStage::SyncDirectory)));
    assert_eq!(
        fixture
            .store
            .remove_device_store_for_identity_reset(request),
        Err(StorageError::IndeterminateCommit)
    );
    assert!(!path.exists());
    assert_eq!(
        fixture
            .store
            .remove_device_store_for_identity_reset(request),
        Err(StorageError::ReloadRequired)
    );
    fixture.store.reload().unwrap();
    assert_eq!(
        fixture.store.device_store_status(),
        DeviceStoreStatus::Revoking
    );
    assert!(
        fixture
            .store
            .verify_device_cookie(&scope, cookie_value(&cookie), wall(ISSUED))
            .is_none()
    );
    assert_eq!(
        fixture
            .store
            .remove_device_store_for_identity_reset(request),
        Err(StorageError::IndeterminateCommit)
    );
    fixture.store.set_commit_hook(Arc::new(NoopCommitHook));
    fixture.store.reload().unwrap();
    fixture
        .store
        .remove_device_store_for_identity_reset(request)
        .unwrap();
    assert_eq!(
        fixture.store.device_store_status(),
        DeviceStoreStatus::Revoking
    );
}

#[test]
fn corrupt_device_file_stays_in_recovery_across_restart_until_confirmed() {
    let mut fixture = Fixture::new();
    let scope = host();
    let (_, cookie) = fixture.enroll_cookie(&scope);
    let path = fixture.root.join(DOCUMENT.name);
    fs::write(&path, b"{ invalid fixture").unwrap();
    drop(fixture.store);
    for _ in 0..2 {
        let (store, _) = SettingsStore::open(&fixture.root).unwrap();
        assert_eq!(
            store.device_store_status(),
            DeviceStoreStatus::RecoveryRequired
        );
        assert!(
            store
                .verify_device_cookie(&scope, cookie_value(&cookie), wall(ISSUED))
                .is_none()
        );
        assert!(!path.exists());
    }
    let (mut store, _) = SettingsStore::open(&fixture.root).unwrap();
    let request = store
        .begin_device_revocation(None, DeviceRevocation::All)
        .unwrap();
    store.commit_device_revocation(request).unwrap();
    drop(store);
    let (store, _) = SettingsStore::open(&fixture.root).unwrap();
    assert_eq!(store.device_store_status(), DeviceStoreStatus::Ready);
    assert_eq!(store.devices_snapshot().unwrap().devices().len(), 0);
}

#[test]
fn future_device_schema_is_left_untouched() {
    let fixture = Fixture::new();
    let path = fixture.root.join(DOCUMENT.name);
    let future = br#"{"format":"tether.paired-devices","schema_version":3,"revision":2,"payload":{"future":true}}"#;
    write_fixture(&path, future).unwrap();
    drop(fixture.store);
    let (mut store, _) = SettingsStore::open(&fixture.root).unwrap();
    assert_eq!(
        store.device_store_status(),
        DeviceStoreStatus::UnsupportedSchema { found: 3 }
    );
    let request = store
        .begin_device_revocation(None, DeviceRevocation::All)
        .unwrap();
    assert_eq!(
        store.commit_device_revocation(request),
        Err(unsupported_schema(3))
    );
    assert_eq!(fs::read(path).unwrap(), future);
}

#[test]
fn strict_file_validation_rejects_duplicate_credentials_and_cross_installation_data() {
    let installation = "00".repeat(32);
    let mut file = json!({
        "format": FORMAT, "schema_version": SCHEMA_VERSION, "revision": 1,
        "payload": { "installation_id": installation, "devices": [{
            "id": "aa".repeat(16), "label": "Fixture", "host": host().host_name(),
            "credential_sha256": "bb".repeat(32), "issued_at": ISSUED, "expires_at": EXPIRY, "last_seen": null
        }] }
    });
    let serialized = serde_json::to_vec(&file).unwrap();
    assert_eq!(
        decode_snapshot(&serialized, &installation)
            .unwrap()
            .devices()
            .len(),
        1
    );
    assert_eq!(
        decode_snapshot(&serialized, &"01".repeat(32)).unwrap_err(),
        invalid_document()
    );
    let records = file["payload"]["devices"].as_array_mut().unwrap();
    let mut duplicate = records[0].clone();
    duplicate["id"] = json!("cc".repeat(16));
    records.push(duplicate);
    assert_eq!(
        decode_snapshot(&serde_json::to_vec(&file).unwrap(), &installation).unwrap_err(),
        invalid_document()
    );
    let record = file["payload"]["devices"][0].clone();
    file["payload"]["devices"] = json!(vec![record; MAX_DEVICES + 1]);
    assert!(matches!(
        decode_snapshot(&serde_json::to_vec(&file).unwrap(), &installation),
        Err(StorageError::Document {
            problem: StorageProblem::MalformedJson,
            ..
        })
    ));
}
