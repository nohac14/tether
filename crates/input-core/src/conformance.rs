//! Reusable, opt-in backend conformance checks.
//!
//! Platform crates run this suite against their production mapper wired to an
//! inert event sink. It must never be pointed at a live native injector during
//! normal `cargo test`; guarded workstation injection remains a separate gate.

use crate::{
    ALLOWLISTED_HID_USAGE_RANGES, BackendError, BackendErrorCategory, BackendKind, BackendStatus,
    ButtonAction, CapabilityRejection, CheckedExecutionError, ClickCount, InputAction,
    InputBackend, KeyAction, LOGICAL_COMMANDS, MediaCommand, MouseButton, OperationClass,
    PhysicalKey, PointerDelta, ReleaseFailure, ReleaseReport, RetryScope, ScrollAxis, ScrollDelta,
    SensitiveText, StatusRefreshReason, TextEditCommand, execute_checked,
};
use std::fmt;

const MEDIA_COMMANDS: [MediaCommand; 6] = [
    MediaCommand::PlayPause,
    MediaCommand::NextTrack,
    MediaCommand::PreviousTrack,
    MediaCommand::VolumeUp,
    MediaCommand::VolumeDown,
    MediaCommand::Mute,
];

const TEXT_EDIT_COMMANDS: [TextEditCommand; 3] = [
    TextEditCommand::DeleteBackward,
    TextEditCommand::DeleteForward,
    TextEditCommand::InsertLineBreak,
];

/// Aggregate counts from one successful inert conformance run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackendConformanceReport {
    pub checked_actions: usize,
    pub executed_actions: usize,
    pub rejected_unsupported_actions: usize,
    pub explicitly_unsupported_mappings: usize,
    pub release_checks: usize,
}

/// Safe failure from the shared backend conformance suite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendConformanceFailure {
    UnexpectedBackendKind,
    InconsistentStatus,
    SupportedActionRejected {
        operation: OperationClass,
        reason: CapabilityRejection,
    },
    UnsupportedActionReachedBackend {
        operation: OperationClass,
    },
    ExecutionFailed {
        operation: OperationClass,
        category: BackendErrorCategory,
    },
    ReleaseNotNeutral,
}

impl fmt::Display for BackendConformanceFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnexpectedBackendKind => "backend kind did not match the conformance target",
            Self::InconsistentStatus => "backend status and capabilities were inconsistent",
            Self::SupportedActionRejected { .. } => {
                "an advertised capability was rejected before execution"
            }
            Self::UnsupportedActionReachedBackend { .. } => {
                "an unsupported action reached backend execution"
            }
            Self::ExecutionFailed { .. } => "an advertised backend action failed",
            Self::ReleaseNotNeutral => "backend release did not confirm a neutral state",
        })
    }
}

impl std::error::Error for BackendConformanceFailure {}

/// Platform mapper exercised by the inert backend conformance harness.
///
/// Implementations are the same pure translation and capability logic a live
/// backend will use. They must not call native injection APIs from these
/// methods. A platform crate supplies a separate inert sink for standard tests.
pub trait InertConformanceMapper: Send {
    type Event: Send;

    fn status(&self) -> BackendStatus;
    fn refresh_status(&mut self, reason: StatusRefreshReason) -> BackendStatus;
    /// Translates one platform-neutral action into inert native events.
    ///
    /// # Errors
    ///
    /// Returns a redacted backend error when the action is unavailable or
    /// cannot be represented safely by this mapper.
    fn map(&mut self, action: &InputAction) -> Result<Vec<Self::Event>, BackendError>;

    /// Clears mapper-local transient state such as fractional scroll residue.
    ///
    /// Held native buttons and keys are released individually by the harness.
    ///
    /// # Errors
    ///
    /// Returns a safe release category when local mapper state cannot be
    /// returned to neutral.
    fn reset_local_state(&mut self) -> Result<(), ReleaseFailure> {
        Ok(())
    }
}

/// Sanitized failure from an inert or live-event sink adapter.
///
/// `accepted` is the number of leading events whose native state may have
/// changed. It contains no input identity or payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertSinkError {
    accepted: usize,
}

impl InertSinkError {
    #[must_use]
    pub const fn new(accepted: usize) -> Self {
        Self { accepted }
    }

    #[must_use]
    pub const fn accepted(self) -> usize {
        self.accepted
    }
}

/// Event target used by the conformance backend.
pub trait InertConformanceSink<E>: Send {
    /// Accepts one already-translated event batch without native injection.
    ///
    /// # Errors
    ///
    /// Reports how many leading events may have been accepted when the batch
    /// cannot be completed.
    fn emit(&mut self, events: &[E]) -> Result<(), InertSinkError>;
}

/// Reusable `InputBackend` wrapper for exercising a platform mapper inertly.
///
/// The wrapper owns the backend-side held-state mirror required by the shared
/// contract. It is intentionally available only with `backend-conformance` (or
/// while testing `input-core`) and is not a native injector.
pub struct InertConformanceBackend<M, S>
where
    M: InertConformanceMapper,
    S: InertConformanceSink<M::Event>,
{
    mapper: M,
    sink: S,
    held_buttons: Vec<MouseButton>,
    held_keys: Vec<PhysicalKey>,
    uncertain_native_state: bool,
}

impl<M, S> InertConformanceBackend<M, S>
where
    M: InertConformanceMapper,
    S: InertConformanceSink<M::Event>,
{
    #[must_use]
    pub const fn new(mapper: M, sink: S) -> Self {
        Self {
            mapper,
            sink,
            held_buttons: Vec::new(),
            held_keys: Vec::new(),
            uncertain_native_state: false,
        }
    }

    #[must_use]
    pub const fn mapper(&self) -> &M {
        &self.mapper
    }

    #[must_use]
    pub const fn sink(&self) -> &S {
        &self.sink
    }

    #[must_use]
    pub const fn sink_mut(&mut self) -> &mut S {
        &mut self.sink
    }

    #[must_use]
    pub fn held_counts(&self) -> (usize, usize) {
        (self.held_buttons.len(), self.held_keys.len())
    }

    #[must_use]
    pub const fn has_uncertain_native_state(&self) -> bool {
        self.uncertain_native_state
    }

    fn emit_action(&mut self, action: &InputAction) -> Result<(), BackendError> {
        let events = self.mapper.map(action)?;
        match self.sink.emit(&events) {
            Ok(()) => {
                self.apply_held_transition(action);
                Ok(())
            }
            Err(error) => {
                let category = if error.accepted() == 0 {
                    BackendErrorCategory::NativeCallFailed
                } else if error.accepted() <= events.len() {
                    self.uncertain_native_state = true;
                    BackendErrorCategory::PartialStateChange
                } else {
                    self.uncertain_native_state = true;
                    BackendErrorCategory::Fatal
                };
                Err(BackendError::new(
                    category,
                    action.class(),
                    RetryScope::AfterSessionRestart,
                ))
            }
        }
    }

    fn apply_held_transition(&mut self, action: &InputAction) {
        match action {
            InputAction::MouseButton(ButtonAction::Down(button)) => {
                if !self.held_buttons.contains(button) {
                    self.held_buttons.push(*button);
                }
            }
            InputAction::MouseButton(ButtonAction::Up(button)) => {
                self.held_buttons.retain(|held| held != button);
            }
            InputAction::PhysicalKey(KeyAction::Down(key)) => {
                if !self.held_keys.contains(key) {
                    self.held_keys.push(*key);
                }
            }
            InputAction::PhysicalKey(KeyAction::Up(key)) => {
                self.held_keys.retain(|held| held != key);
            }
            _ => {}
        }
    }

    fn try_release_action(&mut self, action: &InputAction) -> bool {
        let Ok(events) = self.mapper.map(action) else {
            return false;
        };
        match self.sink.emit(&events) {
            Ok(()) => {
                self.apply_held_transition(action);
                true
            }
            Err(error) => {
                self.uncertain_native_state |= error.accepted() > 0;
                false
            }
        }
    }
}

impl<M, S> InputBackend for InertConformanceBackend<M, S>
where
    M: InertConformanceMapper,
    S: InertConformanceSink<M::Event>,
{
    fn status(&self) -> BackendStatus {
        self.mapper.status()
    }

    fn refresh_status(&mut self, reason: StatusRefreshReason) -> BackendStatus {
        self.mapper.refresh_status(reason)
    }

    fn execute(&mut self, action: &InputAction) -> Result<(), BackendError> {
        self.emit_action(action)
    }

    fn release_all(&mut self) -> ReleaseReport {
        let attempted = self.held_buttons.len().saturating_add(self.held_keys.len());
        let mut released = 0_usize;
        let mut failures = Vec::new();

        for button in self.held_buttons.clone().into_iter().rev() {
            let action = InputAction::MouseButton(ButtonAction::Up(button));
            if self.try_release_action(&action) {
                released = released.saturating_add(1);
            } else {
                failures.push(ReleaseFailure::MouseButton);
            }
        }
        let keys = self.held_keys.clone();
        for key in keys
            .iter()
            .rev()
            .filter(|key| !key.is_modifier())
            .chain(keys.iter().rev().filter(|key| key.is_modifier()))
            .copied()
        {
            let failure = if key.is_modifier() {
                ReleaseFailure::ModifierKey
            } else {
                ReleaseFailure::NonModifierKey
            };
            let action = InputAction::PhysicalKey(KeyAction::Up(key));
            if self.try_release_action(&action) {
                released = released.saturating_add(1);
            } else {
                failures.push(failure);
            }
        }
        if let Err(failure) = self.mapper.reset_local_state() {
            failures.push(failure);
        }
        if self.uncertain_native_state && !failures.contains(&ReleaseFailure::BackendLocalState) {
            failures.push(ReleaseFailure::BackendLocalState);
        }

        let neutral = self.held_buttons.is_empty()
            && self.held_keys.is_empty()
            && !self.uncertain_native_state
            && failures.is_empty();
        ReleaseReport {
            attempted: u32::try_from(attempted).unwrap_or(u32::MAX),
            released: u32::try_from(released).unwrap_or(u32::MAX),
            failures,
            neutral,
        }
    }
}

/// Exercises canonical mappings, capability gating, and idempotent release.
///
/// The supplied backend must use the same mapping and held-state code as its
/// production implementation but an inert event sink. The suite executes every
/// allowlisted HID usage when physical keys are advertised (successful down/up
/// or an explicit `Unsupported` result), every advertised button/logical/media
/// command, both axes, text/edit paths, and release with held state. Unsupported
/// representative actions must stop at the shared gate.
///
/// # Errors
///
/// Returns a payload-free failure at the first contract violation.
pub fn run_backend_conformance(
    backend: &mut dyn InputBackend,
    expected_kind: BackendKind,
) -> Result<BackendConformanceReport, BackendConformanceFailure> {
    let status = backend.status();
    if status.kind != expected_kind {
        return Err(BackendConformanceFailure::UnexpectedBackendKind);
    }
    let permission_ready = matches!(
        status.permission,
        crate::PermissionState::NotRequired | crate::PermissionState::Granted
    );
    if status.available && (!status.initialized || !permission_ready || status.fault.is_some())
        || !status.available && !status.capabilities.is_empty()
    {
        return Err(BackendConformanceFailure::InconsistentStatus);
    }

    ensure_neutral(backend)?;
    let mut report = BackendConformanceReport {
        checked_actions: 0,
        executed_actions: 0,
        rejected_unsupported_actions: 0,
        explicitly_unsupported_mappings: 0,
        release_checks: 1,
    };

    for action in &representative_actions()? {
        check_action(backend, &status, action, &mut report)?;
    }
    exercise_physical_keys(backend, &status, &mut report)?;
    exercise_buttons(backend, &status, &mut report)?;
    establish_held_state(backend, &status, &mut report)?;

    ensure_neutral(backend)?;
    ensure_neutral(backend)?;
    report.release_checks += 2;
    Ok(report)
}

fn representative_actions() -> Result<Vec<InputAction>, BackendConformanceFailure> {
    let pointer =
        PointerDelta::new(1, -1).map_err(|_| BackendConformanceFailure::InconsistentStatus)?;
    let vertical = ScrollDelta::new(ScrollAxis::Vertical, 1)
        .map_err(|_| BackendConformanceFailure::InconsistentStatus)?;
    let horizontal = ScrollDelta::new(ScrollAxis::Horizontal, -1)
        .map_err(|_| BackendConformanceFailure::InconsistentStatus)?;
    let text = SensitiveText::new("conformance-fixture".to_owned())
        .map_err(|_| BackendConformanceFailure::InconsistentStatus)?;
    let mut actions = vec![
        InputAction::PointerMove(pointer),
        InputAction::Scroll(vertical),
        InputAction::Scroll(horizontal),
        InputAction::TextInsert(text),
    ];
    actions.extend(TEXT_EDIT_COMMANDS.into_iter().map(InputAction::TextEdit));
    actions.extend(
        LOGICAL_COMMANDS
            .into_iter()
            .map(InputAction::LogicalCommand),
    );
    actions.extend(MEDIA_COMMANDS.into_iter().map(InputAction::MediaCommand));
    actions.extend(
        [
            MouseButton::Left,
            MouseButton::Right,
            MouseButton::Middle,
            MouseButton::Back,
            MouseButton::Forward,
        ]
        .into_iter()
        .map(|button| {
            InputAction::MouseButton(ButtonAction::Click {
                button,
                count: ClickCount::Single,
            })
        }),
    );
    Ok(actions)
}

fn exercise_physical_keys(
    backend: &mut dyn InputBackend,
    status: &crate::BackendStatus,
    report: &mut BackendConformanceReport,
) -> Result<(), BackendConformanceFailure> {
    if !status.capabilities.physical_keys.is_available() {
        let key =
            PhysicalKey::new(0x04).map_err(|_| BackendConformanceFailure::InconsistentStatus)?;
        return check_action(
            backend,
            status,
            &InputAction::PhysicalKey(KeyAction::Down(key)),
            report,
        );
    }
    for range in ALLOWLISTED_HID_USAGE_RANGES {
        for usage in range.first..=range.last {
            let key = PhysicalKey::new(u16::from(usage))
                .map_err(|_| BackendConformanceFailure::InconsistentStatus)?;
            exercise_physical_mapping(backend, key, report)?;
        }
    }
    Ok(())
}

fn exercise_physical_mapping(
    backend: &mut dyn InputBackend,
    key: PhysicalKey,
    report: &mut BackendConformanceReport,
) -> Result<(), BackendConformanceFailure> {
    let down = InputAction::PhysicalKey(KeyAction::Down(key));
    report.checked_actions += 1;
    match execute_checked(backend, &down) {
        Ok(()) => {
            report.executed_actions += 1;
        }
        Err(CheckedExecutionError::Backend(error))
            if error.category() == BackendErrorCategory::Unsupported =>
        {
            report.explicitly_unsupported_mappings += 1;
            return Ok(());
        }
        Err(CheckedExecutionError::Capability(reason)) => {
            return Err(BackendConformanceFailure::SupportedActionRejected {
                operation: down.class(),
                reason,
            });
        }
        Err(CheckedExecutionError::Backend(error)) => {
            return Err(BackendConformanceFailure::ExecutionFailed {
                operation: down.class(),
                category: error.category(),
            });
        }
    }

    let up = InputAction::PhysicalKey(KeyAction::Up(key));
    execute_required(backend, &up)?;
    report.checked_actions += 1;
    report.executed_actions += 1;
    Ok(())
}

fn exercise_buttons(
    backend: &mut dyn InputBackend,
    status: &crate::BackendStatus,
    report: &mut BackendConformanceReport,
) -> Result<(), BackendConformanceFailure> {
    for button in &status.capabilities.mouse_buttons {
        execute_required(
            backend,
            &InputAction::MouseButton(ButtonAction::Down(*button)),
        )?;
        execute_required(
            backend,
            &InputAction::MouseButton(ButtonAction::Up(*button)),
        )?;
        report.checked_actions += 2;
        report.executed_actions += 2;
    }
    Ok(())
}

fn establish_held_state(
    backend: &mut dyn InputBackend,
    status: &crate::BackendStatus,
    report: &mut BackendConformanceReport,
) -> Result<(), BackendConformanceFailure> {
    if let Some(button) = status.capabilities.mouse_buttons.first() {
        execute_required(
            backend,
            &InputAction::MouseButton(ButtonAction::Down(*button)),
        )?;
        report.checked_actions += 1;
        report.executed_actions += 1;
    }
    if status.capabilities.physical_keys.is_available() {
        let key =
            PhysicalKey::new(0x04).map_err(|_| BackendConformanceFailure::InconsistentStatus)?;
        execute_required(backend, &InputAction::PhysicalKey(KeyAction::Down(key)))?;
        report.checked_actions += 1;
        report.executed_actions += 1;
    }
    Ok(())
}

fn check_action(
    backend: &mut dyn InputBackend,
    status: &crate::BackendStatus,
    action: &InputAction,
    report: &mut BackendConformanceReport,
) -> Result<(), BackendConformanceFailure> {
    report.checked_actions += 1;
    let advertised = status.capabilities.supports(action)
        && status.available
        && status.initialized
        && status.fault.is_none()
        && matches!(
            status.permission,
            crate::PermissionState::NotRequired | crate::PermissionState::Granted
        );
    match (advertised, execute_checked(backend, action)) {
        (true, Ok(())) => report.executed_actions += 1,
        (true, Err(CheckedExecutionError::Capability(reason))) => {
            return Err(BackendConformanceFailure::SupportedActionRejected {
                operation: action.class(),
                reason,
            });
        }
        (true, Err(CheckedExecutionError::Backend(error))) => {
            return Err(BackendConformanceFailure::ExecutionFailed {
                operation: action.class(),
                category: error.category(),
            });
        }
        (false, Err(CheckedExecutionError::Capability(_))) => {
            report.rejected_unsupported_actions += 1;
        }
        (false, Ok(()) | Err(CheckedExecutionError::Backend(_))) => {
            return Err(BackendConformanceFailure::UnsupportedActionReachedBackend {
                operation: action.class(),
            });
        }
    }
    Ok(())
}

fn execute_required(
    backend: &mut dyn InputBackend,
    action: &InputAction,
) -> Result<(), BackendConformanceFailure> {
    match execute_checked(backend, action) {
        Ok(()) => Ok(()),
        Err(CheckedExecutionError::Capability(reason)) => {
            Err(BackendConformanceFailure::SupportedActionRejected {
                operation: action.class(),
                reason,
            })
        }
        Err(CheckedExecutionError::Backend(error)) => {
            Err(BackendConformanceFailure::ExecutionFailed {
                operation: action.class(),
                category: error.category(),
            })
        }
    }
}

fn ensure_neutral(backend: &mut dyn InputBackend) -> Result<(), BackendConformanceFailure> {
    let report: ReleaseReport = backend.release_all();
    if report.is_confirmed_neutral() {
        Ok(())
    } else {
        Err(BackendConformanceFailure::ReleaseNotNeutral)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BackendCapabilities, BackendStatus, CapabilityState, MockInputBackend, PermissionState,
        StatusRefreshReason, TextMode,
    };
    use std::collections::BTreeSet;

    fn ready_mock() -> MockInputBackend {
        let mut mock = MockInputBackend::new();
        mock.stage_status(BackendStatus {
            revision: 0,
            kind: BackendKind::Mock,
            initialized: true,
            available: true,
            permission: PermissionState::NotRequired,
            capabilities: BackendCapabilities {
                relative_pointer: CapabilityState::Available,
                mouse_buttons: BTreeSet::from([
                    MouseButton::Left,
                    MouseButton::Right,
                    MouseButton::Middle,
                    MouseButton::Back,
                    MouseButton::Forward,
                ]),
                atomic_click: CapabilityState::Available,
                vertical_scroll: CapabilityState::Available,
                horizontal_scroll: CapabilityState::Available,
                physical_keys: CapabilityState::Available,
                text_modes: BTreeSet::from([TextMode::NativeUnicodeBestEffort]),
                logical_commands: BTreeSet::from(LOGICAL_COMMANDS),
                media_commands: BTreeSet::from(MEDIA_COMMANDS),
            },
            fault: None,
        });
        mock.refresh_status(StatusRefreshReason::Startup);
        mock
    }

    #[test]
    fn full_inert_backend_passes_mapping_and_release_suite() {
        let mut mock = ready_mock();
        let report = run_backend_conformance(&mut mock, BackendKind::Mock).expect("conformance");
        assert!(report.checked_actions > 400);
        assert_eq!(report.release_checks, 3);
        assert!(mock.is_neutral());
    }

    #[test]
    fn unsupported_action_is_rejected_before_backend_execution() {
        let mut mock = MockInputBackend::new();
        let action = InputAction::PointerMove(PointerDelta::new(1, 1).expect("fixture"));
        assert_eq!(
            execute_checked(&mut mock, &action),
            Err(CheckedExecutionError::Capability(
                CapabilityRejection::BackendUnavailable
            ))
        );
        assert_eq!(mock.calls(), []);
    }

    #[test]
    fn allowlisted_but_unmappable_physical_key_is_an_explicit_outcome() {
        let mut mock = ready_mock();
        mock.push_execute_result(Err(crate::BackendError::new(
            BackendErrorCategory::Unsupported,
            OperationClass::PhysicalKey,
            crate::RetryScope::Never,
        )));
        let key = PhysicalKey::new(0xB0).expect("allowlisted fixture");
        let mut report = BackendConformanceReport {
            checked_actions: 0,
            executed_actions: 0,
            rejected_unsupported_actions: 0,
            explicitly_unsupported_mappings: 0,
            release_checks: 0,
        };
        exercise_physical_mapping(&mut mock, key, &mut report).expect("explicit unsupported");
        assert_eq!(report.checked_actions, 1);
        assert_eq!(report.executed_actions, 0);
        assert_eq!(report.explicitly_unsupported_mappings, 1);
        assert!(mock.is_neutral());
    }

    #[test]
    fn status_overclaim_is_rejected() {
        let mut mock = MockInputBackend::new();
        let mut inconsistent = mock.status();
        inconsistent.capabilities.relative_pointer = CapabilityState::Available;
        mock.stage_status(inconsistent);
        mock.refresh_status(StatusRefreshReason::Startup);
        assert_eq!(
            run_backend_conformance(&mut mock, BackendKind::Mock),
            Err(BackendConformanceFailure::InconsistentStatus)
        );
    }
}
