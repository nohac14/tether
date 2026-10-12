//! Reusable, opt-in backend conformance checks.
//!
//! Platform crates run this suite against their production mapper wired to an
//! inert event sink. It must never be pointed at a live native injector during
//! normal `cargo test`; guarded workstation injection remains a separate gate.

use crate::{
    ALLOWLISTED_HID_USAGE_RANGES, BackendErrorCategory, BackendKind, ButtonAction,
    CapabilityRejection, CheckedExecutionError, ClickCount, InputAction, InputBackend, KeyAction,
    LOGICAL_COMMANDS, MediaCommand, MouseButton, OperationClass, PhysicalKey, PointerDelta,
    ReleaseReport, ScrollAxis, ScrollDelta, SensitiveText, TextEditCommand, execute_checked,
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
