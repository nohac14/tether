use super::*;
use tether_input_core::{ButtonAction, MouseButton, SensitiveText};

struct FakeSink {
    active: bool,
    limit: Option<usize>,
    plans: Vec<Vec<WindowsEvent>>,
}

impl Sink for FakeSink {
    fn active(&self) -> bool {
        self.active
    }

    fn insert(&mut self, events: &[WindowsEvent]) -> usize {
        self.plans.push(events.to_vec());
        self.limit.take().unwrap_or(events.len())
    }
}

fn backend() -> WindowsBackend<FakeSink> {
    WindowsBackend::with_sink(FakeSink {
        active: true,
        limit: None,
        plans: Vec::new(),
    })
}

#[test]
fn partial_unicode_submission_is_faulted_and_conservatively_released() {
    let mut backend = backend();
    backend.sink.limit = Some(1);
    let text = InputAction::TextInsert(SensitiveText::new("🐾".to_owned()).unwrap());
    assert_eq!(
        backend.execute(&text).unwrap_err().category(),
        Error::PartialStateChange
    );
    let release = backend.release_all();
    assert!(release.is_confirmed_neutral());
    assert_eq!(release.attempted, 2);
    assert!(!backend.status().available);
    assert!(backend.release_all().is_confirmed_neutral());
}

#[test]
fn lock_with_held_input_latches_uncertainty_until_ups_succeed() {
    let mut backend = backend();
    backend
        .execute(&InputAction::MouseButton(ButtonAction::Down(
            MouseButton::Left,
        )))
        .unwrap();
    backend.sink.active = false;
    let report = backend.release_all();
    assert_eq!(report.failures, [ReleaseFailure::MouseButton]);
    assert!(!report.is_confirmed_neutral());
    assert_eq!(backend.sink.plans.len(), 1);
    backend.sink.active = true;
    assert!(backend.release_all().is_confirmed_neutral());
    assert!(!backend.status().available);
}

#[test]
fn click_cannot_destroy_a_held_drag_and_release_is_idempotent() {
    let mut backend = backend();
    backend
        .execute(&InputAction::MouseButton(ButtonAction::Down(
            MouseButton::Left,
        )))
        .unwrap();
    let click = InputAction::MouseButton(ButtonAction::click(MouseButton::Left, 1).unwrap());
    assert_eq!(
        backend.execute(&click).unwrap_err().category(),
        Error::Unsupported
    );
    assert_eq!(backend.sink.plans.len(), 1);
    let release = backend.release_all();
    assert_eq!(release.attempted, 1);
    assert!(release.is_confirmed_neutral());
    assert_eq!(backend.release_all(), ReleaseReport::neutral());
}
