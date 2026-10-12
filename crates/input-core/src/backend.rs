use crate::{
    ButtonAction, InputAction, LogicalCommand, MediaCommand, MouseButton, OperationClass,
    ScrollAxis,
};
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

    /// Returns whether no input capability is currently advertised.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.relative_pointer.is_available()
            && self.mouse_buttons.is_empty()
            && !self.atomic_click.is_available()
            && !self.vertical_scroll.is_available()
            && !self.horizontal_scroll.is_available()
            && !self.physical_keys.is_available()
            && self.text_modes.is_empty()
            && self.logical_commands.is_empty()
            && self.media_commands.is_empty()
    }

    /// Returns whether this snapshot advertises the action's capability.
    #[must_use]
    pub fn supports(&self, action: &InputAction) -> bool {
        match action {
            InputAction::PointerMove(_) => self.relative_pointer.is_available(),
            InputAction::Scroll(delta) => match delta.axis() {
                ScrollAxis::Horizontal => self.horizontal_scroll.is_available(),
                ScrollAxis::Vertical => self.vertical_scroll.is_available(),
            },
            InputAction::MouseButton(ButtonAction::Down(button) | ButtonAction::Up(button)) => {
                self.mouse_buttons.contains(button)
            }
            InputAction::MouseButton(ButtonAction::Click { button, .. }) => {
                self.atomic_click.is_available() && self.mouse_buttons.contains(button)
            }
            InputAction::PhysicalKey(_) | InputAction::TextEdit(_) => {
                self.physical_keys.is_available()
            }
            InputAction::TextInsert(_) => !self.text_modes.is_empty(),
            InputAction::LogicalCommand(command) => self.logical_commands.contains(command),
            InputAction::MediaCommand(command) => self.media_commands.contains(command),
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

    /// Validates current availability and the action-specific capability.
    ///
    /// This is a last-moment preflight, not a replacement for native checks in
    /// the backend. It carries no action payload into its error.
    ///
    /// # Errors
    ///
    /// Returns a safe availability category when the backend, permission, or
    /// action-specific capability is unavailable.
    pub fn validate_action(&self, action: &InputAction) -> Result<(), CapabilityRejection> {
        if !self.initialized || self.fault.is_some() {
            return Err(CapabilityRejection::BackendUnavailable);
        }
        if !matches!(
            self.permission,
            PermissionState::NotRequired | PermissionState::Granted
        ) {
            return Err(CapabilityRejection::PermissionUnavailable);
        }
        if !self.available {
            return Err(CapabilityRejection::BackendUnavailable);
        }
        if !self.capabilities.supports(action) {
            return Err(CapabilityRejection::CapabilityUnavailable);
        }
        Ok(())
    }
}

/// Safe reason an action was rejected before native backend execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityRejection {
    BackendUnavailable,
    PermissionUnavailable,
    CapabilityUnavailable,
}

impl fmt::Display for CapabilityRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::BackendUnavailable => "input backend is unavailable",
            Self::PermissionUnavailable => "input permission is unavailable",
            Self::CapabilityUnavailable => "input capability is unavailable",
        })
    }
}

impl std::error::Error for CapabilityRejection {}

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

    /// Returns true only for an internally consistent neutrality claim.
    #[must_use]
    pub fn is_confirmed_neutral(&self) -> bool {
        self.neutral && self.failures.is_empty() && self.released == self.attempted
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

/// Failure from last-moment capability validation or native execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedExecutionError {
    Capability(CapabilityRejection),
    Backend(BackendError),
}

impl fmt::Display for CheckedExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capability(error) => error.fmt(formatter),
            Self::Backend(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CheckedExecutionError {}

/// Rechecks cached authoritative capability state immediately before execution.
///
/// The status lookup is non-blocking by contract. A dynamic native loss can
/// still occur afterward and remains the backend's responsibility.
///
/// # Errors
///
/// Returns a payload-free capability rejection or sanitized backend error.
pub fn execute_checked(
    backend: &mut dyn InputBackend,
    action: &InputAction,
) -> Result<(), CheckedExecutionError> {
    backend
        .status()
        .validate_action(action)
        .map_err(CheckedExecutionError::Capability)?;
    backend
        .execute(action)
        .map_err(CheckedExecutionError::Backend)
}
