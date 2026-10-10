use crate::{
    BackendError, BackendKind, BackendStatus, ButtonAction, InputAction, InputBackend, KeyAction,
    MouseButton, OperationClass, PhysicalKey, ReleaseReport, StatusRefreshReason,
};
use std::collections::{BTreeSet, VecDeque};
use std::fmt;
use std::time::Duration;

/// Sanitized result recorded for a mock call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MockOutcome {
    Succeeded,
    Failed(crate::BackendErrorCategory),
}

/// Sanitized deterministic record of one mock execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MockCall {
    pub sequence: u64,
    pub operation: OperationClass,
    pub outcome: MockOutcome,
}

/// In-memory backend used by protocol, session, and dispatcher tests.
///
/// It records only operation classes and sanitized outcomes. Input payloads,
/// HID usages, and button identities are never retained in the public call log.
pub struct MockInputBackend {
    status: BackendStatus,
    pending_status: Option<BackendStatus>,
    calls: Vec<MockCall>,
    next_sequence: u64,
    execute_results: VecDeque<Result<(), BackendError>>,
    release_reports: VecDeque<ReleaseReport>,
    held_buttons: BTreeSet<MouseButton>,
    held_keys: BTreeSet<PhysicalKey>,
    simulated_latency: Duration,
    simulated_elapsed: Duration,
    time_advance: Option<Box<dyn FnMut(Duration) + Send>>,
}

impl MockInputBackend {
    /// Creates a fail-closed mock. Tests explicitly enable capabilities or rely
    /// on a higher layer to validate them.
    #[must_use]
    pub fn new() -> Self {
        Self {
            status: BackendStatus::unavailable(BackendKind::Mock),
            pending_status: None,
            calls: Vec::new(),
            next_sequence: 0,
            execute_results: VecDeque::new(),
            release_reports: VecDeque::new(),
            held_buttons: BTreeSet::new(),
            held_keys: BTreeSet::new(),
            simulated_latency: Duration::ZERO,
            simulated_elapsed: Duration::ZERO,
            time_advance: None,
        }
    }

    /// Queues the sanitized result of a future execute call.
    pub fn push_execute_result(&mut self, result: Result<(), BackendError>) {
        self.execute_results.push_back(result);
    }

    /// Queues the report returned by a future release call.
    pub fn push_release_report(&mut self, report: ReleaseReport) {
        self.release_reports.push_back(report);
    }

    /// Stages a status for the next refresh. Revision is assigned by the mock.
    pub fn stage_status(&mut self, status: BackendStatus) {
        self.pending_status = Some(status);
    }

    /// Sets deterministic simulated call latency without sleeping.
    pub fn set_simulated_latency(&mut self, latency: Duration) {
        self.simulated_latency = latency;
    }

    /// Connects simulated latency to a test-owned monotonic clock.
    pub fn set_time_advance(&mut self, advance: impl FnMut(Duration) + Send + 'static) {
        self.time_advance = Some(Box::new(advance));
    }

    /// Returns total deterministic simulated execution time.
    #[must_use]
    pub const fn simulated_elapsed(&self) -> Duration {
        self.simulated_elapsed
    }

    /// Returns the sanitized call log.
    #[must_use]
    pub fn calls(&self) -> &[MockCall] {
        &self.calls
    }

    /// Returns held item counts without revealing identities.
    #[must_use]
    pub fn held_counts(&self) -> (usize, usize) {
        (self.held_buttons.len(), self.held_keys.len())
    }

    /// Returns whether the native-state mirror is neutral.
    #[must_use]
    pub fn is_neutral(&self) -> bool {
        self.held_buttons.is_empty() && self.held_keys.is_empty()
    }

    fn record(&mut self, operation: OperationClass, result: Result<(), BackendError>) {
        let outcome = match result {
            Ok(()) => MockOutcome::Succeeded,
            Err(error) => MockOutcome::Failed(error.category()),
        };
        self.calls.push(MockCall {
            sequence: self.next_sequence,
            operation,
            outcome,
        });
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.simulated_elapsed = self
            .simulated_elapsed
            .saturating_add(self.simulated_latency);
        if let Some(advance) = &mut self.time_advance {
            advance(self.simulated_latency);
        }
    }

    fn update_mirror(&mut self, action: &InputAction) {
        match action {
            InputAction::MouseButton(ButtonAction::Down(button)) => {
                self.held_buttons.insert(*button);
            }
            InputAction::MouseButton(ButtonAction::Up(button)) => {
                self.held_buttons.remove(button);
            }
            InputAction::PhysicalKey(KeyAction::Down(key)) => {
                self.held_keys.insert(*key);
            }
            InputAction::PhysicalKey(KeyAction::Up(key)) => {
                self.held_keys.remove(key);
            }
            _ => {}
        }
    }
}

impl Default for MockInputBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for MockInputBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MockInputBackend")
            .field("status_revision", &self.status.revision)
            .field("call_count", &self.calls.len())
            .field("held_button_count", &self.held_buttons.len())
            .field("held_key_count", &self.held_keys.len())
            .finish_non_exhaustive()
    }
}

impl InputBackend for MockInputBackend {
    fn status(&self) -> BackendStatus {
        self.status.clone()
    }

    fn refresh_status(&mut self, _reason: StatusRefreshReason) -> BackendStatus {
        if let Some(mut next) = self.pending_status.take() {
            let changed = BackendStatus {
                revision: self.status.revision,
                ..next.clone()
            } != self.status;
            next.revision = if changed {
                self.status.revision.saturating_add(1)
            } else {
                self.status.revision
            };
            self.status = next;
        }
        self.status.clone()
    }

    fn execute(&mut self, action: &InputAction) -> Result<(), BackendError> {
        let result = self.execute_results.pop_front().unwrap_or(Ok(()));
        if result.is_ok() {
            self.update_mirror(action);
        }
        self.record(action.class(), result);
        result
    }

    fn release_all(&mut self) -> ReleaseReport {
        let held = self.held_buttons.len().saturating_add(self.held_keys.len());
        let default_report = ReleaseReport {
            attempted: u32::try_from(held).unwrap_or(u32::MAX),
            released: u32::try_from(held).unwrap_or(u32::MAX),
            failures: Vec::new(),
            neutral: true,
        };
        let report = self.release_reports.pop_front().unwrap_or(default_report);
        if report.neutral {
            self.held_buttons.clear();
            self.held_keys.clear();
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BackendCapabilities, BackendErrorCategory, MAX_ABSOLUTE_FIXED_DELTA, MAX_TEXT_BYTES,
        PermissionState, PointerDelta, RetryScope, ScrollAxis, ScrollDelta, SensitiveText,
        ValueError,
    };
    use tether_test_support::{
        FakeClock, SafeTraceEvent, SanitizedTraceCapture, TraceCode, TraceComponent,
    };

    fn key() -> PhysicalKey {
        PhysicalKey::new(0x04).expect("standard HID usage is accepted")
    }

    #[test]
    fn fixed_point_values_enforce_hard_bounds() {
        assert!(PointerDelta::new(MAX_ABSOLUTE_FIXED_DELTA, i32::MIN).is_err());
        assert_eq!(
            ScrollDelta::new(ScrollAxis::Vertical, MAX_ABSOLUTE_FIXED_DELTA + 1),
            Err(ValueError::DeltaOutOfRange)
        );
        let pointer = PointerDelta::new(-17, 23).expect("bounded delta");
        assert_eq!((pointer.x(), pointer.y()), (-17, 23));
    }

    #[test]
    fn physical_key_allowlist_excludes_reserved_and_power_usages() {
        assert!(PhysicalKey::new(0x04).is_ok());
        assert!(PhysicalKey::new(0xE7).expect("modifier").is_modifier());
        assert_eq!(PhysicalKey::new(0x66), Err(ValueError::InvalidPhysicalKey));
        assert_eq!(PhysicalKey::new(0xE8), Err(ValueError::InvalidPhysicalKey));
    }

    #[test]
    fn text_is_bounded_and_all_standard_formatting_is_redacted() {
        let secret = "never-log-me";
        let text = SensitiveText::new(secret.to_owned()).expect("valid text");
        assert_eq!(text.byte_len(), secret.len());
        assert_eq!(text.expose_for_injection(), secret);
        assert!(!format!("{text:?}").contains(secret));
        assert!(!text.to_string().contains(secret));
        assert_eq!(
            SensitiveText::new(String::new()),
            Err(ValueError::EmptyText)
        );
        assert_eq!(
            SensitiveText::new("x".repeat(MAX_TEXT_BYTES + 1)),
            Err(ValueError::TextTooLong)
        );
    }

    #[test]
    fn actions_redact_key_button_text_and_delta_payloads() {
        let actions = [
            InputAction::PhysicalKey(KeyAction::Down(key())),
            InputAction::MouseButton(ButtonAction::Down(MouseButton::Forward)),
            InputAction::TextInsert(
                SensitiveText::new("unique-secret".to_owned()).expect("valid text"),
            ),
            InputAction::PointerMove(PointerDelta::new(12_345, -54_321).expect("bounded")),
        ];
        let rendered = format!("{actions:?}");
        for forbidden in ["unique-secret", "Forward", "12345", "54321", "0x04"] {
            assert!(!rendered.contains(forbidden), "leaked {forbidden}");
        }
    }

    #[test]
    fn sanitized_trace_type_cannot_receive_action_payloads() {
        let trace = SanitizedTraceCapture::new(1);
        assert!(trace.record(SafeTraceEvent {
            component: TraceComponent::Input,
            code: TraceCode::Started,
            metric: None,
        }));
        let text = SensitiveText::new("not-in-trace".to_owned()).expect("valid text");
        let diagnostic = format!("{trace:?} {text:?}");
        assert!(!diagnostic.contains("not-in-trace"));
    }

    #[test]
    fn capability_defaults_are_fail_closed() {
        let capabilities = BackendCapabilities::default();
        assert!(!capabilities.relative_pointer.is_available());
        assert!(capabilities.mouse_buttons.is_empty());
        assert!(capabilities.text_modes.is_empty());
        let mock = MockInputBackend::new();
        assert!(!mock.status().available);
        assert!(mock.is_neutral());
    }

    #[test]
    fn status_revision_changes_only_when_staged_content_changes() {
        let mut mock = MockInputBackend::new();
        let same = mock.status();
        mock.stage_status(same);
        assert_eq!(
            mock.refresh_status(StatusRefreshReason::UserRequest)
                .revision,
            0
        );

        let mut available = mock.status();
        available.available = true;
        available.initialized = true;
        available.permission = PermissionState::Granted;
        mock.stage_status(available);
        assert_eq!(
            mock.refresh_status(StatusRefreshReason::PermissionNotification)
                .revision,
            1
        );
    }

    #[test]
    fn mock_records_only_sanitized_classes_and_injected_errors() {
        let mut mock = MockInputBackend::new();
        let error = BackendError::new(
            BackendErrorCategory::NativeCallFailed,
            OperationClass::TextInsert,
            RetryScope::AfterStatusRefresh,
        );
        mock.push_execute_result(Err(error));
        let action = InputAction::TextInsert(
            SensitiveText::new("mock-secret".to_owned()).expect("valid text"),
        );
        assert_eq!(mock.execute(&action), Err(error));
        assert_eq!(
            mock.calls(),
            &[MockCall {
                sequence: 0,
                operation: OperationClass::TextInsert,
                outcome: MockOutcome::Failed(BackendErrorCategory::NativeCallFailed),
            }]
        );
        assert!(!format!("{mock:?} {:?}", mock.calls()).contains("mock-secret"));
    }

    #[test]
    fn successful_transitions_update_mirror_and_neutral_release_is_idempotent() {
        let mut mock = MockInputBackend::new();
        mock.execute(&InputAction::PhysicalKey(KeyAction::Down(key())))
            .expect("key down succeeds");
        mock.execute(&InputAction::MouseButton(ButtonAction::Down(
            MouseButton::Left,
        )))
        .expect("button down succeeds");
        assert_eq!(mock.held_counts(), (1, 1));

        let report = mock.release_all();
        assert_eq!((report.attempted, report.released), (2, 2));
        assert!(report.neutral);
        assert!(mock.is_neutral());
        assert_eq!(mock.release_all(), ReleaseReport::neutral());
    }

    #[test]
    fn partial_release_keeps_mirror_non_neutral_and_latency_is_deterministic() {
        let mut mock = MockInputBackend::new();
        let clock = FakeClock::default();
        let observed_clock = clock.clone();
        mock.set_time_advance(move |duration| {
            observed_clock
                .advance(duration)
                .expect("test clock can represent mock latency");
        });
        mock.set_simulated_latency(Duration::from_millis(4));
        mock.execute(&InputAction::PhysicalKey(KeyAction::Down(key())))
            .expect("key down succeeds");
        mock.push_release_report(ReleaseReport {
            attempted: 1,
            released: 0,
            failures: vec![crate::ReleaseFailure::NonModifierKey],
            neutral: false,
        });
        assert!(!mock.release_all().neutral);
        assert!(!mock.is_neutral());
        assert_eq!(mock.simulated_elapsed(), Duration::from_millis(4));
        assert_eq!(clock.now(), Duration::from_millis(4));
    }
}
