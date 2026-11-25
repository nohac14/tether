use super::{
    PAIRED_DEVICES_FILE_NAME, SettingsStore, StorageError, StorageFileKind, StorageIoOperation,
    StorageProblem, StoreDocument, cleanup_abandoned_temporaries, document_error,
    entry_exists_no_follow, finish_directory_commit, has_quarantine_evidence, io_error,
    quarantine_document, read_bounded, validate_regular_file,
};
use crate::identity::{InstallationIdentity, encode_lower_hex};
use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tether_pairing::{ApprovedPairing, CredentialDigest, DeviceLabel, HostScope};

mod document;

use document::{decode_snapshot, encode_snapshot};

const FORMAT: &str = "tether.paired-devices";
const SCHEMA_VERSION: u32 = 2;
const FILE_LIMIT: usize = 1_048_576;
pub const MAX_DEVICES: usize = 128;
const CREDENTIAL_LIFETIME_SECONDS: u64 = 30 * 24 * 60 * 60;
const SEEN_WRITE_INTERVAL_SECONDS: u64 = 60;
const DOCUMENT: StoreDocument = StoreDocument {
    name: PAIRED_DEVICES_FILE_NAME,
    kind: StorageFileKind::PairedDevices,
    temporary_prefix: ".paired-devices.tmp-",
    temporary_kind: StorageFileKind::TemporaryPairedDevices,
    quarantine_prefix: "devices-corrupt-",
};
static NEXT_REVOCATION: AtomicU64 = AtomicU64::new(1);

/// Opaque installation-local identifier for one paired device.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct DeviceId([u8; 16]);

impl DeviceId {
    /// Returns the opaque identifier for intended local administration.
    #[must_use]
    pub fn for_local_admin(self) -> String {
        encode_lower_hex(&self.0)
    }
}

impl fmt::Debug for DeviceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DeviceId(<redacted>)")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceRevision(u64);

impl DeviceRevision {
    const INITIAL: Self = Self(1);

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    fn next(self) -> Result<Self, StorageError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(StorageError::RevisionExhausted)
    }
}

/// Publicly readable device metadata. Credential digests stay private.
#[derive(Clone)]
pub struct PairedDevice {
    id: DeviceId,
    label: DeviceLabel,
    host: HostScope,
    digest: CredentialDigest,
    issued_at: u64,
    expires_at: u64,
    last_seen: Option<u64>,
}

impl PairedDevice {
    #[must_use]
    pub const fn id(&self) -> DeviceId {
        self.id
    }

    #[must_use]
    pub const fn label(&self) -> &DeviceLabel {
        &self.label
    }

    #[must_use]
    pub const fn host(&self) -> &HostScope {
        &self.host
    }

    #[must_use]
    pub const fn expires_at_unix_seconds(&self) -> u64 {
        self.expires_at
    }

    #[must_use]
    pub const fn last_seen_unix_seconds(&self) -> Option<u64> {
        self.last_seen
    }

    fn valid_at(&self, now: u64) -> bool {
        (self.issued_at..self.expires_at).contains(&now)
            && self.last_seen.is_none_or(|seen| seen <= now)
    }

    #[must_use]
    pub fn is_valid_at(&self, now: SystemTime) -> bool {
        unix_seconds(now).is_ok_and(|now| self.valid_at(now))
    }
}

#[derive(Clone)]
pub struct DevicesSnapshot {
    revision: DeviceRevision,
    records: Vec<PairedDevice>,
}

impl DevicesSnapshot {
    const fn empty() -> Self {
        Self {
            revision: DeviceRevision::INITIAL,
            records: Vec::new(),
        }
    }

    #[must_use]
    pub const fn revision(&self) -> DeviceRevision {
        self.revision
    }

    #[must_use]
    pub fn devices(&self) -> &[PairedDevice] {
        &self.records
    }

    fn device_mut(&mut self, id: DeviceId) -> Result<&mut PairedDevice, StorageError> {
        self.records
            .iter_mut()
            .find(|device| device.id == id)
            .ok_or(StorageError::DeviceNotFound)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceStoreStatus {
    Ready,
    RecoveryRequired,
    UnsupportedSchema { found: u32 },
    IdentityUnavailable,
    ReloadRequired,
    Revoking,
}

pub(super) enum DeviceStoreState {
    Ready(DevicesSnapshot),
    Recovery,
    Unsupported(u32),
    Unbound,
}

/// Typed invalidation notice for the core's session teardown path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceRevocation {
    Device(DeviceId),
    All,
}

/// Identifies one accepted invalidation even if its disk operation is retried.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RevocationRequest {
    serial: u64,
    scope: DeviceRevocation,
}

impl RevocationRequest {
    #[must_use]
    pub const fn scope(self) -> DeviceRevocation {
        self.scope
    }
}

/// Evidence of matching a credential against this device-store revision.
/// Transport and session authorization remain separate checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedDevice {
    id: DeviceId,
    revision: DeviceRevision,
    expires_at: u64,
}

impl VerifiedDevice {
    #[must_use]
    pub const fn id(self) -> DeviceId {
        self.id
    }

    #[must_use]
    pub const fn revision(self) -> DeviceRevision {
        self.revision
    }

    #[must_use]
    pub const fn expires_at_unix_seconds(self) -> u64 {
        self.expires_at
    }
}

/// Single-use, memory-only result of a durable enrollment.
pub struct DeviceEnrollment {
    id: DeviceId,
    approval: ApprovedPairing,
    secret: [u8; 32],
}

impl DeviceEnrollment {
    #[must_use]
    pub const fn device_id(&self) -> DeviceId {
        self.id
    }
}

/// Header value for the intended HTTP response. Keep out of diagnostics.
pub struct DeviceCookie(String);

impl DeviceCookie {
    pub const NAME: &str = "tether_device";

    /// Host-only expiration header for a rejected control credential.
    #[must_use]
    pub fn clear() -> Self {
        Self(format!("{}; Max-Age=0", cookie_attributes("")))
    }

    #[must_use]
    pub fn as_set_cookie(&self) -> &str {
        &self.0
    }
}

fn cookie_attributes(value: &str) -> String {
    format!(
        "{}={value}; HttpOnly; SameSite=Strict; Path=/control",
        DeviceCookie::NAME
    )
}

macro_rules! redacted {
    ($($name:ident),+ $(,)?) => {$(
        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($name), "(<redacted>)"))
            }
        }
    )+};
}

redacted!(
    PairedDevice,
    DevicesSnapshot,
    DeviceEnrollment,
    DeviceCookie
);

impl SettingsStore {
    #[must_use]
    pub const fn device_store_status(&self) -> DeviceStoreStatus {
        if self.reload_required {
            return DeviceStoreStatus::ReloadRequired;
        }
        if self.device_revocation.is_some() {
            return DeviceStoreStatus::Revoking;
        }
        match self.devices {
            DeviceStoreState::Ready(_) => DeviceStoreStatus::Ready,
            DeviceStoreState::Recovery => DeviceStoreStatus::RecoveryRequired,
            DeviceStoreState::Unsupported(found) => DeviceStoreStatus::UnsupportedSchema { found },
            DeviceStoreState::Unbound => DeviceStoreStatus::IdentityUnavailable,
        }
    }

    /// The current list and revision for local administration.
    #[must_use]
    pub const fn devices_snapshot(&self) -> Option<&DevicesSnapshot> {
        if self.reload_required {
            return None;
        }
        match &self.devices {
            DeviceStoreState::Ready(snapshot) => Some(snapshot),
            _ => None,
        }
    }

    /// Uses one current snapshot for host, expiry and constant-time digest checks.
    /// The caller supplies the single cookie and the validated browser host.
    #[must_use]
    pub fn verify_device_cookie(
        &self,
        host: &HostScope,
        value: &str,
        now: SystemTime,
    ) -> Option<VerifiedDevice> {
        self.verify_digest(
            host,
            &CredentialDigest::from_cookie_value(value)?,
            unix_seconds(now).ok()?,
        )
        .map(|(verified, _)| verified)
    }

    fn verify_digest(
        &self,
        host: &HostScope,
        digest: &CredentialDigest,
        now: u64,
    ) -> Option<(VerifiedDevice, &PairedDevice)> {
        if self.device_revocation.is_some()
            || self.snapshot().is_none()
            || self.identity_snapshot().is_none()
        {
            return None;
        }
        let snapshot = self.devices_snapshot()?;
        let device = snapshot.records.iter().find(|device| {
            device.host == *host && device.valid_at(now) && device.digest.matches_digest(digest)
        })?;
        Some((
            VerifiedDevice {
                id: device.id,
                revision: snapshot.revision,
                expires_at: device.expires_at,
            },
            device,
        ))
    }

    /// Persists only the digest and typed metadata. Callers must revalidate the
    /// current local approval and transport generation immediately beforehand.
    ///
    /// # Errors
    /// Fails closed for unavailable storage, expiration, stale revisions, full
    /// capacity, OS entropy failure and unsuccessful durable commit.
    pub fn enroll_device(
        &mut self,
        expected: DeviceRevision,
        approval: ApprovedPairing,
        monotonic_now: Instant,
        now: SystemTime,
    ) -> Result<DeviceEnrollment, StorageError> {
        let Some(_) = self.snapshot() else {
            return Err(StorageError::DeviceStoreUnavailable);
        };
        self.check_device_mutation(expected)?;
        let request = approval.approval();
        if monotonic_now >= request.expires_at() {
            return Err(StorageError::ApprovalExpired);
        }
        let issued_at = unix_seconds(now)?;
        let expires_at = issued_at
            .checked_add(CREDENTIAL_LIFETIME_SECONDS)
            .ok_or(StorageError::InvalidDeviceTime)?;
        let mut snapshot = self.ready_devices()?.clone();
        if snapshot.records.len() >= MAX_DEVICES {
            return Err(StorageError::DeviceLimitReached);
        }

        let mut identifier = [0; 16];
        let mut secret = [0; 32];
        self.entropy.fill(&mut identifier)?;
        self.entropy.fill(&mut secret)?;
        let id = DeviceId(identifier);
        let digest = CredentialDigest::for_enrollment(&secret);
        if identifier == [0; 16]
            || snapshot
                .records
                .iter()
                .any(|device| device.id == id || device.digest.matches_digest(&digest))
        {
            return Err(StorageError::EntropyUnavailable);
        }
        snapshot.revision = snapshot.revision.next()?;
        snapshot.records.push(PairedDevice {
            id,
            label: request.label().clone(),
            host: request.host().clone(),
            digest,
            issued_at,
            expires_at,
            last_seen: None,
        });
        self.persist_devices(snapshot)?;
        Ok(DeviceEnrollment {
            id,
            approval,
            secret,
        })
    }

    /// Consumes a durable enrollment and produces the one intended cookie.
    /// The core rechecks the approval and transport generation before calling.
    /// If it cannot publish the cookie, it reconciles the stored credential.
    ///
    /// # Errors
    /// Rejects expired approvals or credentials, invalid time, a store needing
    /// reload, a pending revocation and any matching credential that was removed.
    pub fn deliver_device_cookie(
        &self,
        enrollment: DeviceEnrollment,
        monotonic_now: Instant,
        now: SystemTime,
    ) -> Result<DeviceCookie, StorageError> {
        let DeviceEnrollment {
            id,
            approval,
            secret,
        } = enrollment;
        let request = approval.approval();
        if monotonic_now >= request.expires_at() {
            return Err(StorageError::ApprovalExpired);
        }
        let now = unix_seconds(now)?;
        let digest = CredentialDigest::for_enrollment(&secret);
        let (_, device) = self
            .verify_digest(request.host(), &digest, now)
            .filter(|(_, record)| record.id == id)
            .ok_or(StorageError::DeviceStoreUnavailable)?;
        let max_age = device.expires_at - now;
        let secret = encode_lower_hex(&secret);
        Ok(DeviceCookie(format!(
            "{}; Max-Age={max_age}",
            cookie_attributes(&secret)
        )))
    }

    /// Renames one device using the current device-store revision.
    ///
    /// # Errors
    /// Rejects a missing device, stale revision or unavailable storage.
    pub fn rename_device(
        &mut self,
        expected: DeviceRevision,
        id: DeviceId,
        label: DeviceLabel,
    ) -> Result<DeviceRevision, StorageError> {
        self.check_device_mutation(expected)?;
        let mut snapshot = self.ready_devices()?.clone();
        let device = snapshot.device_mut(id)?;
        if device.label == label {
            return Ok(expected);
        }
        device.label = label;
        snapshot.revision = snapshot.revision.next()?;
        self.persist_devices(snapshot)
    }

    /// Extends an unexpired credential after verified use, at most once per
    /// minute per device. Call this from the storage actor.
    ///
    /// # Errors
    /// Rejects stale verification, invalid time or unavailable storage.
    pub fn record_device_seen(
        &mut self,
        verified: VerifiedDevice,
        now: SystemTime,
    ) -> Result<DeviceRevision, StorageError> {
        self.check_device_mutation(verified.revision)?;
        let mut snapshot = self.ready_devices()?.clone();
        let device = snapshot.device_mut(verified.id)?;
        let seen = unix_seconds(now)?;
        if !device.valid_at(seen) || device.last_seen.is_some_and(|last| seen < last) {
            return Err(StorageError::InvalidDeviceTime);
        }
        if device
            .last_seen
            .is_some_and(|last| seen - last < SEEN_WRITE_INTERVAL_SECONDS)
        {
            return Ok(snapshot.revision);
        }
        device.last_seen = Some(seen);
        device.expires_at = seen
            .checked_add(CREDENTIAL_LIFETIME_SECONDS)
            .ok_or(StorageError::InvalidDeviceTime)?;
        snapshot.revision = snapshot.revision.next()?;
        self.persist_devices(snapshot)
    }

    /// Renews a valid host-scoped credential and returns its durable expiry
    /// for a refreshed browser cookie. No credential is issued on failure.
    ///
    /// # Errors
    /// Returns the underlying storage error if the renewal could not commit.
    pub fn renew_device_cookie(
        &mut self,
        host: &HostScope,
        value: &str,
        now: SystemTime,
    ) -> Result<Option<DeviceCookie>, StorageError> {
        let Some(verified) = self.verify_device_cookie(host, value, now) else {
            return Ok(None);
        };
        self.record_device_seen(verified, now)?;
        let renewed = self
            .verify_device_cookie(host, value, now)
            .ok_or(StorageError::DeviceStoreUnavailable)?;
        let max_age = renewed.expires_at - unix_seconds(now)?;
        Ok(Some(DeviceCookie(format!(
            "{}; Max-Age={max_age}",
            cookie_attributes(value)
        ))))
    }

    /// Closes credential verification immediately and returns the invalidation
    /// for the core's active-session teardown. This step performs no I/O.
    ///
    /// # Errors
    /// Rejects stale revisions, unknown devices, unavailable storage and
    /// conflicting revocations. A confirmed all-device reset can start in
    /// recovery even without a readable device revision.
    pub fn begin_device_revocation(
        &mut self,
        expected: Option<DeviceRevision>,
        scope: DeviceRevocation,
    ) -> Result<RevocationRequest, StorageError> {
        if self.reload_required {
            return Err(StorageError::ReloadRequired);
        }
        if let Some(pending) = self.device_revocation {
            if pending.scope == scope {
                return Ok(pending);
            }
            if scope != DeviceRevocation::All {
                return Err(StorageError::DeviceRevocationPending);
            }
        }
        self.check_device_revision(expected)?;
        if let DeviceRevocation::Device(id) = scope
            && !self
                .ready_devices()?
                .records
                .iter()
                .any(|device| device.id == id)
        {
            return Err(StorageError::DeviceNotFound);
        }
        let serial = NEXT_REVOCATION
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| StorageError::RevisionExhausted)?;
        let request = RevocationRequest { serial, scope };
        self.device_revocation = Some(request);
        Ok(request)
    }

    /// Persists the pending invalidation. The core invokes this after starting
    /// its ordered session teardown. Failure leaves verification closed.
    ///
    /// # Errors
    /// Rejects missing revocation, unsupported schema, unavailable identity or
    /// storage failures. An indeterminate commit needs a locked reload first.
    pub fn commit_device_revocation(
        &mut self,
        request: RevocationRequest,
    ) -> Result<DeviceRevision, StorageError> {
        self.check_revocation(request)?;
        let scope = request.scope;
        let next = match &self.devices {
            DeviceStoreState::Ready(snapshot) => {
                let mut next = snapshot.clone();
                match scope {
                    DeviceRevocation::All => next.records.clear(),
                    DeviceRevocation::Device(id) => next.records.retain(|device| device.id != id),
                }
                next.revision = next.revision.next()?;
                next
            }
            DeviceStoreState::Recovery if scope == DeviceRevocation::All => {
                DevicesSnapshot::empty()
            }
            DeviceStoreState::Unsupported(found) => return Err(unsupported_schema(*found)),
            DeviceStoreState::Recovery | DeviceStoreState::Unbound => {
                return Err(StorageError::DeviceStoreUnavailable);
            }
        };
        let revision = self.persist_devices(next)?;
        self.device_revocation = None;
        Ok(revision)
    }

    /// Removes only the paired-device document during an already-started
    /// factory reset. The core must first confirm teardown and neutral input.
    /// The pending all-device gate stays closed through identity rotation.
    ///
    /// # Errors
    /// Rejects removal without an all-device invalidation or after an uncertain
    /// commit. Invalid filesystem entries and any I/O errors fail closed.
    pub fn remove_device_store_for_identity_reset(
        &mut self,
        request: RevocationRequest,
    ) -> Result<(), StorageError> {
        self.check_revocation(request)?;
        if request.scope != DeviceRevocation::All {
            return Err(StorageError::StaleDeviceRevocation);
        }
        let path = self.root.join(DOCUMENT.name);
        if entry_exists_no_follow(&path, DOCUMENT.kind, StorageIoOperation::InspectEntry)? {
            validate_regular_file(&path, DOCUMENT.kind)?;
            let opened = super::open_read_file(&path)
                .map_err(|error| io_error(DOCUMENT.kind, StorageIoOperation::Read, error))?;
            super::validate_opened_file(&path, &opened, DOCUMENT.kind)?;
            super::tighten_open_file_permissions(&opened, DOCUMENT.kind)?;
            super::validate_opened_file(&path, &opened, DOCUMENT.kind)?;
            drop(opened);
            self.commit_hook
                .check(super::CommitStage::Replace)
                .and_then(|()| fs::remove_file(path))
                .map_err(|error| io_error(DOCUMENT.kind, StorageIoOperation::Remove, error))?;
        }
        if self
            .commit_hook
            .check(super::CommitStage::SyncDirectory)
            .and_then(|()| finish_directory_commit(&self.root))
            .is_err()
        {
            self.reload_required = true;
            return Err(StorageError::IndeterminateCommit);
        }
        self.devices = DeviceStoreState::Unbound;
        Ok(())
    }

    fn check_revocation(&self, request: RevocationRequest) -> Result<(), StorageError> {
        if self.reload_required {
            return Err(StorageError::ReloadRequired);
        }
        match self.device_revocation {
            Some(pending) if pending == request => Ok(()),
            Some(_) => Err(StorageError::StaleDeviceRevocation),
            None => Err(StorageError::DeviceRevocationNotStarted),
        }
    }

    fn ready_devices(&self) -> Result<&DevicesSnapshot, StorageError> {
        match &self.devices {
            DeviceStoreState::Ready(snapshot) => Ok(snapshot),
            DeviceStoreState::Unsupported(found) => Err(unsupported_schema(*found)),
            DeviceStoreState::Recovery | DeviceStoreState::Unbound => {
                Err(StorageError::DeviceStoreUnavailable)
            }
        }
    }

    fn check_device_revision(&self, expected: Option<DeviceRevision>) -> Result<(), StorageError> {
        let actual = self.devices_snapshot().map(DevicesSnapshot::revision);
        if actual != expected {
            return Err(StorageError::DeviceRevisionConflict { expected, actual });
        }
        Ok(())
    }

    fn check_device_mutation(&self, expected: DeviceRevision) -> Result<(), StorageError> {
        if self.reload_required {
            return Err(StorageError::ReloadRequired);
        }
        if self.device_revocation.is_some() {
            return Err(StorageError::DeviceRevocationPending);
        }
        self.ready_devices()?;
        self.check_device_revision(Some(expected))
    }

    fn persist_devices(
        &mut self,
        snapshot: DevicesSnapshot,
    ) -> Result<DeviceRevision, StorageError> {
        let identity = self
            .identity_snapshot()
            .ok_or(StorageError::IdentityResetRequired)?
            .identity();
        let installation = identity.encoded_installation_id();
        let bytes = encode_snapshot(&snapshot, &installation)?;
        self.commit_document(DOCUMENT, &bytes, |path| {
            let candidate = read_bounded(path, DOCUMENT.temporary_kind, FILE_LIMIT)?;
            decode_snapshot(&candidate, &installation)?;
            if candidate != bytes {
                return Err(invalid_document());
            }
            Ok(())
        })?;
        let revision = snapshot.revision;
        self.devices = DeviceStoreState::Ready(snapshot);
        Ok(revision)
    }

    pub(super) fn load_devices(&mut self) -> Result<(), StorageError> {
        let Some(identity) = self
            .identity_snapshot()
            .map(super::IdentitySnapshot::identity)
        else {
            self.devices = DeviceStoreState::Unbound;
            return Ok(());
        };
        self.devices = load_snapshot(&self.root, identity)?;
        Ok(())
    }

    pub(super) fn reset_device_state_for_identity(&mut self) {
        self.devices = DeviceStoreState::Ready(DevicesSnapshot::empty());
    }
}

pub(super) fn cleanup_device_temporaries(root: &Path) -> Result<(), StorageError> {
    cleanup_abandoned_temporaries(root, DOCUMENT.temporary_prefix, DOCUMENT.temporary_kind)
}

fn load_snapshot(
    root: &Path,
    identity: &InstallationIdentity,
) -> Result<DeviceStoreState, StorageError> {
    let path = root.join(DOCUMENT.name);
    if !entry_exists_no_follow(&path, DOCUMENT.kind, StorageIoOperation::Read)? {
        return if has_quarantine_evidence(root, DOCUMENT.quarantine_prefix)? {
            Ok(DeviceStoreState::Recovery)
        } else {
            Ok(DeviceStoreState::Ready(DevicesSnapshot::empty()))
        };
    }
    let bytes = read_bounded(&path, DOCUMENT.kind, FILE_LIMIT);
    match bytes.and_then(|bytes| decode_snapshot(&bytes, &identity.encoded_installation_id())) {
        Ok(snapshot) => Ok(DeviceStoreState::Ready(snapshot)),
        Err(StorageError::UnsupportedDeviceSchema { found, .. }) => {
            Ok(DeviceStoreState::Unsupported(found))
        }
        Err(error) if error.is_corrupt_document() => {
            quarantine_document(root, DOCUMENT)?;
            Ok(DeviceStoreState::Recovery)
        }
        Err(error) => Err(error),
    }
}

const fn invalid_document() -> StorageError {
    document_error(StorageProblem::InvalidPairedDevices)
}

const fn unsupported_schema(found: u32) -> StorageError {
    StorageError::UnsupportedDeviceSchema {
        found,
        current: SCHEMA_VERSION,
    }
}

fn unix_seconds(time: SystemTime) -> Result<u64, StorageError> {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| StorageError::InvalidDeviceTime)
}

#[cfg(test)]
mod tests;
