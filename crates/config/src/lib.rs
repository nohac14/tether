//! Versioned local configuration and persistence boundary.
//!
//! The crate accepts an application-data root from the desktop composition
//! layer. It never discovers paths from network or IPC input.

mod identity;
mod settings;
mod store;

pub use identity::{
    HOSTNAME_SEED_BYTES, HOSTNAME_SUFFIX_BYTES, INSTALLATION_ID_BYTES, IdentityValidationError,
    InstallationIdentity, LocalHostname, MAX_HOSTNAME_CONFLICT_ATTEMPT,
};
pub use settings::{
    ApprovedInterfaceId, DesktopSettings, LogLevel, MAX_APPROVED_INTERFACES,
    MAX_INTERFACE_ID_BYTES, SettingsValidationError,
};
pub use store::{
    CURRENT_IDENTITY_SCHEMA_VERSION, CURRENT_SETTINGS_SCHEMA_VERSION, IDENTITY_FILE_LIMIT,
    IdentityLoadStatus, IdentityRevision, IdentitySnapshot, LoadStatus, SETTINGS_FILE_LIMIT,
    SettingsRevision, SettingsSnapshot, SettingsStore, StorageError, StorageFileKind,
    StorageIoOperation, StorageProblem,
};
