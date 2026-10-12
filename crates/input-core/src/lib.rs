//! Platform-neutral input abstractions and dispatch boundary.
//!
//! Wire formats and operating-system event structures are deliberately kept
//! outside this crate's public domain model.

mod action;
mod backend;
#[cfg(any(test, feature = "backend-conformance"))]
mod conformance;
mod dispatcher;
mod mapping;
mod mock;
mod normalization;

pub use action::{
    ButtonAction, ClickCount, FIXED_POINT_SCALE, InputAction, KeyAction, LogicalCommand,
    MAX_ABSOLUTE_FIXED_DELTA, MAX_TEXT_BYTES, MediaCommand, MouseButton, OperationClass,
    PhysicalKey, PointerDelta, ScrollAxis, ScrollDelta, SensitiveText, TextEditCommand, ValueError,
};
pub use backend::{
    BackendCapabilities, BackendError, BackendErrorCategory, BackendKind, BackendStatus,
    CapabilityRejection, CapabilityState, CheckedExecutionError, InputBackend, PermissionState,
    ReleaseFailure, ReleaseReport, RetryScope, StatusRefreshReason, TextMode, execute_checked,
};
#[cfg(any(test, feature = "backend-conformance"))]
pub use conformance::{
    BackendConformanceFailure, BackendConformanceReport, run_backend_conformance,
};
pub use dispatcher::{
    AdmissionError, CancellationReason, DEFAULT_QUEUE_CAPACITY, DispatchOutcome, DispatchPosition,
    DispatchReceipt, DispatcherConfig, DispatcherConfigError, DispatcherMode, DispatcherSnapshot,
    EmergencyReason, InputDispatcher, MAX_QUEUE_CAPACITY, RejectionReason, ReleaseOutcome,
    ReleaseTicket, ResumeError, ShutdownResult, TicketError,
};
pub use mapping::{
    ALLOWLISTED_HID_USAGE_RANGES, CanonicalChord, DesktopShortcutPlatform, HID_KEYBOARD_USAGE_PAGE,
    HidUsageClass, HidUsageRange, LOGICAL_COMMANDS, canonical_logical_chord,
    is_allowlisted_keyboard_usage,
};
pub use mock::{MockCall, MockInputBackend, MockOutcome};
pub use normalization::{
    DiscreteScrollDelta, DiscreteScrollTranslator, MAX_ACCELERATION_MILLI, MAX_SENSITIVITY_MILLI,
    MIN_SENSITIVITY_MILLI, NativePointerDelta, NativeUnitQuantum, NormalizationConfigError,
    NormalizationLimit, PointerAcceleration, PointerNormalizationConfig, PointerNormalizer,
    PointerUnitTranslator, ScrollAxisConfig, ScrollDirection, ScrollNormalizationConfig,
    ScrollNormalizer, Sensitivity,
};
