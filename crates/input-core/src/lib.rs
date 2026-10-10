//! Platform-neutral input abstractions and dispatch boundary.
//!
//! Wire formats and operating-system event structures are deliberately kept
//! outside this crate's public domain model.

mod action;
mod backend;
mod mock;

pub use action::{
    ButtonAction, ClickCount, FIXED_POINT_SCALE, InputAction, KeyAction, LogicalCommand,
    MAX_ABSOLUTE_FIXED_DELTA, MAX_TEXT_BYTES, MediaCommand, MouseButton, OperationClass,
    PhysicalKey, PointerDelta, ScrollAxis, ScrollDelta, SensitiveText, TextEditCommand, ValueError,
};
pub use backend::{
    BackendCapabilities, BackendError, BackendErrorCategory, BackendKind, BackendStatus,
    CapabilityState, InputBackend, PermissionState, ReleaseFailure, ReleaseReport, RetryScope,
    StatusRefreshReason, TextMode,
};
pub use mock::{MockCall, MockInputBackend, MockOutcome};
