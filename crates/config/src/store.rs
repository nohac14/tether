use crate::SettingsValidationError;
use crate::identity::{
    HOSTNAME_SEED_BYTES, INSTALLATION_ID_BYTES, IdentityValidationError, InstallationIdentity,
};
use crate::settings::{DesktopSettings, SettingsPayloadV1, SettingsPayloadV2};
use serde::{Deserialize, Serialize, de::IgnoredAny};
use std::collections::hash_map::RandomState;
#[cfg(not(windows))]
use std::fs::OpenOptions;
use std::fs::{self, File, TryLockError};
use std::hash::{BuildHasher, Hasher};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
mod windows_storage;

const SETTINGS_FORMAT: &str = "tether.settings";
const IDENTITY_FORMAT: &str = "tether.identity";
const SETTINGS_FILE_NAME: &str = "settings.json";
const IDENTITY_FILE_NAME: &str = "identity.json";
const PAIRED_DEVICES_FILE_NAME: &str = "paired-devices.json";
const LOCK_FILE_NAME: &str = ".instance.lock";
const QUARANTINE_DIRECTORY_NAME: &str = "quarantine";
const SETTINGS_TEMP_PREFIX: &str = ".settings.tmp-";
const IDENTITY_TEMP_PREFIX: &str = ".identity.tmp-";
const IDENTITY_QUARANTINE_PREFIX: &str = "identity-corrupt-";
const MAX_TEMP_ATTEMPTS: u8 = 64;

#[derive(Clone, Copy)]
struct StoreDocument {
    name: &'static str,
    kind: StorageFileKind,
    temporary_prefix: &'static str,
    temporary_kind: StorageFileKind,
    quarantine_prefix: &'static str,
}

const SETTINGS_DOCUMENT: StoreDocument = StoreDocument {
    name: SETTINGS_FILE_NAME,
    kind: StorageFileKind::Settings,
    temporary_prefix: SETTINGS_TEMP_PREFIX,
    temporary_kind: StorageFileKind::TemporarySettings,
    quarantine_prefix: "corrupt-",
};
const IDENTITY_DOCUMENT: StoreDocument = StoreDocument {
    name: IDENTITY_FILE_NAME,
    kind: StorageFileKind::Identity,
    temporary_prefix: IDENTITY_TEMP_PREFIX,
    temporary_kind: StorageFileKind::TemporaryIdentity,
    quarantine_prefix: IDENTITY_QUARANTINE_PREFIX,
};

mod devices;
use devices::DeviceStoreState;
pub use devices::{
    DeviceCookie, DeviceEnrollment, DeviceId, DeviceRevision, DeviceRevocation, DeviceStoreStatus,
    DevicesSnapshot, MAX_DEVICES, PairedDevice, RevocationRequest, VerifiedDevice,
};

/// Current on-disk settings schema.
pub const CURRENT_SETTINGS_SCHEMA_VERSION: u32 = 2;

/// Current on-disk installation-identity schema.
pub const CURRENT_IDENTITY_SCHEMA_VERSION: u32 = 1;

/// Maximum encoded settings file size.
pub const SETTINGS_FILE_LIMIT: usize = 256 * 1_024;

/// Maximum encoded identity file size.
pub const IDENTITY_FILE_LIMIT: usize = 16 * 1_024;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Owned storage file category safe for diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageFileKind {
    Root,
    ProcessLock,
    Settings,
    Identity,
    PairedDevices,
    Quarantine,
    TemporarySettings,
    TemporaryIdentity,
    TemporaryPairedDevices,
}

/// Storage I/O phase without a path or content value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageIoOperation {
    CreateDirectory,
    InspectEntry,
    TightenPermissions,
    OpenLock,
    AcquireLock,
    Read,
    CreateTemporary,
    WriteTemporary,
    SyncTemporary,
    ValidateTemporary,
    Replace,
    SyncDirectory,
    Quarantine,
    CleanupTemporary,
    Remove,
}

/// Non-sensitive document or filesystem problem category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageProblem {
    MalformedJson,
    InvalidEnvelope,
    InvalidFormat,
    InvalidRevision,
    InvalidSettings,
    InvalidIdentity,
    InvalidPairedDevices,
    EncodedLimitExceeded,
    UnsafeEntryKind,
    HardLinkDetected,
}

/// Closed storage failure. It intentionally retains no path, JSON excerpt, or
/// operating-system error message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageError {
    Io {
        file: StorageFileKind,
        operation: StorageIoOperation,
        kind: io::ErrorKind,
    },
    LockHeld,
    Document {
        problem: StorageProblem,
        line: Option<u64>,
        column: Option<u64>,
    },
    UnsupportedSchema {
        found: u32,
        current: u32,
    },
    UnsupportedIdentitySchema {
        found: u32,
        current: u32,
    },
    UnsupportedDeviceSchema {
        found: u32,
        current: u32,
    },
    RevisionConflict {
        expected: SettingsRevision,
        actual: SettingsRevision,
    },
    RevisionExhausted,
    IdentityRevisionConflict {
        expected: IdentityRevision,
        actual: IdentityRevision,
    },
    RecoveryConfirmationRequired,
    IdentityResetRequired,
    IdentityResetNotRequired,
    IdentityResetBlockedByPairedDevices,
    DeviceStoreUnavailable,
    DeviceNotFound,
    DeviceLimitReached,
    InvalidDeviceTime,
    ApprovalExpired,
    DeviceRevocationPending,
    DeviceRevocationNotStarted,
    StaleDeviceRevocation,
    DeviceRevisionConflict {
        expected: Option<DeviceRevision>,
        actual: Option<DeviceRevision>,
    },
    ReloadRequired,
    IndeterminateCommit,
    EntropyUnavailable,
    PlatformPermissionEnforcementUnavailable,
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Io { .. } => "local storage I/O failed",
            Self::LockHeld => "local storage is already owned by another process",
            Self::Document { .. } => "local storage document is invalid",
            Self::UnsupportedSchema { .. } => "settings schema is not supported",
            Self::UnsupportedIdentitySchema { .. } => "identity schema is not supported",
            Self::UnsupportedDeviceSchema { .. } => "paired-device schema is not supported",
            Self::RevisionConflict { .. } => "settings revision is stale",
            Self::RevisionExhausted => "local storage revision space is exhausted",
            Self::IdentityRevisionConflict { .. } => "identity revision is stale",
            Self::RecoveryConfirmationRequired => "settings recovery requires confirmation",
            Self::IdentityResetRequired => "installation identity reset requires confirmation",
            Self::IdentityResetNotRequired => "installation identity reset is not pending",
            Self::IdentityResetBlockedByPairedDevices => {
                "paired devices must be invalidated before identity reset"
            }
            Self::DeviceStoreUnavailable => "paired-device storage is unavailable",
            Self::DeviceNotFound => "paired device is unavailable",
            Self::DeviceLimitReached => "paired-device limit reached",
            Self::InvalidDeviceTime => "invalid paired-device time",
            Self::ApprovalExpired => "pairing approval has expired",
            Self::DeviceRevocationPending => "paired-device revocation is pending",
            Self::DeviceRevocationNotStarted => "paired-device revocation has not started",
            Self::StaleDeviceRevocation => "paired-device revocation request is stale",
            Self::DeviceRevisionConflict { .. } => "paired-device revision is stale",
            Self::ReloadRequired => "settings must be reloaded before another mutation",
            Self::IndeterminateCommit => "settings replacement requires a reload",
            Self::EntropyUnavailable => "operating-system entropy is unavailable",
            Self::PlatformPermissionEnforcementUnavailable => {
                "owner-only storage permission enforcement is unavailable"
            }
        })
    }
}

impl std::error::Error for StorageError {}

impl StorageError {
    const fn is_corrupt_document(self) -> bool {
        matches!(
            self,
            Self::Document { .. }
                | Self::Io {
                    operation: StorageIoOperation::Read,
                    kind: io::ErrorKind::InvalidData,
                    ..
                }
        )
    }
}

/// Positive committed revision.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SettingsRevision(u64);

impl SettingsRevision {
    const INITIAL: Self = Self(1);

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    fn from_stored(value: u64) -> Result<Self, StorageError> {
        if value == 0 {
            Err(document_error(StorageProblem::InvalidRevision))
        } else {
            Ok(Self(value))
        }
    }

    fn next(self) -> Result<Self, StorageError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(StorageError::RevisionExhausted)
    }
}

/// Positive committed installation-identity revision.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct IdentityRevision(u64);

impl IdentityRevision {
    const INITIAL: Self = Self(1);

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    fn from_stored(value: u64) -> Result<Self, StorageError> {
        if value == 0 {
            Err(document_error(StorageProblem::InvalidRevision))
        } else {
            Ok(Self(value))
        }
    }

    fn next(self) -> Result<Self, StorageError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(StorageError::RevisionExhausted)
    }
}

/// Immutable settings value and the revision required for its next mutation.
#[derive(Clone, Eq, PartialEq)]
pub struct SettingsSnapshot {
    revision: SettingsRevision,
    settings: DesktopSettings,
}

impl SettingsSnapshot {
    #[must_use]
    pub const fn revision(&self) -> SettingsRevision {
        self.revision
    }

    #[must_use]
    pub const fn settings(&self) -> &DesktopSettings {
        &self.settings
    }
}

impl std::fmt::Debug for SettingsSnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SettingsSnapshot")
            .field("revision", &self.revision)
            .field("settings", &self.settings)
            .finish()
    }
}

/// Immutable installation identity and its on-disk revision.
#[derive(Clone, Eq, PartialEq)]
pub struct IdentitySnapshot {
    revision: IdentityRevision,
    identity: InstallationIdentity,
}

impl IdentitySnapshot {
    #[must_use]
    pub const fn revision(&self) -> IdentityRevision {
        self.revision
    }

    #[must_use]
    pub const fn identity(&self) -> &InstallationIdentity {
        &self.identity
    }
}

impl std::fmt::Debug for IdentitySnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IdentitySnapshot")
            .field("revision", &self.revision)
            .field("identity", &self.identity)
            .finish()
    }
}

/// Result of loading under the lifetime process lock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadStatus {
    CreatedDefaults,
    Loaded,
    Migrated { from: u32 },
    RecoveryRequired,
}

/// Result of loading the distinct installation identity document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityLoadStatus {
    Created,
    Loaded,
    ResetRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommitStage {
    CreateTemporary,
    WriteTemporary,
    SyncTemporary,
    ValidateTemporary,
    Replace,
    SyncDirectory,
}

trait CommitHook: Send + Sync {
    fn check(&self, _stage: CommitStage) -> io::Result<()> {
        Ok(())
    }
}

struct NoopCommitHook;

impl CommitHook for NoopCommitHook {}

trait EntropySource: Send + Sync {
    fn fill(&self, destination: &mut [u8]) -> Result<(), StorageError>;
}

struct OsEntropy;

impl EntropySource for OsEntropy {
    fn fill(&self, destination: &mut [u8]) -> Result<(), StorageError> {
        getrandom::fill(destination).map_err(|_| StorageError::EntropyUnavailable)
    }
}

enum StoreState {
    Ready(SettingsSnapshot),
    RecoveryRequired,
}

enum IdentityStoreState {
    Ready(IdentitySnapshot),
    ResetRequired,
}

/// Lifetime-locked, serialized owner of `settings.json`.
pub struct SettingsStore {
    root: PathBuf,
    state: StoreState,
    identity_state: IdentityStoreState,
    identity_status: IdentityLoadStatus,
    devices: DeviceStoreState,
    device_revocation: Option<RevocationRequest>,
    reload_required: bool,
    commit_hook: Arc<dyn CommitHook>,
    entropy: Arc<dyn EntropySource>,
    #[cfg(windows)]
    _root_guards: windows_storage::RootGuards,
    // Declared last so the process lock is released after all other state.
    _process_lock: File,
}

impl SettingsStore {
    /// Opens the injected application-data root, acquires its non-blocking
    /// lifetime lock, cleans abandoned settings temporaries, and loads or
    /// creates the current schema.
    ///
    /// # Errors
    ///
    /// Fails closed for unsafe entries, an owned lock, I/O failures, and an
    /// unsupported newer schema. Corrupt current/older settings are quarantined
    /// and returned as [`LoadStatus::RecoveryRequired`].
    pub fn open(root: &Path) -> Result<(Self, LoadStatus), StorageError> {
        Self::open_with_entropy(root, Arc::new(OsEntropy))
    }

    fn open_with_entropy(
        root: &Path,
        entropy: Arc<dyn EntropySource>,
    ) -> Result<(Self, LoadStatus), StorageError> {
        #[cfg(windows)]
        let (root, root_guards) = windows_storage::prepare_root(root)?;
        #[cfg(not(windows))]
        let root = {
            prepare_root(root)?;
            fs::canonicalize(root).map_err(|error| {
                io_error(
                    StorageFileKind::Root,
                    StorageIoOperation::InspectEntry,
                    error,
                )
            })?
        };
        let process_lock = acquire_process_lock(&root)?;
        cleanup_abandoned_temporaries(
            &root,
            SETTINGS_TEMP_PREFIX,
            StorageFileKind::TemporarySettings,
        )?;
        cleanup_abandoned_temporaries(
            &root,
            IDENTITY_TEMP_PREFIX,
            StorageFileKind::TemporaryIdentity,
        )?;
        devices::cleanup_device_temporaries(&root)?;
        let mut store = Self {
            root,
            state: StoreState::RecoveryRequired,
            identity_state: IdentityStoreState::ResetRequired,
            identity_status: IdentityLoadStatus::ResetRequired,
            devices: DeviceStoreState::Unbound,
            device_revocation: None,
            reload_required: false,
            commit_hook: Arc::new(NoopCommitHook),
            entropy,
            #[cfg(windows)]
            _root_guards: root_guards,
            _process_lock: process_lock,
        };
        let allow_identity_creation = !entry_exists_no_follow(
            &store.root.join(SETTINGS_FILE_NAME),
            StorageFileKind::Settings,
            StorageIoOperation::InspectEntry,
        )? && !entry_exists_no_follow(
            &store.root.join(IDENTITY_FILE_NAME),
            StorageFileKind::Identity,
            StorageIoOperation::InspectEntry,
        )?;
        if allow_identity_creation {
            store.identity_status = store.load_or_initialize_identity(true)?;
        }
        let status = store.load_or_initialize()?;
        if !allow_identity_creation {
            store.identity_status = store.load_or_initialize_identity(false)?;
        }
        store.load_devices()?;
        Ok((store, status))
    }

    /// Returns the current immutable snapshot, or `None` until corrupt settings
    /// recovery is explicitly confirmed.
    #[must_use]
    pub const fn snapshot(&self) -> Option<&SettingsSnapshot> {
        match &self.state {
            StoreState::Ready(snapshot) => Some(snapshot),
            StoreState::RecoveryRequired => None,
        }
    }

    /// Returns the current installation identity, or `None` while an explicit
    /// identity reset is required.
    #[must_use]
    pub const fn identity_snapshot(&self) -> Option<&IdentitySnapshot> {
        match &self.identity_state {
            IdentityStoreState::Ready(snapshot) => Some(snapshot),
            IdentityStoreState::ResetRequired => None,
        }
    }

    /// Returns how the identity document was established during the latest
    /// open or reload.
    #[must_use]
    pub const fn identity_load_status(&self) -> IdentityLoadStatus {
        self.identity_status
    }

    /// Commits a fully validated replacement when `expected_revision` is still
    /// current.
    ///
    /// # Errors
    ///
    /// Returns a conflict without writing for a stale revision. A directory
    /// sync failure after replacement is reported as indeterminate and requires
    /// [`Self::reload`] before any later mutation.
    pub fn commit(
        &mut self,
        expected_revision: SettingsRevision,
        settings: DesktopSettings,
    ) -> Result<SettingsSnapshot, StorageError> {
        if self.reload_required {
            return Err(StorageError::ReloadRequired);
        }
        let current = match &self.state {
            StoreState::Ready(snapshot) => snapshot,
            StoreState::RecoveryRequired => {
                return Err(StorageError::RecoveryConfirmationRequired);
            }
        };
        if current.revision != expected_revision {
            return Err(StorageError::RevisionConflict {
                expected: expected_revision,
                actual: current.revision,
            });
        }
        let next = SettingsSnapshot {
            revision: current.revision.next()?,
            settings,
        };
        self.commit_snapshot(&next)?;
        self.state = StoreState::Ready(next.clone());
        Ok(next)
    }

    /// Writes safe settings after the desktop user confirms recovery from a
    /// quarantined corrupt file.
    ///
    /// # Errors
    ///
    /// Fails if recovery is not pending or if atomic persistence fails.
    pub fn confirm_recovery(
        &mut self,
        settings: DesktopSettings,
    ) -> Result<SettingsSnapshot, StorageError> {
        if self.reload_required {
            return Err(StorageError::ReloadRequired);
        }
        if !matches!(self.state, StoreState::RecoveryRequired) {
            let actual = self
                .snapshot()
                .map_or(SettingsRevision::INITIAL, SettingsSnapshot::revision);
            return Err(StorageError::RevisionConflict {
                expected: SettingsRevision::INITIAL,
                actual,
            });
        }
        let snapshot = SettingsSnapshot {
            revision: SettingsRevision::INITIAL,
            settings,
        };
        self.commit_snapshot(&snapshot)?;
        self.state = StoreState::Ready(snapshot.clone());
        Ok(snapshot)
    }

    /// Generates and commits a fresh installation identity only after corrupt
    /// identity state was quarantined and the desktop user explicitly confirms
    /// reset.
    ///
    /// The reset remains blocked while any paired-device document exists so a
    /// future caller cannot rotate identity before invalidating credentials.
    ///
    /// # Errors
    ///
    /// Fails unless reset is pending, paired-device state is absent, OS entropy
    /// succeeds, and the atomic commit completes.
    pub fn confirm_identity_reset(&mut self) -> Result<IdentitySnapshot, StorageError> {
        if self.reload_required {
            return Err(StorageError::ReloadRequired);
        }
        if !matches!(self.identity_state, IdentityStoreState::ResetRequired) {
            return Err(StorageError::IdentityResetNotRequired);
        }
        self.ensure_paired_devices_absent()?;
        let snapshot = self.generate_identity(IdentityRevision::INITIAL)?;
        self.commit_identity_snapshot(&snapshot)?;
        self.identity_state = IdentityStoreState::Ready(snapshot.clone());
        self.identity_status = IdentityLoadStatus::Created;
        self.reset_device_state_for_identity();
        Ok(snapshot)
    }

    /// Explicitly rotates a healthy identity after callers have stopped
    /// services and invalidated paired-device state.
    ///
    /// # Errors
    ///
    /// Rejects stale revisions, pending recovery, existing paired-device state,
    /// unavailable OS entropy, and atomic persistence failures.
    pub fn rotate_identity(
        &mut self,
        expected_revision: IdentityRevision,
    ) -> Result<IdentitySnapshot, StorageError> {
        if self.reload_required {
            return Err(StorageError::ReloadRequired);
        }
        let current = match &self.identity_state {
            IdentityStoreState::Ready(snapshot) => snapshot,
            IdentityStoreState::ResetRequired => return Err(StorageError::IdentityResetRequired),
        };
        if current.revision != expected_revision {
            return Err(StorageError::IdentityRevisionConflict {
                expected: expected_revision,
                actual: current.revision,
            });
        }
        let next_revision = current.revision.next()?;
        self.ensure_paired_devices_absent()?;
        let snapshot = self.generate_identity(next_revision)?;
        self.commit_identity_snapshot(&snapshot)?;
        self.identity_state = IdentityStoreState::Ready(snapshot.clone());
        self.identity_status = IdentityLoadStatus::Loaded;
        self.reset_device_state_for_identity();
        Ok(snapshot)
    }

    /// Reloads authoritative disk state after an indeterminate replacement.
    ///
    /// # Errors
    ///
    /// Returns the same safe load errors as [`Self::open`]. The process lock is
    /// retained throughout.
    pub fn reload(&mut self) -> Result<LoadStatus, StorageError> {
        self.reload_required = false;
        let result = (|| {
            let allow_identity_creation = !entry_exists_no_follow(
                &self.root.join(SETTINGS_FILE_NAME),
                StorageFileKind::Settings,
                StorageIoOperation::InspectEntry,
            )? && !entry_exists_no_follow(
                &self.root.join(IDENTITY_FILE_NAME),
                StorageFileKind::Identity,
                StorageIoOperation::InspectEntry,
            )?;
            if allow_identity_creation {
                self.identity_status = self.load_or_initialize_identity(true)?;
            }
            let status = self.load_or_initialize()?;
            if !allow_identity_creation {
                self.identity_status = self.load_or_initialize_identity(false)?;
            }
            self.load_devices()?;
            Ok(status)
        })();
        match result {
            Ok(status) => Ok(status),
            Err(error) => {
                self.reload_required = true;
                Err(error)
            }
        }
    }

    fn load_or_initialize(&mut self) -> Result<LoadStatus, StorageError> {
        let path = self.root.join(SETTINGS_FILE_NAME);
        if !entry_exists_no_follow(&path, StorageFileKind::Settings, StorageIoOperation::Read)? {
            let snapshot = SettingsSnapshot {
                revision: SettingsRevision::INITIAL,
                settings: DesktopSettings::default(),
            };
            self.commit_snapshot(&snapshot)?;
            self.state = StoreState::Ready(snapshot);
            return Ok(LoadStatus::CreatedDefaults);
        }

        match read_and_decode(&path) {
            Ok(DecodedSettings::Current(snapshot)) => {
                self.state = StoreState::Ready(snapshot);
                Ok(LoadStatus::Loaded)
            }
            Ok(DecodedSettings::Migrated {
                from,
                revision,
                settings,
            }) => {
                let snapshot = SettingsSnapshot {
                    revision: revision.next()?,
                    settings,
                };
                self.commit_snapshot(&snapshot)?;
                self.state = StoreState::Ready(snapshot);
                Ok(LoadStatus::Migrated { from })
            }
            Err(error) if error.is_corrupt_document() => {
                quarantine_document(&self.root, SETTINGS_DOCUMENT)?;
                self.state = StoreState::RecoveryRequired;
                Ok(LoadStatus::RecoveryRequired)
            }
            Err(error) => Err(error),
        }
    }

    fn load_or_initialize_identity(
        &mut self,
        settings_were_created: bool,
    ) -> Result<IdentityLoadStatus, StorageError> {
        let path = self.root.join(IDENTITY_FILE_NAME);
        if !entry_exists_no_follow(&path, StorageFileKind::Identity, StorageIoOperation::Read)? {
            if !settings_were_created || has_identity_recovery_evidence(&self.root)? {
                self.identity_state = IdentityStoreState::ResetRequired;
                return Ok(IdentityLoadStatus::ResetRequired);
            }
            self.ensure_paired_devices_absent()?;
            let snapshot = self.generate_identity(IdentityRevision::INITIAL)?;
            self.commit_identity_snapshot(&snapshot)?;
            self.identity_state = IdentityStoreState::Ready(snapshot);
            return Ok(IdentityLoadStatus::Created);
        }

        match read_and_decode_identity(&path) {
            Ok(snapshot) => {
                self.identity_state = IdentityStoreState::Ready(snapshot);
                Ok(IdentityLoadStatus::Loaded)
            }
            Err(error) if error.is_corrupt_document() => {
                quarantine_document(&self.root, IDENTITY_DOCUMENT)?;
                self.identity_state = IdentityStoreState::ResetRequired;
                Ok(IdentityLoadStatus::ResetRequired)
            }
            Err(error) => Err(error),
        }
    }

    fn generate_identity(
        &self,
        revision: IdentityRevision,
    ) -> Result<IdentitySnapshot, StorageError> {
        let mut installation_id = [0_u8; INSTALLATION_ID_BYTES];
        let mut hostname_seed = [0_u8; HOSTNAME_SEED_BYTES];
        self.entropy.fill(&mut installation_id)?;
        self.entropy.fill(&mut hostname_seed)?;
        Ok(IdentitySnapshot {
            revision,
            identity: InstallationIdentity::from_parts(installation_id, hostname_seed),
        })
    }

    fn ensure_paired_devices_absent(&self) -> Result<(), StorageError> {
        match fs::symlink_metadata(self.root.join(PAIRED_DEVICES_FILE_NAME)) {
            Ok(_) => Err(StorageError::IdentityResetBlockedByPairedDevices),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(io_error(
                StorageFileKind::Identity,
                StorageIoOperation::InspectEntry,
                error,
            )),
        }
    }

    fn commit_snapshot(&mut self, snapshot: &SettingsSnapshot) -> Result<(), StorageError> {
        let bytes = encode_snapshot(snapshot)?;
        self.commit_document(SETTINGS_DOCUMENT, &bytes, |path| {
            match read_and_decode(path)? {
                DecodedSettings::Current(candidate) if candidate == *snapshot => Ok(()),
                _ => Err(document_error(StorageProblem::InvalidEnvelope)),
            }
        })
    }

    fn commit_identity_snapshot(
        &mut self,
        snapshot: &IdentitySnapshot,
    ) -> Result<(), StorageError> {
        let bytes = encode_identity_snapshot(snapshot)?;
        self.commit_document(IDENTITY_DOCUMENT, &bytes, |path| {
            if read_and_decode_identity(path)? == *snapshot {
                Ok(())
            } else {
                Err(document_error(StorageProblem::InvalidEnvelope))
            }
        })
    }

    fn commit_document(
        &mut self,
        document: StoreDocument,
        bytes: &[u8],
        validate: impl FnOnce(&Path) -> Result<(), StorageError>,
    ) -> Result<(), StorageError> {
        let destination = self.root.join(document.name);
        if entry_exists_no_follow(
            &destination,
            document.kind,
            StorageIoOperation::InspectEntry,
        )? {
            validate_regular_file(&destination, document.kind)?;
        }

        let temporary_io = |operation, error| io_error(document.temporary_kind, operation, error);
        self.commit_hook
            .check(CommitStage::CreateTemporary)
            .map_err(|error| temporary_io(StorageIoOperation::CreateTemporary, error))?;
        let (staged_file, path) = create_temporary(
            &self.root,
            document.temporary_prefix,
            document.temporary_kind,
        )?;
        let mut cleanup = TemporaryCleanup::new(path.clone());

        {
            // Close the file before cleanup on every failure. Windows denies
            // deletion while this handle is open.
            let mut temporary = staged_file;
            self.commit_hook
                .check(CommitStage::WriteTemporary)
                .and_then(|()| temporary.write_all(bytes))
                .and_then(|()| temporary.flush())
                .map_err(|error| temporary_io(StorageIoOperation::WriteTemporary, error))?;
            self.commit_hook
                .check(CommitStage::SyncTemporary)
                .and_then(|()| temporary.sync_all())
                .map_err(|error| temporary_io(StorageIoOperation::SyncTemporary, error))?;
        }

        self.commit_hook
            .check(CommitStage::ValidateTemporary)
            .map_err(|error| temporary_io(StorageIoOperation::ValidateTemporary, error))?;
        validate(&path)?;
        self.commit_hook
            .check(CommitStage::Replace)
            .and_then(|()| move_storage_file(&path, &destination, true))
            .map_err(|error| io_error(document.kind, StorageIoOperation::Replace, error))?;
        cleanup.disarm();

        if self
            .commit_hook
            .check(CommitStage::SyncDirectory)
            .and_then(|()| finish_directory_commit(&self.root))
            .is_err()
        {
            self.reload_required = true;
            return Err(StorageError::IndeterminateCommit);
        }
        Ok(())
    }

    #[cfg(test)]
    fn set_commit_hook(&mut self, hook: Arc<dyn CommitHook>) {
        self.commit_hook = hook;
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvelopeHeader {
    format: String,
    schema_version: u32,
    revision: u64,
    #[serde(rename = "payload")]
    _payload: IgnoredAny,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsEnvelopeV1 {
    format: String,
    schema_version: u32,
    revision: u64,
    payload: SettingsPayloadV1,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SettingsEnvelopeV2 {
    format: String,
    schema_version: u32,
    revision: u64,
    payload: SettingsPayloadV2,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct IdentityPayloadV1 {
    installation_id: String,
    hostname_seed: String,
}

impl From<&InstallationIdentity> for IdentityPayloadV1 {
    fn from(identity: &InstallationIdentity) -> Self {
        Self {
            installation_id: identity.encoded_installation_id(),
            hostname_seed: identity.encoded_hostname_seed(),
        }
    }
}

impl TryFrom<IdentityPayloadV1> for InstallationIdentity {
    type Error = IdentityValidationError;

    fn try_from(payload: IdentityPayloadV1) -> Result<Self, Self::Error> {
        Self::from_encoded(&payload.installation_id, &payload.hostname_seed)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct IdentityEnvelopeV1 {
    format: String,
    schema_version: u32,
    revision: u64,
    payload: IdentityPayloadV1,
}

enum DecodedSettings {
    Current(SettingsSnapshot),
    Migrated {
        from: u32,
        revision: SettingsRevision,
        settings: DesktopSettings,
    },
}

fn encode_snapshot(snapshot: &SettingsSnapshot) -> Result<Vec<u8>, StorageError> {
    let envelope = SettingsEnvelopeV2 {
        format: SETTINGS_FORMAT.to_owned(),
        schema_version: CURRENT_SETTINGS_SCHEMA_VERSION,
        revision: snapshot.revision.get(),
        payload: SettingsPayloadV2::from(&snapshot.settings),
    };
    let bytes = serde_json::to_vec(&envelope)
        .map_err(|_| document_error(StorageProblem::InvalidEnvelope))?;
    if bytes.len() > SETTINGS_FILE_LIMIT {
        Err(document_error(StorageProblem::EncodedLimitExceeded))
    } else {
        Ok(bytes)
    }
}

fn encode_identity_snapshot(snapshot: &IdentitySnapshot) -> Result<Vec<u8>, StorageError> {
    let envelope = IdentityEnvelopeV1 {
        format: IDENTITY_FORMAT.to_owned(),
        schema_version: CURRENT_IDENTITY_SCHEMA_VERSION,
        revision: snapshot.revision.get(),
        payload: IdentityPayloadV1::from(&snapshot.identity),
    };
    let bytes = serde_json::to_vec(&envelope)
        .map_err(|_| document_error(StorageProblem::InvalidEnvelope))?;
    if bytes.len() > IDENTITY_FILE_LIMIT {
        Err(document_error(StorageProblem::EncodedLimitExceeded))
    } else {
        Ok(bytes)
    }
}

fn read_and_decode(path: &Path) -> Result<DecodedSettings, StorageError> {
    let bytes = read_bounded(path, StorageFileKind::Settings, SETTINGS_FILE_LIMIT)?;
    decode_settings(&bytes)
}

fn read_and_decode_identity(path: &Path) -> Result<IdentitySnapshot, StorageError> {
    let bytes = read_bounded(path, StorageFileKind::Identity, IDENTITY_FILE_LIMIT)?;
    decode_identity(&bytes)
}

fn decode_settings(bytes: &[u8]) -> Result<DecodedSettings, StorageError> {
    if bytes.len() > SETTINGS_FILE_LIMIT {
        return Err(document_error(StorageProblem::EncodedLimitExceeded));
    }
    let header: EnvelopeHeader = parse_json(bytes)?;
    if header.format != SETTINGS_FORMAT {
        return Err(document_error(StorageProblem::InvalidFormat));
    }
    let revision = SettingsRevision::from_stored(header.revision)?;
    match header.schema_version {
        1 => {
            let envelope: SettingsEnvelopeV1 = parse_json(bytes)?;
            if envelope.format != SETTINGS_FORMAT
                || envelope.schema_version != 1
                || envelope.revision != revision.get()
            {
                return Err(document_error(StorageProblem::InvalidEnvelope));
            }
            let settings =
                DesktopSettings::try_from(envelope.payload).map_err(settings_validation_error)?;
            Ok(DecodedSettings::Migrated {
                from: 1,
                revision,
                settings,
            })
        }
        CURRENT_SETTINGS_SCHEMA_VERSION => {
            let envelope: SettingsEnvelopeV2 = parse_json(bytes)?;
            if envelope.format != SETTINGS_FORMAT
                || envelope.schema_version != CURRENT_SETTINGS_SCHEMA_VERSION
                || envelope.revision != revision.get()
            {
                return Err(document_error(StorageProblem::InvalidEnvelope));
            }
            let settings =
                DesktopSettings::try_from(envelope.payload).map_err(settings_validation_error)?;
            Ok(DecodedSettings::Current(SettingsSnapshot {
                revision,
                settings,
            }))
        }
        found => Err(StorageError::UnsupportedSchema {
            found,
            current: CURRENT_SETTINGS_SCHEMA_VERSION,
        }),
    }
}

fn decode_identity(bytes: &[u8]) -> Result<IdentitySnapshot, StorageError> {
    if bytes.len() > IDENTITY_FILE_LIMIT {
        return Err(document_error(StorageProblem::EncodedLimitExceeded));
    }
    let header: EnvelopeHeader = parse_json(bytes)?;
    if header.format != IDENTITY_FORMAT {
        return Err(document_error(StorageProblem::InvalidFormat));
    }
    let revision = IdentityRevision::from_stored(header.revision)?;
    match header.schema_version {
        0 => Err(document_error(StorageProblem::InvalidEnvelope)),
        CURRENT_IDENTITY_SCHEMA_VERSION => {
            let envelope: IdentityEnvelopeV1 = parse_json(bytes)?;
            if envelope.format != IDENTITY_FORMAT
                || envelope.schema_version != CURRENT_IDENTITY_SCHEMA_VERSION
                || envelope.revision != revision.get()
            {
                return Err(document_error(StorageProblem::InvalidEnvelope));
            }
            let identity = InstallationIdentity::try_from(envelope.payload)
                .map_err(identity_validation_error)?;
            Ok(IdentitySnapshot { revision, identity })
        }
        found => Err(StorageError::UnsupportedIdentitySchema {
            found,
            current: CURRENT_IDENTITY_SCHEMA_VERSION,
        }),
    }
}

fn parse_json<'de, T: Deserialize<'de>>(bytes: &'de [u8]) -> Result<T, StorageError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = T::deserialize(&mut deserializer).map_err(json_error)?;
    deserializer.end().map_err(json_error)?;
    Ok(value)
}

fn json_error(error: serde_json::Error) -> StorageError {
    let result = StorageError::Document {
        problem: StorageProblem::MalformedJson,
        line: u64::try_from(error.line()).ok(),
        column: u64::try_from(error.column()).ok(),
    };
    drop(error);
    result
}

fn settings_validation_error(_error: SettingsValidationError) -> StorageError {
    document_error(StorageProblem::InvalidSettings)
}

fn identity_validation_error(_error: IdentityValidationError) -> StorageError {
    document_error(StorageProblem::InvalidIdentity)
}

const fn document_error(problem: StorageProblem) -> StorageError {
    StorageError::Document {
        problem,
        line: None,
        column: None,
    }
}

fn io_error(
    file: StorageFileKind,
    operation: StorageIoOperation,
    error: io::Error,
) -> StorageError {
    let kind = error.kind();
    drop(error);
    StorageError::Io {
        file,
        operation,
        kind,
    }
}

#[cfg(not(windows))]
fn prepare_root(root: &Path) -> Result<(), StorageError> {
    if !root.is_absolute() {
        return Err(document_error(StorageProblem::UnsafeEntryKind));
    }
    match fs::symlink_metadata(root) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(document_error(StorageProblem::UnsafeEntryKind));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            create_private_directory(root, StorageFileKind::Root)?;
        }
        Err(error) => {
            return Err(io_error(
                StorageFileKind::Root,
                StorageIoOperation::InspectEntry,
                error,
            ));
        }
    }
    tighten_directory_permissions(root, StorageFileKind::Root)?;
    let quarantine = root.join(QUARANTINE_DIRECTORY_NAME);
    match fs::symlink_metadata(&quarantine) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(document_error(StorageProblem::UnsafeEntryKind));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            create_private_directory(&quarantine, StorageFileKind::Quarantine)?;
        }
        Err(error) => {
            return Err(io_error(
                StorageFileKind::Quarantine,
                StorageIoOperation::InspectEntry,
                error,
            ));
        }
    }
    tighten_directory_permissions(&quarantine, StorageFileKind::Quarantine)
}

fn acquire_process_lock(root: &Path) -> Result<File, StorageError> {
    let path = root.join(LOCK_FILE_NAME);
    #[cfg(windows)]
    let file = windows_storage::open_lock(&path)?;
    #[cfg(not(windows))]
    let file = open_lock_file(&path)?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(StorageError::LockHeld),
        Err(TryLockError::Error(error)) => Err(io_error(
            StorageFileKind::ProcessLock,
            StorageIoOperation::AcquireLock,
            error,
        )),
    }
}

fn read_bounded(
    path: &Path,
    kind: StorageFileKind,
    encoded_limit: usize,
) -> Result<Vec<u8>, StorageError> {
    let file =
        open_read_file(path).map_err(|error| io_error(kind, StorageIoOperation::Read, error))?;
    validate_opened_file(path, &file, kind)?;
    tighten_open_file_permissions(&file, kind)?;
    validate_opened_file(path, &file, kind)?;
    let metadata = file
        .metadata()
        .map_err(|error| io_error(kind, StorageIoOperation::Read, error))?;
    if metadata.len() > encoded_limit as u64 {
        return Err(document_error(StorageProblem::EncodedLimitExceeded));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(encoded_limit));
    file.take((encoded_limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| io_error(kind, StorageIoOperation::Read, error))?;
    if bytes.len() > encoded_limit {
        Err(document_error(StorageProblem::EncodedLimitExceeded))
    } else {
        Ok(bytes)
    }
}

fn create_temporary(
    root: &Path,
    prefix: &str,
    kind: StorageFileKind,
) -> Result<(File, PathBuf), StorageError> {
    for _ in 0..MAX_TEMP_ATTEMPTS {
        let path = root.join(temporary_name(prefix));
        match create_private_file(&path) {
            Ok(created) => {
                let mut cleanup = TemporaryCleanup::new(path.clone());
                let file = created;
                validate_opened_file(&path, &file, kind)?;
                tighten_open_file_permissions(&file, kind)?;
                validate_opened_file(&path, &file, kind)?;
                cleanup.disarm();
                return Ok((file, path));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(io_error(kind, StorageIoOperation::CreateTemporary, error));
            }
        }
    }
    Err(StorageError::Io {
        file: kind,
        operation: StorageIoOperation::CreateTemporary,
        kind: io::ErrorKind::AlreadyExists,
    })
}

fn temporary_name(prefix: &str) -> String {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(sequence);
    hasher.write_u32(std::process::id());
    hasher.write_u128(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos()),
    );
    format!("{prefix}{:016x}-{sequence:016x}", hasher.finish())
}

fn cleanup_abandoned_temporaries(
    root: &Path,
    prefix: &str,
    kind: StorageFileKind,
) -> Result<(), StorageError> {
    let entries = fs::read_dir(root).map_err(|error| {
        io_error(
            StorageFileKind::Root,
            StorageIoOperation::CleanupTemporary,
            error,
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            io_error(
                StorageFileKind::Root,
                StorageIoOperation::CleanupTemporary,
                error,
            )
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !is_exact_temporary_name(name, prefix) {
            continue;
        }
        let path = entry.path();
        validate_regular_file(&path, kind)?;
        fs::remove_file(path)
            .map_err(|error| io_error(kind, StorageIoOperation::CleanupTemporary, error))?;
    }
    Ok(())
}

fn is_exact_temporary_name(name: &str, prefix: &str) -> bool {
    let Some(suffix) = name.strip_prefix(prefix) else {
        return false;
    };
    let mut parts = suffix.split('-');
    let valid_hex =
        |part: &str| part.len() == 16 && part.bytes().all(|byte| byte.is_ascii_hexdigit());
    matches!((parts.next(), parts.next(), parts.next()), (Some(first), Some(second), None) if valid_hex(first) && valid_hex(second))
}

fn quarantine_document(root: &Path, document: StoreDocument) -> Result<(), StorageError> {
    let source = root.join(document.name);
    validate_regular_file(&source, document.kind)?;
    let quarantine = root.join(QUARANTINE_DIRECTORY_NAME);
    for _ in 0..MAX_TEMP_ATTEMPTS {
        let generated = temporary_name(document.temporary_prefix);
        let suffix = &generated[document.temporary_prefix.len()..];
        let target = quarantine.join(format!("{}{suffix}.json", document.quarantine_prefix));
        if entry_exists_no_follow(
            &target,
            StorageFileKind::Quarantine,
            StorageIoOperation::Quarantine,
        )? {
            continue;
        }
        match move_storage_file(&source, &target, false) {
            Ok(()) => {
                let quarantined = open_read_file(&target).map_err(|error| {
                    io_error(
                        StorageFileKind::Quarantine,
                        StorageIoOperation::TightenPermissions,
                        error,
                    )
                })?;
                validate_opened_file(&target, &quarantined, StorageFileKind::Quarantine)?;
                tighten_open_file_permissions(&quarantined, StorageFileKind::Quarantine)?;
                validate_opened_file(&target, &quarantined, StorageFileKind::Quarantine)?;
                finish_directory_commit(&quarantine).map_err(|error| {
                    io_error(
                        StorageFileKind::Quarantine,
                        StorageIoOperation::SyncDirectory,
                        error,
                    )
                })?;
                finish_directory_commit(root).map_err(|error| {
                    io_error(
                        StorageFileKind::Root,
                        StorageIoOperation::SyncDirectory,
                        error,
                    )
                })?;
                return Ok(());
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(io_error(
                    document.kind,
                    StorageIoOperation::Quarantine,
                    error,
                ));
            }
        }
    }
    Err(StorageError::Io {
        file: StorageFileKind::Quarantine,
        operation: StorageIoOperation::Quarantine,
        kind: io::ErrorKind::AlreadyExists,
    })
}

fn has_identity_recovery_evidence(root: &Path) -> Result<bool, StorageError> {
    has_quarantine_evidence(root, IDENTITY_DOCUMENT.quarantine_prefix)
}

fn has_quarantine_evidence(root: &Path, prefix: &str) -> Result<bool, StorageError> {
    const MAX_ENTRIES: usize = 4_096;
    let quarantine = root.join(QUARANTINE_DIRECTORY_NAME);
    let entries = fs::read_dir(quarantine).map_err(|error| {
        io_error(
            StorageFileKind::Quarantine,
            StorageIoOperation::InspectEntry,
            error,
        )
    })?;
    for (index, entry) in entries.enumerate() {
        if index >= MAX_ENTRIES {
            return Err(io_error(
                StorageFileKind::Quarantine,
                StorageIoOperation::InspectEntry,
                io::Error::from(io::ErrorKind::FileTooLarge),
            ));
        }
        let entry = entry.map_err(|error| {
            io_error(
                StorageFileKind::Quarantine,
                StorageIoOperation::InspectEntry,
                error,
            )
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if is_quarantine_name(name, prefix) {
            validate_regular_file(&entry.path(), StorageFileKind::Quarantine)?;
            return Ok(true);
        }
    }
    Ok(false)
}

fn is_quarantine_name(name: &str, prefix: &str) -> bool {
    let Some(suffix) = name
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(".json"))
    else {
        return false;
    };
    let mut parts = suffix.split('-');
    let valid_hex =
        |part: &str| part.len() == 16 && part.bytes().all(|byte| byte.is_ascii_hexdigit());
    matches!((parts.next(), parts.next(), parts.next()), (Some(first), Some(second), None) if valid_hex(first) && valid_hex(second))
}

struct TemporaryCleanup {
    path: PathBuf,
    armed: bool,
}

impl TemporaryCleanup {
    const fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TemporaryCleanup {
    fn drop(&mut self) {
        if self.armed {
            let _ignored = fs::remove_file(&self.path);
        }
    }
}

fn entry_exists_no_follow(
    path: &Path,
    kind: StorageFileKind,
    operation: StorageIoOperation,
) -> Result<bool, StorageError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(kind, operation, error)),
    }
}

#[cfg(not(windows))]
fn validate_regular_file(path: &Path, kind: StorageFileKind) -> Result<(), StorageError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| io_error(kind, StorageIoOperation::InspectEntry, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(document_error(StorageProblem::UnsafeEntryKind));
    }
    validate_link_count(&metadata)
}

#[cfg(not(windows))]
fn validate_opened_file(
    path: &Path,
    file: &File,
    kind: StorageFileKind,
) -> Result<(), StorageError> {
    let path_metadata = fs::symlink_metadata(path)
        .map_err(|error| io_error(kind, StorageIoOperation::InspectEntry, error))?;
    let opened_metadata = file
        .metadata()
        .map_err(|error| io_error(kind, StorageIoOperation::InspectEntry, error))?;
    if path_metadata.file_type().is_symlink()
        || !path_metadata.is_file()
        || !opened_metadata.is_file()
        || !same_file(&path_metadata, &opened_metadata)
    {
        return Err(document_error(StorageProblem::UnsafeEntryKind));
    }
    validate_link_count(&opened_metadata)
}

#[cfg(unix)]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(any(unix, windows)))]
fn same_file(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    true
}

#[cfg(unix)]
fn validate_link_count(metadata: &fs::Metadata) -> Result<(), StorageError> {
    use std::os::unix::fs::MetadataExt;
    if metadata.nlink() == 1 {
        Ok(())
    } else {
        Err(document_error(StorageProblem::HardLinkDetected))
    }
}

#[cfg(not(any(unix, windows)))]
fn validate_link_count(_metadata: &fs::Metadata) -> Result<(), StorageError> {
    Err(StorageError::PlatformPermissionEnforcementUnavailable)
}

#[cfg(unix)]
fn create_private_directory(path: &Path, kind: StorageFileKind) -> Result<(), StorageError> {
    use std::os::unix::fs::DirBuilderExt;
    let mut builder = fs::DirBuilder::new();
    builder.mode(0o700);
    builder
        .create(path)
        .map_err(|error| io_error(kind, StorageIoOperation::CreateDirectory, error))
}

#[cfg(not(any(unix, windows)))]
fn create_private_directory(path: &Path, kind: StorageFileKind) -> Result<(), StorageError> {
    fs::create_dir(path).map_err(|error| io_error(kind, StorageIoOperation::CreateDirectory, error))
}

#[cfg(unix)]
fn configure_private_file_creation(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
}

#[cfg(not(any(unix, windows)))]
fn configure_private_file_creation(_options: &mut OpenOptions) {}

#[cfg(unix)]
fn tighten_directory_permissions(path: &Path, kind: StorageFileKind) -> Result<(), StorageError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| io_error(kind, StorageIoOperation::TightenPermissions, error))
}

#[cfg(not(any(unix, windows)))]
fn tighten_directory_permissions(_path: &Path, _kind: StorageFileKind) -> Result<(), StorageError> {
    Err(StorageError::PlatformPermissionEnforcementUnavailable)
}

#[cfg(unix)]
fn tighten_open_file_permissions(file: &File, kind: StorageFileKind) -> Result<(), StorageError> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|error| io_error(kind, StorageIoOperation::TightenPermissions, error))
}

#[cfg(not(any(unix, windows)))]
fn tighten_open_file_permissions(_file: &File, _kind: StorageFileKind) -> Result<(), StorageError> {
    Err(StorageError::PlatformPermissionEnforcementUnavailable)
}

#[cfg(unix)]
fn finish_directory_commit(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(not(any(unix, windows)))]
fn finish_directory_commit(_path: &Path) -> io::Result<()> {
    Err(io::Error::from(io::ErrorKind::Unsupported))
}

#[cfg(not(windows))]
fn open_lock_file(path: &Path) -> Result<File, StorageError> {
    let kind = StorageFileKind::ProcessLock;
    if entry_exists_no_follow(path, kind, StorageIoOperation::InspectEntry)? {
        validate_regular_file(path, kind)?;
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    configure_private_file_creation(&mut options);
    let file = options
        .open(path)
        .map_err(|error| io_error(kind, StorageIoOperation::OpenLock, error))?;
    validate_opened_file(path, &file, kind)?;
    tighten_open_file_permissions(&file, kind)?;
    validate_opened_file(path, &file, kind)?;
    Ok(file)
}

#[cfg(not(windows))]
fn create_private_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    configure_private_file_creation(&mut options);
    options.open(path)
}

#[cfg(not(windows))]
fn open_read_file(path: &Path) -> io::Result<File> {
    File::open(path)
}

#[cfg(not(windows))]
fn move_storage_file(from: &Path, to: &Path, _replace: bool) -> io::Result<()> {
    fs::rename(from, to)
}

#[cfg(windows)]
use windows_storage::{
    confirm_directory as finish_directory_commit, create_temporary as create_private_file,
    move_file as move_storage_file, open_read as open_read_file,
    validate_file as validate_regular_file, validate_opened as validate_opened_file,
};

#[cfg(windows)]
fn tighten_open_file_permissions(file: &File, _kind: StorageFileKind) -> Result<(), StorageError> {
    windows_storage::verify_file_security(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ApprovedInterfaceId, LogLevel};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tether_test_support::{PrivateTempDir, SeededRandom};

    struct SeededEntropy(Mutex<SeededRandom>);

    impl SeededEntropy {
        const fn new(seed: u64) -> Self {
            Self(Mutex::new(SeededRandom::new(seed)))
        }
    }

    impl EntropySource for SeededEntropy {
        fn fill(&self, destination: &mut [u8]) -> Result<(), StorageError> {
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .fill_bytes(destination);
            Ok(())
        }
    }

    struct FailingEntropy;

    impl EntropySource for FailingEntropy {
        fn fill(&self, _destination: &mut [u8]) -> Result<(), StorageError> {
            Err(StorageError::EntropyUnavailable)
        }
    }

    fn open_seeded(root: &Path, seed: u64) -> Result<(SettingsStore, LoadStatus), StorageError> {
        SettingsStore::open_with_entropy(root, Arc::new(SeededEntropy::new(seed)))
    }

    pub(super) fn write_fixture(
        path: impl AsRef<Path>,
        contents: impl AsRef<[u8]>,
    ) -> io::Result<()> {
        #[cfg(windows)]
        match create_private_file(path.as_ref()) {
            Ok(mut file) => return file.write_all(contents.as_ref()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        fs::write(path, contents)
    }

    fn changed_settings() -> DesktopSettings {
        DesktopSettings::new(
            Some(48_312),
            vec![ApprovedInterfaceId::new("interface-stable-id".to_owned()).expect("valid")],
            true,
            true,
            LogLevel::Debug,
        )
        .expect("valid settings")
    }

    #[test]
    fn first_run_identity_uses_injected_entropy_and_persists() {
        let temporary = PrivateTempDir::new().expect("temporary root");
        let root = temporary.path().join("config");
        let seed = 0x4944_454e_5449_5459;
        let (store, status) = open_seeded(&root, seed).expect("first open");
        assert_eq!(status, LoadStatus::CreatedDefaults);
        assert_eq!(store.identity_load_status(), IdentityLoadStatus::Created);
        let snapshot = store.identity_snapshot().expect("identity ready").clone();
        assert_eq!(snapshot.revision().get(), 1);

        let mut expected_random = SeededRandom::new(seed);
        let mut expected_id = [0_u8; INSTALLATION_ID_BYTES];
        let mut expected_seed = [0_u8; HOSTNAME_SEED_BYTES];
        expected_random.fill_bytes(&mut expected_id);
        expected_random.fill_bytes(&mut expected_seed);
        let expected = InstallationIdentity::from_parts(expected_id, expected_seed);
        assert_eq!(snapshot.identity(), &expected);
        assert_eq!(
            snapshot.identity().preferred_hostname(),
            expected.preferred_hostname()
        );
        let encoded = fs::read(root.join(IDENTITY_FILE_NAME)).expect("identity document");
        assert!(encoded.len() <= IDENTITY_FILE_LIMIT);
        let encoded = String::from_utf8(encoded).expect("identity JSON is UTF-8");
        assert!(encoded.contains("\"format\":\"tether.identity\""));
        assert!(!encoded.contains("tether.settings"));
        drop(store);

        let (reopened, status) = open_seeded(&root, seed.wrapping_add(1)).expect("restart");
        assert_eq!(status, LoadStatus::Loaded);
        assert_eq!(reopened.identity_load_status(), IdentityLoadStatus::Loaded);
        assert_eq!(reopened.identity_snapshot(), Some(&snapshot));
    }

    #[test]
    fn operating_system_entropy_failure_creates_no_identity_document() {
        let temporary = PrivateTempDir::new().expect("temporary root");
        let root = temporary.path().join("config");
        assert_eq!(
            SettingsStore::open_with_entropy(&root, Arc::new(FailingEntropy)).map(|_| ()),
            Err(StorageError::EntropyUnavailable)
        );
        assert!(!root.join(IDENTITY_FILE_NAME).exists());
        assert!(!root.join(SETTINGS_FILE_NAME).exists());
    }

    #[test]
    fn corrupt_identity_stays_fail_closed_across_restart_until_explicit_reset() {
        let temporary = PrivateTempDir::new().expect("temporary root");
        let root = temporary.path().join("config");
        let (store, _) = open_seeded(&root, 1).expect("first open");
        drop(store);
        write_fixture(root.join(IDENTITY_FILE_NAME), b"{not-json").expect("corrupt identity");

        let (store, status) = open_seeded(&root, 2).expect("quarantine corruption");
        assert_eq!(status, LoadStatus::Loaded);
        assert_eq!(
            store.identity_load_status(),
            IdentityLoadStatus::ResetRequired
        );
        assert!(store.identity_snapshot().is_none());
        assert!(!root.join(IDENTITY_FILE_NAME).exists());
        assert!(has_identity_recovery_evidence(&root).expect("recovery evidence"));
        drop(store);

        let (mut store, _) = open_seeded(&root, 3).expect("restart remains closed");
        assert_eq!(
            store.identity_load_status(),
            IdentityLoadStatus::ResetRequired
        );
        assert!(store.identity_snapshot().is_none());
        write_fixture(root.join(PAIRED_DEVICES_FILE_NAME), b"placeholder").expect("paired state");
        assert_eq!(
            store.confirm_identity_reset(),
            Err(StorageError::IdentityResetBlockedByPairedDevices)
        );
        fs::remove_file(root.join(PAIRED_DEVICES_FILE_NAME)).expect("invalidate paired state");
        let reset = store.confirm_identity_reset().expect("explicit reset");
        assert_eq!(reset.revision().get(), 1);
        drop(store);

        let (store, _) = open_seeded(&root, 4).expect("reset persisted");
        assert_eq!(store.identity_load_status(), IdentityLoadStatus::Loaded);
        assert_eq!(store.identity_snapshot(), Some(&reset));
    }

    #[test]
    fn invalid_and_oversized_identity_documents_require_reset() {
        let fixtures = [
            br#"{"format":"tether.settings","schema_version":1,"revision":1,"payload":{"installation_id":"00","hostname_seed":"00"}}"#.to_vec(),
            br#"{"format":"tether.identity","schema_version":0,"revision":1,"payload":{"installation_id":"00","hostname_seed":"00"}}"#.to_vec(),
            vec![b' '; IDENTITY_FILE_LIMIT + 1],
        ];
        for contents in fixtures {
            let temporary = PrivateTempDir::new().expect("temporary root");
            let root = temporary.path().join("config");
            let (store, _) = open_seeded(&root, 5).expect("first open");
            drop(store);
            write_fixture(root.join(IDENTITY_FILE_NAME), contents).expect("invalid fixture");
            let (store, _) = open_seeded(&root, 6).expect("fail-closed open");
            assert_eq!(
                store.identity_load_status(),
                IdentityLoadStatus::ResetRequired
            );
            assert!(store.identity_snapshot().is_none());
        }
    }

    #[test]
    fn future_identity_schema_is_untouched_and_refused() {
        let temporary = PrivateTempDir::new().expect("temporary root");
        let root = temporary.path().join("config");
        let (store, _) = open_seeded(&root, 7).expect("first open");
        drop(store);
        let future = br#"{"format":"tether.identity","schema_version":99,"revision":1,"payload":{"future":true}}"#;
        write_fixture(root.join(IDENTITY_FILE_NAME), future).expect("future fixture");
        assert!(matches!(
            open_seeded(&root, 8),
            Err(StorageError::UnsupportedIdentitySchema {
                found: 99,
                current: CURRENT_IDENTITY_SCHEMA_VERSION
            })
        ));
        assert_eq!(
            fs::read(root.join(IDENTITY_FILE_NAME)).expect("untouched"),
            future
        );
    }

    #[test]
    fn identity_rotation_checks_revision_and_paired_device_invalidation() {
        let temporary = PrivateTempDir::new().expect("temporary root");
        let root = temporary.path().join("config");
        let (mut store, _) = open_seeded(&root, 9).expect("first open");
        let original = store.identity_snapshot().expect("identity").clone();
        let rotated = store
            .rotate_identity(original.revision())
            .expect("explicit rotation");
        assert_eq!(rotated.revision().get(), 2);
        assert_ne!(rotated.identity(), original.identity());
        assert_eq!(
            store.rotate_identity(original.revision()),
            Err(StorageError::IdentityRevisionConflict {
                expected: original.revision(),
                actual: rotated.revision(),
            })
        );

        write_fixture(root.join(PAIRED_DEVICES_FILE_NAME), b"placeholder").expect("paired state");
        assert_eq!(
            store.rotate_identity(rotated.revision()),
            Err(StorageError::IdentityResetBlockedByPairedDevices)
        );
        assert_eq!(store.identity_snapshot(), Some(&rotated));
    }

    #[test]
    fn missing_identity_is_not_silently_recreated_for_an_existing_installation() {
        let temporary = PrivateTempDir::new().expect("temporary root");
        let root = temporary.path().join("config");
        let (store, _) = open_seeded(&root, 10).expect("first open");
        drop(store);
        fs::remove_file(root.join(IDENTITY_FILE_NAME)).expect("remove identity");

        let (store, status) = open_seeded(&root, 11).expect("fail-closed restart");
        assert_eq!(status, LoadStatus::Loaded);
        assert_eq!(
            store.identity_load_status(),
            IdentityLoadStatus::ResetRequired
        );
        assert!(store.identity_snapshot().is_none());
        assert!(!root.join(IDENTITY_FILE_NAME).exists());
    }

    #[test]
    fn clean_first_run_commits_defaults_and_restart_loads_them() {
        let temporary = PrivateTempDir::new().expect("temporary root");
        let root = temporary.path().join("config");
        let (store, status) = SettingsStore::open(&root).expect("first open");
        assert_eq!(status, LoadStatus::CreatedDefaults);
        let snapshot = store.snapshot().expect("ready");
        assert_eq!(snapshot.revision().get(), 1);
        assert_eq!(snapshot.settings(), &DesktopSettings::default());
        drop(store);

        let (store, status) = SettingsStore::open(&root).expect("restart");
        assert_eq!(status, LoadStatus::Loaded);
        assert_eq!(
            store.snapshot().expect("ready").settings(),
            &DesktopSettings::default()
        );
    }

    #[test]
    fn commit_is_revision_checked_and_persists() {
        let temporary = PrivateTempDir::new().expect("temporary root");
        let root = temporary.path().join("config");
        let (mut store, _) = SettingsStore::open(&root).expect("open");
        let revision = store.snapshot().expect("ready").revision();
        let committed = store.commit(revision, changed_settings()).expect("commit");
        assert_eq!(committed.revision().get(), 2);
        assert_eq!(
            store.commit(revision, DesktopSettings::default()),
            Err(StorageError::RevisionConflict {
                expected: revision,
                actual: committed.revision(),
            })
        );
        drop(store);

        let (store, status) = SettingsStore::open(&root).expect("restart");
        assert_eq!(status, LoadStatus::Loaded);
        assert_eq!(store.snapshot(), Some(&committed));
    }

    #[test]
    fn version_one_migrates_with_safe_defaults_and_commits_before_exposure() {
        let temporary = PrivateTempDir::new().expect("temporary root");
        let root = temporary.path().join("config");
        let (store, _) = SettingsStore::open(&root).expect("create root");
        drop(store);
        write_fixture(
            root.join(SETTINGS_FILE_NAME),
            br#"{"format":"tether.settings","schema_version":1,"revision":7,"payload":{"server_port":48312,"approved_interface_ids":["stable-id"],"auto_listen":true}}"#,
        )
        .expect("write fixture");

        let (store, status) = SettingsStore::open(&root).expect("migrate");
        assert_eq!(status, LoadStatus::Migrated { from: 1 });
        let snapshot = store.snapshot().expect("ready");
        assert_eq!(snapshot.revision().get(), 8);
        assert!(!snapshot.settings().start_at_login());
        assert_eq!(snapshot.settings().log_level(), LogLevel::Info);
        drop(store);

        let (_, status) = SettingsStore::open(&root).expect("current restart");
        assert_eq!(status, LoadStatus::Loaded);
    }

    #[test]
    fn corrupt_settings_are_quarantined_and_require_confirmation() {
        let temporary = PrivateTempDir::new().expect("temporary root");
        let root = temporary.path().join("config");
        let (store, _) = SettingsStore::open(&root).expect("create root");
        drop(store);
        write_fixture(root.join(SETTINGS_FILE_NAME), b"{not-json").expect("corrupt fixture");

        let (mut store, status) = SettingsStore::open(&root).expect("quarantine");
        assert_eq!(status, LoadStatus::RecoveryRequired);
        assert!(store.snapshot().is_none());
        assert_eq!(
            store.commit(SettingsRevision::INITIAL, DesktopSettings::default()),
            Err(StorageError::RecoveryConfirmationRequired)
        );
        let quarantined = fs::read_dir(root.join(QUARANTINE_DIRECTORY_NAME))
            .expect("quarantine readable")
            .count();
        assert_eq!(quarantined, 1);
        let recovered = store
            .confirm_recovery(DesktopSettings::default())
            .expect("confirmed recovery");
        assert_eq!(recovered.revision().get(), 1);
    }

    #[test]
    fn duplicate_keys_and_oversize_documents_fail_as_corruption() {
        for contents in [
            br#"{"format":"tether.settings","format":"tether.settings","schema_version":2,"revision":1,"payload":{"server_port":null,"approved_interface_ids":[],"auto_listen":false,"start_at_login":false,"log_level":"info"}}"#.to_vec(),
            vec![b' '; SETTINGS_FILE_LIMIT + 1],
        ] {
            let temporary = PrivateTempDir::new().expect("temporary root");
            let root = temporary.path().join("config");
            let (store, _) = SettingsStore::open(&root).expect("create root");
            drop(store);
            write_fixture(root.join(SETTINGS_FILE_NAME), contents).expect("fixture");
            let (store, status) = SettingsStore::open(&root).expect("recover corruption");
            assert_eq!(status, LoadStatus::RecoveryRequired);
            assert!(store.snapshot().is_none());
        }
    }

    #[test]
    fn invalid_payload_values_unknown_fields_and_trailing_data_are_quarantined() {
        let oversized_identity = "x".repeat(crate::MAX_INTERFACE_ID_BYTES + 1);
        let fixtures = [
            br#"{"format":"tether.settings","schema_version":2,"revision":1,"payload":{"server_port":0,"approved_interface_ids":[],"auto_listen":false,"start_at_login":false,"log_level":"info"}}"#.to_vec(),
            br#"{"format":"tether.settings","schema_version":2,"revision":1,"payload":{"server_port":null,"approved_interface_ids":[],"auto_listen":false,"start_at_login":false,"log_level":"info","misspelled":true}}"#.to_vec(),
            br#"{"format":"tether.settings","schema_version":2,"revision":1,"payload":{"server_port":null,"approved_interface_ids":[],"auto_listen":false,"start_at_login":false,"log_level":"info"}} trailing"#.to_vec(),
            format!(
                "{{\"format\":\"tether.settings\",\"schema_version\":2,\"revision\":1,\"payload\":{{\"server_port\":null,\"approved_interface_ids\":[\"{oversized_identity}\"],\"auto_listen\":false,\"start_at_login\":false,\"log_level\":\"info\"}}}}"
            )
            .into_bytes(),
        ];
        for contents in fixtures {
            let temporary = PrivateTempDir::new().expect("temporary root");
            let root = temporary.path().join("config");
            let (store, _) = SettingsStore::open(&root).expect("create root");
            drop(store);
            write_fixture(root.join(SETTINGS_FILE_NAME), contents).expect("fixture");
            let (store, status) = SettingsStore::open(&root).expect("recover corruption");
            assert_eq!(status, LoadStatus::RecoveryRequired);
            assert!(store.snapshot().is_none());
        }
    }

    #[test]
    fn newer_schema_is_left_untouched_and_refused() {
        let temporary = PrivateTempDir::new().expect("temporary root");
        let root = temporary.path().join("config");
        let (store, _) = SettingsStore::open(&root).expect("create root");
        drop(store);
        let future = br#"{"format":"tether.settings","schema_version":99,"revision":1,"payload":{"future":true}}"#;
        write_fixture(root.join(SETTINGS_FILE_NAME), future).expect("future fixture");
        assert!(matches!(
            SettingsStore::open(&root),
            Err(StorageError::UnsupportedSchema {
                found: 99,
                current: CURRENT_SETTINGS_SCHEMA_VERSION
            })
        ));
        assert_eq!(
            fs::read(root.join(SETTINGS_FILE_NAME)).expect("untouched"),
            future
        );
    }

    #[test]
    fn second_store_cannot_acquire_the_lifetime_lock() {
        let temporary = PrivateTempDir::new().expect("temporary root");
        let root = temporary.path().join("config");
        let (first, _) = SettingsStore::open(&root).expect("first lock");
        assert!(matches!(
            SettingsStore::open(&root),
            Err(StorageError::LockHeld)
        ));
        drop(first);
        assert!(SettingsStore::open(&root).is_ok());
    }

    #[test]
    fn abandoned_exact_temporary_is_removed_but_similar_name_is_not() {
        let temporary = PrivateTempDir::new().expect("temporary root");
        let root = temporary.path().join("config");
        let (store, _) = SettingsStore::open(&root).expect("create root");
        drop(store);
        let abandoned = root.join(".settings.tmp-0123456789abcdef-0123456789abcdef");
        let abandoned_identity = root.join(".identity.tmp-fedcba9876543210-fedcba9876543210");
        let similar = root.join(".settings.tmp-not-owned");
        let similar_identity = root.join(".identity.tmp-not-owned");
        write_fixture(&abandoned, b"partial").expect("abandoned");
        write_fixture(&abandoned_identity, b"partial").expect("abandoned identity");
        write_fixture(&similar, b"leave alone").expect("similar");
        write_fixture(&similar_identity, b"leave alone").expect("similar identity");
        let (_store, _) = SettingsStore::open(&root).expect("cleanup");
        assert!(!abandoned.exists());
        assert!(!abandoned_identity.exists());
        assert!(similar.exists());
        assert!(similar_identity.exists());
    }

    struct FailOnce {
        stage: CommitStage,
        fired: AtomicBool,
    }

    impl FailOnce {
        const fn new(stage: CommitStage) -> Self {
            Self {
                stage,
                fired: AtomicBool::new(false),
            }
        }
    }

    impl CommitHook for FailOnce {
        fn check(&self, stage: CommitStage) -> io::Result<()> {
            if stage == self.stage && !self.fired.swap(true, Ordering::SeqCst) {
                Err(io::Error::new(io::ErrorKind::StorageFull, "injected"))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn pre_replace_disk_failures_preserve_the_previous_commit() {
        for stage in [
            CommitStage::CreateTemporary,
            CommitStage::WriteTemporary,
            CommitStage::SyncTemporary,
            CommitStage::ValidateTemporary,
            CommitStage::Replace,
        ] {
            let temporary = PrivateTempDir::new().expect("temporary root");
            let root = temporary.path().join("config");
            let (mut store, _) = SettingsStore::open(&root).expect("open");
            let original = store.snapshot().expect("ready").clone();
            store.set_commit_hook(Arc::new(FailOnce::new(stage)));
            assert!(matches!(
                store.commit(original.revision(), changed_settings()),
                Err(StorageError::Io {
                    kind: io::ErrorKind::StorageFull,
                    ..
                })
            ));
            drop(store);

            let (reopened, status) = SettingsStore::open(&root).expect("reopen");
            assert_eq!(status, LoadStatus::Loaded);
            assert_eq!(reopened.snapshot(), Some(&original));
        }
    }

    #[test]
    fn identity_pre_replace_failures_preserve_the_previous_commit() {
        for stage in [
            CommitStage::CreateTemporary,
            CommitStage::WriteTemporary,
            CommitStage::SyncTemporary,
            CommitStage::ValidateTemporary,
            CommitStage::Replace,
        ] {
            let temporary = PrivateTempDir::new().expect("temporary root");
            let root = temporary.path().join("config");
            let (mut store, _) = open_seeded(&root, 12).expect("open");
            let original = store.identity_snapshot().expect("identity").clone();
            store.set_commit_hook(Arc::new(FailOnce::new(stage)));
            assert!(matches!(
                store.rotate_identity(original.revision()),
                Err(StorageError::Io {
                    kind: io::ErrorKind::StorageFull,
                    ..
                })
            ));
            drop(store);

            let (reopened, _) = open_seeded(&root, 13).expect("reopen");
            assert_eq!(reopened.identity_snapshot(), Some(&original));
        }
    }

    #[test]
    fn directory_sync_failure_is_indeterminate_until_reload() {
        let temporary = PrivateTempDir::new().expect("temporary root");
        let root = temporary.path().join("config");
        let (mut store, _) = SettingsStore::open(&root).expect("open");
        let original = store.snapshot().expect("ready").clone();
        store.set_commit_hook(Arc::new(FailOnce::new(CommitStage::SyncDirectory)));
        assert_eq!(
            store.commit(original.revision(), changed_settings()),
            Err(StorageError::IndeterminateCommit)
        );
        assert_eq!(
            store.commit(original.revision(), DesktopSettings::default()),
            Err(StorageError::ReloadRequired)
        );
        assert_eq!(store.reload().expect("reload"), LoadStatus::Loaded);
        let loaded = store.snapshot().expect("ready");
        assert_eq!(loaded.revision().get(), 2);
        assert_eq!(loaded.settings(), &changed_settings());
    }

    #[test]
    fn identity_directory_sync_failure_requires_reload() {
        let temporary = PrivateTempDir::new().expect("temporary root");
        let root = temporary.path().join("config");
        let (mut store, _) = open_seeded(&root, 14).expect("open");
        let original = store.identity_snapshot().expect("identity").clone();
        store.set_commit_hook(Arc::new(FailOnce::new(CommitStage::SyncDirectory)));
        assert_eq!(
            store.rotate_identity(original.revision()),
            Err(StorageError::IndeterminateCommit)
        );
        assert_eq!(
            store.rotate_identity(original.revision()),
            Err(StorageError::ReloadRequired)
        );
        assert_eq!(store.reload().expect("reload"), LoadStatus::Loaded);
        let loaded = store.identity_snapshot().expect("identity ready");
        assert_eq!(loaded.revision().get(), 2);
        assert_ne!(loaded.identity(), original.identity());
    }

    #[cfg(unix)]
    #[test]
    fn unix_entries_are_tightened_and_symlinks_are_rejected() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let temporary = PrivateTempDir::new().expect("temporary root");
        let root = temporary.path().join("config");
        let (store, _) = SettingsStore::open(&root).expect("open");
        assert_eq!(
            fs::metadata(&root)
                .expect("root metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(root.join(SETTINGS_FILE_NAME))
                .expect("settings metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(root.join(IDENTITY_FILE_NAME))
                .expect("identity metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        drop(store);

        fs::set_permissions(&root, fs::Permissions::from_mode(0o777)).expect("widen root");
        fs::set_permissions(
            root.join(SETTINGS_FILE_NAME),
            fs::Permissions::from_mode(0o666),
        )
        .expect("widen settings");
        fs::set_permissions(
            root.join(IDENTITY_FILE_NAME),
            fs::Permissions::from_mode(0o666),
        )
        .expect("widen identity");
        let (store, _) = SettingsStore::open(&root).expect("tighten existing entries");
        assert_eq!(
            fs::metadata(&root)
                .expect("root metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(root.join(SETTINGS_FILE_NAME))
                .expect("settings metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(root.join(IDENTITY_FILE_NAME))
                .expect("identity metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        drop(store);

        fs::remove_file(root.join(SETTINGS_FILE_NAME)).expect("remove settings");
        let target = root.join("outside.json");
        fs::write(&target, b"do not follow").expect("target");
        symlink(&target, root.join(SETTINGS_FILE_NAME)).expect("symlink");
        assert!(matches!(
            SettingsStore::open(&root),
            Err(StorageError::Document {
                problem: StorageProblem::UnsafeEntryKind,
                ..
            })
        ));
        assert_eq!(
            fs::read(target).expect("target untouched"),
            b"do not follow"
        );
    }

    #[cfg(unix)]
    #[test]
    fn permission_tightening_targets_the_validated_open_descriptor() {
        use std::os::unix::fs::PermissionsExt;

        let temporary = PrivateTempDir::new().expect("temporary root");
        let original_path = temporary.path().join("opened");
        let renamed_path = temporary.path().join("opened-renamed");
        let replacement_path = original_path.clone();

        fs::write(&original_path, b"opened inode").expect("create original");
        fs::set_permissions(&original_path, fs::Permissions::from_mode(0o666))
            .expect("widen original");
        let opened = File::open(&original_path).expect("open original");
        validate_opened_file(&original_path, &opened, StorageFileKind::Settings)
            .expect("validate original descriptor");

        fs::rename(&original_path, &renamed_path).expect("move opened inode");
        fs::write(&replacement_path, b"replacement inode").expect("create replacement");
        fs::set_permissions(&replacement_path, fs::Permissions::from_mode(0o666))
            .expect("widen replacement");

        tighten_open_file_permissions(&opened, StorageFileKind::Settings)
            .expect("tighten opened inode");

        assert_eq!(
            fs::metadata(&renamed_path)
                .expect("opened inode metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&replacement_path)
                .expect("replacement metadata")
                .permissions()
                .mode()
                & 0o777,
            0o666
        );
        assert!(matches!(
            validate_opened_file(&replacement_path, &opened, StorageFileKind::Settings),
            Err(StorageError::Document {
                problem: StorageProblem::UnsafeEntryKind,
                ..
            })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn hard_link_anomaly_is_rejected_before_settings_read() {
        let temporary = PrivateTempDir::new().expect("temporary root");
        let root = temporary.path().join("config");
        let (store, _) = SettingsStore::open(&root).expect("open");
        drop(store);
        fs::hard_link(
            root.join(SETTINGS_FILE_NAME),
            root.join("unexpected-hard-link.json"),
        )
        .expect("create hard link");
        assert!(matches!(
            SettingsStore::open(&root),
            Err(StorageError::Document {
                problem: StorageProblem::HardLinkDetected,
                ..
            })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn hard_link_anomaly_is_rejected_before_identity_read() {
        let temporary = PrivateTempDir::new().expect("temporary root");
        let root = temporary.path().join("config");
        let (store, _) = SettingsStore::open(&root).expect("open");
        drop(store);
        fs::hard_link(
            root.join(IDENTITY_FILE_NAME),
            root.join("unexpected-identity-hard-link.json"),
        )
        .expect("create hard link");
        assert!(matches!(
            SettingsStore::open(&root),
            Err(StorageError::Document {
                problem: StorageProblem::HardLinkDetected,
                ..
            })
        ));
    }

    #[test]
    fn standard_error_formatting_never_includes_document_values() {
        let Err(raw_error) = serde_json::from_slice::<EnvelopeHeader>(b"secret-token") else {
            panic!("invalid JSON unexpectedly parsed");
        };
        let error = json_error(raw_error);
        let debug = format!("{error:?}");
        let display = error.to_string();
        assert!(!debug.contains("secret-token"));
        assert!(!display.contains("secret-token"));
    }
}
