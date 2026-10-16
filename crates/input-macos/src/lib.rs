//! macOS Quartz event translation spike.
//!
//! This crate builds inert Core Graphics event plans and performs a bounded
//! Accessibility preflight on macOS. It never posts native events; live event
//! creation/posting belongs to MAC-002 and guarded opt-in verification.

use std::collections::BTreeSet;
use std::fmt;
use tether_input_core::{
    BackendCapabilities, BackendError, BackendErrorCategory, BackendKind, BackendStatus,
    ButtonAction, CapabilityState, ClickCount, DesktopShortcutPlatform, InputAction, KeyAction,
    LOGICAL_COMMANDS, MediaCommand, MouseButton, OperationClass, PermissionState, PhysicalKey,
    RetryScope, ScrollAxis, StatusRefreshReason, TextEditCommand, TextMode,
    canonical_logical_chord,
};

/// Result of bounded, non-injecting macOS environment inspection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MacProbe {
    pub core_graphics_available: bool,
    pub accessibility: PermissionState,
}

impl MacProbe {
    #[must_use]
    pub const fn inert_ready() -> Self {
        Self {
            core_graphics_available: true,
            accessibility: PermissionState::Granted,
        }
    }

    /// Performs the non-prompting Core Graphics Accessibility preflight.
    #[must_use]
    pub fn current() -> Self {
        platform_probe()
    }
}

#[cfg(target_os = "macos")]
fn platform_probe() -> MacProbe {
    use objc2_core_graphics::CGPreflightPostEventAccess;

    MacProbe {
        core_graphics_available: true,
        accessibility: if CGPreflightPostEventAccess() {
            PermissionState::Granted
        } else {
            // Preflight intentionally does not prompt and cannot distinguish a
            // first-run prompt from a denial. The UI coordinator owns prompts.
            PermissionState::PromptRequired
        },
    }
}

#[cfg(not(target_os = "macos"))]
const fn platform_probe() -> MacProbe {
    MacProbe {
        core_graphics_available: false,
        accessibility: PermissionState::Unavailable,
    }
}

/// Pure description of one future Quartz event operation.
#[derive(Clone, Eq, PartialEq)]
pub enum MacEvent {
    RelativeMotion {
        dx: i32,
        dy: i32,
    },
    MouseButton {
        button_number: u8,
        down: bool,
        click_state: u8,
    },
    Scroll {
        horizontal: bool,
        delta: i32,
        continuous: bool,
    },
    KeyCode {
        key_code: u16,
        down: bool,
    },
    Unicode {
        utf16: Vec<u16>,
        down: bool,
    },
    AuxiliaryKey {
        key_type: u16,
        down: bool,
    },
}

impl fmt::Debug for MacEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("MacEvent(<redacted>)")
    }
}

/// Pure macOS action translator shared by inert and future live sinks.
pub struct MacMapper {
    status: BackendStatus,
}

impl MacMapper {
    #[must_use]
    pub fn from_probe(probe: MacProbe) -> Self {
        let available = probe.core_graphics_available
            && matches!(probe.accessibility, PermissionState::Granted);
        let capabilities = if available {
            BackendCapabilities {
                relative_pointer: CapabilityState::Available,
                mouse_buttons: all_mouse_buttons(),
                atomic_click: CapabilityState::Available,
                vertical_scroll: CapabilityState::Available,
                horizontal_scroll: CapabilityState::Available,
                // General keyboard availability is independent of individual
                // HID usages that `map_key` may reject (F-08).
                physical_keys: CapabilityState::Available,
                text_modes: BTreeSet::from([TextMode::NativeUnicodeBestEffort]),
                logical_commands: BTreeSet::from(LOGICAL_COMMANDS),
                media_commands: all_media_commands(),
            }
        } else {
            BackendCapabilities::unavailable()
        };
        Self {
            status: BackendStatus {
                revision: 1,
                kind: BackendKind::MacOs,
                initialized: probe.core_graphics_available,
                available,
                permission: probe.accessibility,
                capabilities,
                fault: None,
            },
        }
    }

    #[must_use]
    pub fn status(&self) -> BackendStatus {
        self.status.clone()
    }

    #[must_use]
    pub fn refresh_status(&mut self, _reason: StatusRefreshReason) -> BackendStatus {
        self.status()
    }

    /// Translates one action without creating or posting a Quartz event.
    ///
    /// # Errors
    ///
    /// Returns `Unsupported` before producing any event when an allowlisted HID
    /// usage lacks a reviewed macOS virtual-key mapping.
    pub fn map(&mut self, action: &InputAction) -> Result<Vec<MacEvent>, BackendError> {
        match action {
            InputAction::PointerMove(delta) => Ok(vec![MacEvent::RelativeMotion {
                dx: delta.x(),
                dy: delta.y(),
            }]),
            InputAction::Scroll(delta) => Ok(vec![MacEvent::Scroll {
                horizontal: delta.axis() == ScrollAxis::Horizontal,
                delta: delta.amount(),
                continuous: true,
            }]),
            InputAction::MouseButton(action) => Ok(map_button(*action)),
            InputAction::PhysicalKey(action) => map_key(*action).map(|event| vec![event]),
            InputAction::TextInsert(text) => {
                let utf16 = text
                    .expose_for_injection()
                    .encode_utf16()
                    .collect::<Vec<_>>();
                Ok(vec![
                    MacEvent::Unicode {
                        utf16: utf16.clone(),
                        down: true,
                    },
                    MacEvent::Unicode { utf16, down: false },
                ])
            }
            InputAction::TextEdit(command) => {
                let key = text_edit_key(*command);
                Ok(vec![
                    map_key(KeyAction::Down(key))?,
                    map_key(KeyAction::Up(key))?,
                ])
            }
            InputAction::LogicalCommand(command) => map_logical(*command),
            InputAction::MediaCommand(command) => Ok(map_media(*command)),
        }
    }
}

fn all_mouse_buttons() -> BTreeSet<MouseButton> {
    BTreeSet::from([
        MouseButton::Left,
        MouseButton::Right,
        MouseButton::Middle,
        MouseButton::Back,
        MouseButton::Forward,
    ])
}

fn all_media_commands() -> BTreeSet<MediaCommand> {
    BTreeSet::from([
        MediaCommand::PlayPause,
        MediaCommand::NextTrack,
        MediaCommand::PreviousTrack,
        MediaCommand::VolumeUp,
        MediaCommand::VolumeDown,
        MediaCommand::Mute,
    ])
}

fn unsupported(operation: OperationClass) -> BackendError {
    BackendError::new(
        BackendErrorCategory::Unsupported,
        operation,
        RetryScope::Never,
    )
}

const fn button_number(button: MouseButton) -> u8 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        MouseButton::Middle => 2,
        MouseButton::Back => 3,
        MouseButton::Forward => 4,
    }
}

fn map_button(action: ButtonAction) -> Vec<MacEvent> {
    match action {
        ButtonAction::Down(button) => vec![MacEvent::MouseButton {
            button_number: button_number(button),
            down: true,
            click_state: 1,
        }],
        ButtonAction::Up(button) => vec![MacEvent::MouseButton {
            button_number: button_number(button),
            down: false,
            click_state: 1,
        }],
        ButtonAction::Click { button, count } => {
            let repeats = match count {
                ClickCount::Single => 1,
                ClickCount::Double => 2,
            };
            let mut events = Vec::with_capacity(repeats * 2);
            for click_state in 1..=repeats {
                events.push(MacEvent::MouseButton {
                    button_number: button_number(button),
                    down: true,
                    click_state: u8::try_from(click_state).unwrap_or(2),
                });
                events.push(MacEvent::MouseButton {
                    button_number: button_number(button),
                    down: false,
                    click_state: u8::try_from(click_state).unwrap_or(2),
                });
            }
            events
        }
    }
}

fn map_key(action: KeyAction) -> Result<MacEvent, BackendError> {
    let (key, down) = match action {
        KeyAction::Down(key) => (key, true),
        KeyAction::Up(key) => (key, false),
    };
    let Some(key_code) = hid_to_key_code(key.usage()) else {
        return Err(unsupported(OperationClass::PhysicalKey));
    };
    Ok(MacEvent::KeyCode { key_code, down })
}

fn map_logical(command: tether_input_core::LogicalCommand) -> Result<Vec<MacEvent>, BackendError> {
    let Some(chord) = canonical_logical_chord(DesktopShortcutPlatform::MacOs, command) else {
        return Err(unsupported(OperationClass::LogicalCommand));
    };
    let mut events = Vec::with_capacity(chord.len().saturating_mul(2));
    for key in chord.press_order() {
        events.push(map_key(KeyAction::Down(key))?);
    }
    for key in chord.release_order() {
        events.push(map_key(KeyAction::Up(key))?);
    }
    Ok(events)
}

fn map_media(command: MediaCommand) -> Vec<MacEvent> {
    let key_type = match command {
        MediaCommand::PlayPause => 16,
        MediaCommand::NextTrack => 17,
        MediaCommand::PreviousTrack => 18,
        MediaCommand::VolumeUp => 0,
        MediaCommand::VolumeDown => 1,
        MediaCommand::Mute => 7,
    };
    vec![
        MacEvent::AuxiliaryKey {
            key_type,
            down: true,
        },
        MacEvent::AuxiliaryKey {
            key_type,
            down: false,
        },
    ]
}

fn text_edit_key(command: TextEditCommand) -> PhysicalKey {
    let usage = match command {
        TextEditCommand::DeleteBackward => 0x2A,
        TextEditCommand::DeleteForward => 0x4C,
        TextEditCommand::InsertLineBreak => 0x28,
    };
    PhysicalKey::new(usage).unwrap_or_else(|_| unreachable!("reviewed HID usage"))
}

#[rustfmt::skip]
fn hid_to_key_code(usage: u8) -> Option<u16> {
    Some(match usage {
        0x04 => 0, 0x05 => 11, 0x06 => 8, 0x07 => 2, 0x08 => 14,
        0x09 => 3, 0x0A => 5, 0x0B => 4, 0x0C => 34, 0x0D => 38,
        0x0E => 40, 0x0F => 37, 0x10 => 46, 0x11 => 45, 0x12 => 31,
        0x13 => 35, 0x14 => 12, 0x15 => 15, 0x16 => 1, 0x17 => 17,
        0x18 => 32, 0x19 => 9, 0x1A => 13, 0x1B => 7, 0x1C => 16,
        0x1D => 6, 0x1E => 18, 0x1F => 19, 0x20 => 20, 0x21 => 21,
        0x22 => 23, 0x23 => 22, 0x24 => 26, 0x25 => 28, 0x26 => 25,
        0x27 => 29, 0x28 => 36, 0x29 => 53, 0x2A => 51, 0x2B => 48,
        0x2C => 49, 0x2D => 27, 0x2E => 24, 0x2F => 33, 0x30 => 30,
        0x31 | 0x32 => 42, 0x33 => 41, 0x34 => 39, 0x35 => 50,
        0x36 => 43, 0x37 => 47, 0x38 => 44, 0x39 => 57,
        0x3A => 122, 0x3B => 120, 0x3C => 99, 0x3D => 118,
        0x3E => 96, 0x3F => 97, 0x40 => 98, 0x41 => 100,
        0x42 => 101, 0x43 => 109, 0x44 => 103, 0x45 => 111,
        0x4A => 115, 0x4B => 116, 0x4C => 117, 0x4D => 119,
        0x4E => 121, 0x4F => 124, 0x50 => 123, 0x51 => 125, 0x52 => 126,
        0x54 => 75, 0x55 => 67, 0x56 => 78, 0x57 => 69, 0x58 => 76,
        0x59 => 83, 0x5A => 84, 0x5B => 85, 0x5C => 86, 0x5D => 87,
        0x5E => 88, 0x5F => 89, 0x60 => 91, 0x61 => 92, 0x62 => 82,
        0x63 => 65, 0xE0 => 59, 0xE1 => 56, 0xE2 => 58, 0xE3 => 55,
        0xE4 => 62, 0xE5 => 60, 0xE6 => 61, 0xE7 => 54,
        _ => return None,
    })
}

#[cfg(target_os = "macos")]
mod binding_contract {
    use objc2_core_graphics::CGEvent;

    #[allow(dead_code)]
    fn api_surface_compiles() {
        let _ = CGEvent::new_keyboard_event;
        let _ = CGEvent::new_mouse_event;
        let _ = CGEvent::new_scroll_wheel_event2;
        let _ = CGEvent::keyboard_set_unicode_string;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tether_input_core::{
        InertConformanceBackend, InertConformanceMapper, InertConformanceSink, InertSinkError,
        InputBackend, SensitiveText, run_backend_conformance,
    };

    #[derive(Default)]
    struct RecordingSink {
        events: Vec<MacEvent>,
        fail_next_after: Option<usize>,
    }

    impl InertConformanceSink<MacEvent> for RecordingSink {
        fn emit(&mut self, events: &[MacEvent]) -> Result<(), InertSinkError> {
            if let Some(accepted) = self.fail_next_after.take() {
                let accepted = accepted.min(events.len());
                self.events.extend_from_slice(&events[..accepted]);
                return Err(InertSinkError::new(accepted));
            }
            self.events.extend_from_slice(events);
            Ok(())
        }
    }

    impl InertConformanceMapper for MacMapper {
        type Event = MacEvent;

        fn status(&self) -> BackendStatus {
            self.status()
        }
        fn refresh_status(&mut self, reason: StatusRefreshReason) -> BackendStatus {
            self.refresh_status(reason)
        }
        fn map(&mut self, action: &InputAction) -> Result<Vec<Self::Event>, BackendError> {
            self.map(action)
        }
    }

    fn backend() -> InertConformanceBackend<MacMapper, RecordingSink> {
        InertConformanceBackend::new(
            MacMapper::from_probe(MacProbe::inert_ready()),
            RecordingSink::default(),
        )
    }

    #[test]
    fn inert_mapper_passes_shared_conformance() {
        let mut backend = backend();
        let report = run_backend_conformance(&mut backend, BackendKind::MacOs)
            .expect("macOS inert conformance");
        assert!(report.checked_actions > 200);
        assert!(report.explicitly_unsupported_mappings > 0);
        assert_eq!(backend.held_counts(), (0, 0));
    }

    #[test]
    fn unicode_preserves_utf16_surrogate_order() {
        let mut mapper = MacMapper::from_probe(MacProbe::inert_ready());
        let text = SensitiveText::new("😀".to_owned()).expect("fixture");
        let events = mapper.map(&InputAction::TextInsert(text)).expect("mapping");
        assert_eq!(events.len(), 2);
        assert!(matches!(
            &events[0],
            MacEvent::Unicode { utf16, down: true } if utf16 == &[0xD83D, 0xDE00]
        ));
        assert!(matches!(
            &events[1],
            MacEvent::Unicode { utf16, down: false } if utf16 == &[0xD83D, 0xDE00]
        ));
    }

    #[test]
    fn unmappable_hid_does_not_hide_keyboard_capability() {
        let mut backend = backend();
        assert!(backend.status().capabilities.physical_keys.is_available());
        let key = PhysicalKey::new(0xB0).expect("allowlisted fixture");
        let error = backend
            .execute(&InputAction::PhysicalKey(KeyAction::Down(key)))
            .expect_err("unsupported mapping");
        assert_eq!(error.category(), BackendErrorCategory::Unsupported);
        assert_eq!(backend.sink().events.len(), 0);
        assert_eq!(backend.held_counts(), (0, 0));
    }

    #[test]
    fn held_cleanup_and_partial_sink_failure_are_explicit() {
        let mut held = backend();
        let key = PhysicalKey::new(0x04).expect("mapped fixture");
        held.execute(&InputAction::PhysicalKey(KeyAction::Down(key)))
            .expect("key down");
        assert!(held.release_all().is_confirmed_neutral());
        assert_eq!(held.held_counts(), (0, 0));

        let mut failing = backend();
        failing.sink_mut().fail_next_after = Some(1);
        let click = InputAction::MouseButton(
            ButtonAction::click(MouseButton::Left, 2).expect("double click"),
        );
        let error = failing.execute(&click).expect_err("partial sink failure");
        assert_eq!(error.category(), BackendErrorCategory::PartialStateChange);
        assert!(!failing.release_all().is_confirmed_neutral());
    }

    #[test]
    fn permission_probe_fails_closed() {
        let mapper = MacMapper::from_probe(MacProbe {
            core_graphics_available: true,
            accessibility: PermissionState::PromptRequired,
        });
        assert!(!mapper.status().available);
        assert!(mapper.status().capabilities.is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_accessibility_preflight_is_non_prompting_and_conservative() {
        let probe = MacProbe::current();
        assert!(probe.core_graphics_available);
        assert!(matches!(
            probe.accessibility,
            PermissionState::Granted | PermissionState::PromptRequired
        ));
    }
}
