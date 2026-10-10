use crate::{InputAction, LogicalCommand, MediaCommand, MouseButton, OperationClass};
use std::collections::BTreeSet;
use std::fmt;

/// Active native implementation selected for the desktop session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendKind {
    Mock,
    Windows,
    MacOs,
    X11,
    WaylandEi,
}

/// Current native permission state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PermissionState {
    NotRequired,
    Granted,
    Denied,
    PromptRequired,
    Unavailable,
    TransientlyUnknown,
}

/// Native text-injection mode currently available.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TextMode {
    ActiveLayoutMappable,
    NativeUnicodeBestEffort,
    EiUtf8,
}

/// Availability of one independently negotiated capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityState {
    Unavailable,
    Available,
}

impl CapabilityState {
    /// Returns whether the capability is currently available.
    #[must_use]
    pub const fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }
}

/// Conservative snapshot of functionality currently available.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendCapabilities {
    pub relative_pointer: CapabilityState,
    pub mouse_buttons: BTreeSet<MouseButton>,
    pub atomic_click: CapabilityState,
    pub vertical_scroll: CapabilityState,
    pub horizontal_scroll: CapabilityState,
    pub physical_keys: CapabilityState,
    pub text_modes: BTreeSet<TextMode>,
    pub logical_commands: BTreeSet<LogicalCommand>,
    pub media_commands: BTreeSet<MediaCommand>,
}

impl BackendCapabilities {
    /// Returns a conservative snapshot with no input capability enabled.
    #[must_use]
    pub const fn unavailable() -> Self {
        Self {
            relative_pointer: CapabilityState::Unavailable,
            mouse_buttons: BTreeSet::new(),
            atomic_click: CapabilityState::Unavailable,
            vertical_scroll: CapabilityState::Unavailable,
            horizontal_scroll: CapabilityState::Unavailable,
            physical_keys: CapabilityState::Unavailable,
            text_modes: BTreeSet::new(),
            logical_commands: BTreeSet::new(),
            media_commands: BTreeSet::new(),
        }
    }
}

impl Default for BackendCapabilities {
    fn default() -> Self {
        Self::unavailable()
    }
}

/// Cached non-blocking backend health and capability snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendStatus {
    pub revision: u64,
    pub kind: BackendKind,
    pub initialized: bool,
    pub available: bool,
    pub permission: PermissionState,
    pub capabilities: BackendCapabilities,
    pub fault: Option<BackendErrorCategory>,
}

impl BackendStatus {
    /// Returns a fail-closed unavailable status for a backend kind.
    #[must_use]
    pub const fn unavailable(kind: BackendKind) -> Self {
        Self {
            revision: 0,
            kind,
            initialized: false,
            available: false,
            permission: PermissionState::TransientlyUnknown,
            capabilities: BackendCapabilities::unavailable(),
            fault: None,
        }
    }
}

/// Reason for a bounded native status refresh.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusRefreshReason {
    Startup,
    UserRequest,
    PermissionNotification,
    SessionTransition,
    DeviceTopologyChanged,
    BeforeControl,
}

/// Safe backend error category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendErrorCategory {
    Unsupported,
    PermissionRequired,
    PermissionDenied,
    TemporarilyUnavailable,
    InvalidPlatformState,
    NativeCallFailed,
    PartialStateChange,
    Fatal,
}

/// Scope at which a failed operation may be retried.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryScope {
    Never,
    AfterStatusRefresh,
    AfterPermissionChange,
    AfterSessionRestart,
}

/// Closed error value that cannot carry native messages or input payloads.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct BackendError {
    category: BackendErrorCategory,
    operation: OperationClass,
    retry: RetryScope,
}

impl BackendError {
    /// Creates a sanitized backend error.
    #[must_use]
    pub const fn new(
        category: BackendErrorCategory,
        operation: OperationClass,
        retry: RetryScope,
    ) -> Self {
        Self {
            category,
            operation,
            retry,
        }
    }

    #[must_use]
    pub const fn category(self) -> BackendErrorCategory {
        self.category
    }

    #[must_use]
    pub const fn operation(self) -> OperationClass {
        self.operation
    }

    #[must_use]
    pub const fn retry_scope(self) -> RetryScope {
        self.retry
    }

    /// Returns whether ordinary admission must stop before cleanup.
    #[must_use]
    pub const fn requires_emergency_release(self) -> bool {
        matches!(
            self.category,
            BackendErrorCategory::PermissionDenied
                | BackendErrorCategory::InvalidPlatformState
                | BackendErrorCategory::PartialStateChange
                | BackendErrorCategory::Fatal
        )
    }
}

impl fmt::Debug for BackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BackendError")
            .field("category", &self.category)
            .field("operation", &self.operation)
            .field("retry", &self.retry)
            .finish()
    }
}

impl fmt::Display for BackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "backend {:?} during {:?}; retry {:?}",
            self.category, self.operation, self.retry
        )
    }
}

impl std::error::Error for BackendError {}

/// Safe release failure category without the identity of the held item.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseFailure {
    MouseButton,
    NonModifierKey,
    ModifierKey,
    BackendLocalState,
}

/// Outcome of a best-effort, idempotent release operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseReport {
    pub attempted: u32,
    pub released: u32,
    pub failures: Vec<ReleaseFailure>,
    pub neutral: bool,
}

impl ReleaseReport {
    /// Returns a successful already-neutral report.
    #[must_use]
    pub const fn neutral() -> Self {
        Self {
            attempted: 0,
            released: 0,
            failures: Vec::new(),
            neutral: true,
        }
    }
}

/// Serialized platform-native input boundary.
pub trait InputBackend: Send {
    /// Returns the current cached snapshot without blocking.
    fn status(&self) -> BackendStatus;

    /// Refreshes bounded platform state on the backend execution context.
    fn refresh_status(&mut self, reason: StatusRefreshReason) -> BackendStatus;

    /// Executes one already validated semantic action.
    ///
    /// # Errors
    ///
    /// Returns a sanitized [`BackendError`] when native execution cannot be
    /// completed with known state.
    fn execute(&mut self, action: &InputAction) -> Result<(), BackendError>;

    /// Best-effort release of all native state held by this backend.
    fn release_all(&mut self) -> ReleaseReport;
}
