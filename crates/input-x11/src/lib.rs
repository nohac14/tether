//! X11 XTEST/XKB translation spike.
//!
//! The mapper consumes a capability/keymap snapshot and emits inert XTEST
//! plans. It does not connect to an X server or inject input in normal tests.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use tether_input_core::{
    BackendCapabilities, BackendError, BackendErrorCategory, BackendKind, BackendStatus,
    ButtonAction, CapabilityState, ClickCount, DesktopShortcutPlatform, FIXED_POINT_SCALE,
    InputAction, KeyAction, LOGICAL_COMMANDS, MediaCommand, MouseButton, OperationClass,
    PermissionState, PhysicalKey, RetryScope, ScrollAxis, StatusRefreshReason, TextEditCommand,
    TextMode, canonical_logical_chord,
};

const MAX_SCROLL_STEPS_PER_ACTION: i32 = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct X11TextStroke {
    pub keycode: u8,
    pub shift: bool,
}

/// Bounded extension and active-layout snapshot produced by future X11 setup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct X11Probe {
    pub connected: bool,
    pub xtest_available: bool,
    pub xkb_available: bool,
    pub physical_keycodes: BTreeMap<u8, u8>,
    pub text_keycodes: BTreeMap<char, X11TextStroke>,
    pub media_keycodes: BTreeMap<MediaCommand, u8>,
}

impl X11Probe {
    #[must_use]
    pub fn inert_ready() -> Self {
        let physical_keycodes = reviewed_us_keycodes().into_iter().collect();
        let text_keycodes = reviewed_ascii_text_map();
        let media_keycodes = BTreeMap::from([
            (MediaCommand::PlayPause, 172),
            (MediaCommand::NextTrack, 171),
            (MediaCommand::PreviousTrack, 173),
            (MediaCommand::VolumeUp, 123),
            (MediaCommand::VolumeDown, 122),
            (MediaCommand::Mute, 121),
        ]);
        Self {
            connected: true,
            xtest_available: true,
            xkb_available: true,
            physical_keycodes,
            text_keycodes,
            media_keycodes,
        }
    }
}

/// Pure description of one future XTEST event.
#[derive(Clone, Eq, PartialEq)]
pub enum X11Event {
    RelativeMotion { dx: i16, dy: i16 },
    Button { button: u8, down: bool },
    Key { keycode: u8, down: bool },
}

impl fmt::Debug for X11Event {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("X11Event(<redacted>)")
    }
}

/// Pure X11 translator with per-axis fractional wheel accumulation.
pub struct X11Mapper {
    status: BackendStatus,
    physical_keycodes: BTreeMap<u8, u8>,
    text_keycodes: BTreeMap<char, X11TextStroke>,
    media_keycodes: BTreeMap<MediaCommand, u8>,
    vertical_remainder: i32,
    horizontal_remainder: i32,
}

impl X11Mapper {
    #[must_use]
    pub fn from_probe(probe: X11Probe) -> Self {
        let available = probe.connected && probe.xtest_available;
        let keyboard_available =
            available && probe.xkb_available && !probe.physical_keycodes.is_empty();
        let logical_commands = if keyboard_available {
            LOGICAL_COMMANDS
                .into_iter()
                .filter(|command| {
                    canonical_logical_chord(DesktopShortcutPlatform::Linux, *command).is_some()
                })
                .collect()
        } else {
            BTreeSet::new()
        };
        let media_commands = if available {
            probe.media_keycodes.keys().copied().collect()
        } else {
            BTreeSet::new()
        };
        let capabilities = if available {
            BackendCapabilities {
                relative_pointer: CapabilityState::Available,
                mouse_buttons: all_mouse_buttons(),
                atomic_click: CapabilityState::Available,
                vertical_scroll: CapabilityState::Available,
                horizontal_scroll: CapabilityState::Available,
                // An incomplete active keymap remains a partially capable
                // keyboard; individual misses are explicit (F-08/D-04).
                physical_keys: if keyboard_available {
                    CapabilityState::Available
                } else {
                    CapabilityState::Unavailable
                },
                text_modes: if keyboard_available && !probe.text_keycodes.is_empty() {
                    BTreeSet::from([TextMode::ActiveLayoutMappable])
                } else {
                    BTreeSet::new()
                },
                logical_commands,
                media_commands,
            }
        } else {
            BackendCapabilities::unavailable()
        };
        Self {
            status: BackendStatus {
                revision: 1,
                kind: BackendKind::X11,
                initialized: probe.connected,
                available,
                permission: PermissionState::NotRequired,
                capabilities,
                fault: None,
            },
            physical_keycodes: probe.physical_keycodes,
            text_keycodes: probe.text_keycodes,
            media_keycodes: probe.media_keycodes,
            vertical_remainder: 0,
            horizontal_remainder: 0,
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

    /// Translates one action without touching an X connection.
    ///
    /// # Errors
    ///
    /// Returns a safe error before emission for an unmappable key/text scalar,
    /// an out-of-range XTEST motion, or an excessive discrete scroll batch.
    pub fn map(&mut self, action: &InputAction) -> Result<Vec<X11Event>, BackendError> {
        match action {
            InputAction::PointerMove(delta) => {
                let dx = i16::try_from(delta.x()).map_err(|_| invalid_state(action.class()))?;
                let dy = i16::try_from(delta.y()).map_err(|_| invalid_state(action.class()))?;
                Ok(vec![X11Event::RelativeMotion { dx, dy }])
            }
            InputAction::Scroll(delta) => self.map_scroll(delta.axis(), delta.amount()),
            InputAction::MouseButton(action) => Ok(map_button(*action)),
            InputAction::PhysicalKey(action) => self.map_key(*action).map(|event| vec![event]),
            InputAction::TextInsert(text) => self.map_text(text.expose_for_injection()),
            InputAction::TextEdit(command) => {
                let key = text_edit_key(*command);
                Ok(vec![
                    self.map_key(KeyAction::Down(key))?,
                    self.map_key(KeyAction::Up(key))?,
                ])
            }
            InputAction::LogicalCommand(command) => self.map_logical(*command),
            InputAction::MediaCommand(command) => self.map_media(*command),
        }
    }

    fn map_key(&self, action: KeyAction) -> Result<X11Event, BackendError> {
        let (key, down) = match action {
            KeyAction::Down(key) => (key, true),
            KeyAction::Up(key) => (key, false),
        };
        let Some(keycode) = self.physical_keycodes.get(&key.usage()).copied() else {
            return Err(unsupported(OperationClass::PhysicalKey));
        };
        Ok(X11Event::Key { keycode, down })
    }

    fn map_scroll(&mut self, axis: ScrollAxis, amount: i32) -> Result<Vec<X11Event>, BackendError> {
        let remainder = match axis {
            ScrollAxis::Vertical => self.vertical_remainder,
            ScrollAxis::Horizontal => self.horizontal_remainder,
        };
        let total = remainder.saturating_add(amount);
        let steps = total / FIXED_POINT_SCALE;
        if steps.unsigned_abs() > MAX_SCROLL_STEPS_PER_ACTION as u32 {
            return Err(invalid_state(OperationClass::Scroll));
        }
        let new_remainder = total % FIXED_POINT_SCALE;
        match axis {
            ScrollAxis::Vertical => self.vertical_remainder = new_remainder,
            ScrollAxis::Horizontal => self.horizontal_remainder = new_remainder,
        }
        let button = match (axis, steps.signum()) {
            (ScrollAxis::Vertical, 1) => 4,
            (ScrollAxis::Vertical, -1) => 5,
            (ScrollAxis::Horizontal, -1) => 6,
            (ScrollAxis::Horizontal, 1) => 7,
            (_, _) => return Ok(Vec::new()),
        };
        let mut events = Vec::with_capacity(steps.unsigned_abs() as usize * 2);
        for _ in 0..steps.unsigned_abs() {
            events.push(X11Event::Button { button, down: true });
            events.push(X11Event::Button {
                button,
                down: false,
            });
        }
        Ok(events)
    }

    fn map_text(&self, text: &str) -> Result<Vec<X11Event>, BackendError> {
        let strokes = text
            .chars()
            .map(|character| self.text_keycodes.get(&character).copied())
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| unsupported(OperationClass::TextInsert))?;
        let shift_keycode = self.physical_keycodes.get(&0xE1).copied();
        if strokes.iter().any(|stroke| stroke.shift) && shift_keycode.is_none() {
            return Err(unsupported(OperationClass::TextInsert));
        }
        let mut events = Vec::with_capacity(strokes.len().saturating_mul(4));
        for stroke in strokes {
            if stroke.shift {
                events.push(X11Event::Key {
                    keycode: shift_keycode.unwrap_or_else(|| unreachable!("preflighted shift")),
                    down: true,
                });
            }
            events.push(X11Event::Key {
                keycode: stroke.keycode,
                down: true,
            });
            events.push(X11Event::Key {
                keycode: stroke.keycode,
                down: false,
            });
            if stroke.shift {
                events.push(X11Event::Key {
                    keycode: shift_keycode.unwrap_or_else(|| unreachable!("preflighted shift")),
                    down: false,
                });
            }
        }
        Ok(events)
    }

    fn map_logical(
        &self,
        command: tether_input_core::LogicalCommand,
    ) -> Result<Vec<X11Event>, BackendError> {
        let Some(chord) = canonical_logical_chord(DesktopShortcutPlatform::Linux, command) else {
            return Err(unsupported(OperationClass::LogicalCommand));
        };
        let mut events = Vec::with_capacity(chord.len().saturating_mul(2));
        for key in chord.press_order() {
            events.push(self.map_key(KeyAction::Down(key))?);
        }
        for key in chord.release_order() {
            events.push(self.map_key(KeyAction::Up(key))?);
        }
        Ok(events)
    }

    fn map_media(&self, command: MediaCommand) -> Result<Vec<X11Event>, BackendError> {
        let Some(keycode) = self.media_keycodes.get(&command).copied() else {
            return Err(unsupported(OperationClass::MediaCommand));
        };
        Ok(vec![
            X11Event::Key {
                keycode,
                down: true,
            },
            X11Event::Key {
                keycode,
                down: false,
            },
        ])
    }

    /// Clears scroll residue during release-all and session transitions.
    pub fn clear_fractional_scroll(&mut self) {
        self.vertical_remainder = 0;
        self.horizontal_remainder = 0;
    }
}

fn unsupported(operation: OperationClass) -> BackendError {
    BackendError::new(
        BackendErrorCategory::Unsupported,
        operation,
        RetryScope::Never,
    )
}

fn invalid_state(operation: OperationClass) -> BackendError {
    BackendError::new(
        BackendErrorCategory::InvalidPlatformState,
        operation,
        RetryScope::AfterStatusRefresh,
    )
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

const fn pointer_button(button: MouseButton) -> u8 {
    match button {
        MouseButton::Left => 1,
        MouseButton::Middle => 2,
        MouseButton::Right => 3,
        MouseButton::Back => 8,
        MouseButton::Forward => 9,
    }
}

fn map_button(action: ButtonAction) -> Vec<X11Event> {
    match action {
        ButtonAction::Down(button) => vec![X11Event::Button {
            button: pointer_button(button),
            down: true,
        }],
        ButtonAction::Up(button) => vec![X11Event::Button {
            button: pointer_button(button),
            down: false,
        }],
        ButtonAction::Click { button, count } => {
            let repeats = match count {
                ClickCount::Single => 1,
                ClickCount::Double => 2,
            };
            let mut events = Vec::with_capacity(repeats * 2);
            for _ in 0..repeats {
                events.push(X11Event::Button {
                    button: pointer_button(button),
                    down: true,
                });
                events.push(X11Event::Button {
                    button: pointer_button(button),
                    down: false,
                });
            }
            events
        }
    }
}

fn text_edit_key(command: TextEditCommand) -> PhysicalKey {
    let usage = match command {
        TextEditCommand::DeleteBackward => 0x2A,
        TextEditCommand::DeleteForward => 0x4C,
        TextEditCommand::InsertLineBreak => 0x28,
    };
    PhysicalKey::new(usage).unwrap_or_else(|_| unreachable!("reviewed HID usage"))
}

fn reviewed_us_keycodes() -> BTreeMap<u8, u8> {
    BTreeMap::from([
        (0x04, 38),
        (0x05, 56),
        (0x06, 54),
        (0x07, 40),
        (0x08, 26),
        (0x09, 41),
        (0x0A, 42),
        (0x0B, 43),
        (0x0C, 31),
        (0x0D, 44),
        (0x0E, 45),
        (0x0F, 46),
        (0x10, 58),
        (0x11, 57),
        (0x12, 32),
        (0x13, 33),
        (0x14, 24),
        (0x15, 27),
        (0x16, 39),
        (0x17, 28),
        (0x18, 30),
        (0x19, 55),
        (0x1A, 25),
        (0x1B, 53),
        (0x1C, 29),
        (0x1D, 52),
        (0x28, 36),
        (0x29, 9),
        (0x2A, 22),
        (0x2B, 23),
        (0x2C, 65),
        (0x2D, 20),
        (0x4C, 119),
        (0xE0, 37),
        (0xE1, 50),
        (0xE2, 64),
        (0xE3, 133),
        (0xE4, 105),
        (0xE5, 62),
        (0xE6, 108),
        (0xE7, 134),
    ])
}

fn reviewed_ascii_text_map() -> BTreeMap<char, X11TextStroke> {
    let mut map = BTreeMap::new();
    for (usage, keycode) in reviewed_us_keycodes() {
        if (0x04..=0x1D).contains(&usage) {
            let lower = char::from(b'a' + (usage - 0x04));
            map.insert(
                lower,
                X11TextStroke {
                    keycode,
                    shift: false,
                },
            );
            map.insert(
                lower.to_ascii_uppercase(),
                X11TextStroke {
                    keycode,
                    shift: true,
                },
            );
        }
    }
    map.insert(
        '-',
        X11TextStroke {
            keycode: 20,
            shift: false,
        },
    );
    map
}

#[cfg(target_os = "linux")]
mod binding_contract {
    use x11rb::protocol::{xkb, xtest};

    #[allow(dead_code)]
    fn api_surface_compiles() {
        let _ = xtest::get_version::<x11rb::rust_connection::RustConnection>;
        let _ = xtest::fake_input::<x11rb::rust_connection::RustConnection>;
        let _ = xkb::use_extension::<x11rb::rust_connection::RustConnection>;
        let _ = xkb::get_state::<x11rb::rust_connection::RustConnection>;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tether_input_core::{
        InertConformanceBackend, InertConformanceMapper, InertConformanceSink, InertSinkError,
        InputBackend, ReleaseFailure, ScrollDelta, SensitiveText, run_backend_conformance,
    };

    #[derive(Default)]
    struct RecordingSink {
        events: Vec<X11Event>,
        fail_next_after: Option<usize>,
    }

    impl InertConformanceSink<X11Event> for RecordingSink {
        fn emit(&mut self, events: &[X11Event]) -> Result<(), InertSinkError> {
            if let Some(accepted) = self.fail_next_after.take() {
                let accepted = accepted.min(events.len());
                self.events.extend_from_slice(&events[..accepted]);
                return Err(InertSinkError::new(accepted));
            }
            self.events.extend_from_slice(events);
            Ok(())
        }
    }

    impl InertConformanceMapper for X11Mapper {
        type Event = X11Event;
        fn status(&self) -> BackendStatus {
            self.status()
        }
        fn refresh_status(&mut self, reason: StatusRefreshReason) -> BackendStatus {
            self.refresh_status(reason)
        }
        fn map(&mut self, action: &InputAction) -> Result<Vec<Self::Event>, BackendError> {
            self.map(action)
        }
        fn reset_local_state(&mut self) -> Result<(), ReleaseFailure> {
            self.clear_fractional_scroll();
            Ok(())
        }
    }

    fn backend() -> InertConformanceBackend<X11Mapper, RecordingSink> {
        InertConformanceBackend::new(
            X11Mapper::from_probe(X11Probe::inert_ready()),
            RecordingSink::default(),
        )
    }

    #[test]
    fn inert_mapper_passes_shared_conformance() {
        let mut backend = backend();
        let report =
            run_backend_conformance(&mut backend, BackendKind::X11).expect("X11 inert conformance");
        assert!(report.checked_actions > 200);
        assert!(report.explicitly_unsupported_mappings > 0);
        assert_eq!(backend.held_counts(), (0, 0));
    }

    #[test]
    fn fractional_axes_accumulate_independently_into_balanced_buttons() {
        let mut mapper = X11Mapper::from_probe(X11Probe::inert_ready());
        let vertical = |amount| {
            InputAction::Scroll(
                ScrollDelta::new(ScrollAxis::Vertical, amount).expect("valid delta"),
            )
        };
        let horizontal = |amount| {
            InputAction::Scroll(
                ScrollDelta::new(ScrollAxis::Horizontal, amount).expect("valid delta"),
            )
        };
        assert_eq!(mapper.map(&vertical(600)).expect("map").len(), 0);
        assert_eq!(mapper.map(&horizontal(-600)).expect("map").len(), 0);
        assert_eq!(
            mapper.map(&vertical(500)).expect("map"),
            vec![
                X11Event::Button {
                    button: 4,
                    down: true
                },
                X11Event::Button {
                    button: 4,
                    down: false
                },
            ]
        );
        assert_eq!(
            mapper.map(&horizontal(-500)).expect("map"),
            vec![
                X11Event::Button {
                    button: 6,
                    down: true
                },
                X11Event::Button {
                    button: 6,
                    down: false
                },
            ]
        );
    }

    #[test]
    fn text_preflight_rejects_whole_unmappable_value() {
        let mut backend = backend();
        let text = SensitiveText::new("ok✓".to_owned()).expect("fixture");
        let error = backend
            .execute(&InputAction::TextInsert(text))
            .expect_err("unsupported");
        assert_eq!(error.category(), BackendErrorCategory::Unsupported);
        assert_eq!(backend.sink().events.len(), 0);
    }

    #[test]
    fn partial_keyboard_capability_and_release_errors_are_explicit() {
        let mut backend = backend();
        assert!(backend.status().capabilities.physical_keys.is_available());
        let key = PhysicalKey::new(0xB0).expect("allowlisted fixture");
        let error = backend
            .execute(&InputAction::PhysicalKey(KeyAction::Down(key)))
            .expect_err("unsupported mapping");
        assert_eq!(error.category(), BackendErrorCategory::Unsupported);

        let held = PhysicalKey::new(0x04).expect("mapped key");
        backend
            .execute(&InputAction::PhysicalKey(KeyAction::Down(held)))
            .expect("down");
        backend.sink_mut().fail_next_after = Some(0);
        assert!(!backend.release_all().is_confirmed_neutral());
    }

    #[test]
    fn connection_or_xtest_absence_fails_closed() {
        let mut probe = X11Probe::inert_ready();
        probe.connected = false;
        let disconnected = X11Mapper::from_probe(probe);
        assert!(!disconnected.status().available);
        assert!(disconnected.status().capabilities.is_empty());

        let mut probe = X11Probe::inert_ready();
        probe.xtest_available = false;
        let no_xtest = X11Mapper::from_probe(probe);
        assert!(!no_xtest.status().available);
        assert!(no_xtest.status().capabilities.is_empty());
    }

    #[test]
    fn release_clears_fractional_scroll_residue() {
        let mut backend = backend();
        let half = InputAction::Scroll(
            ScrollDelta::new(ScrollAxis::Vertical, FIXED_POINT_SCALE / 2).expect("delta"),
        );
        backend.execute(&half).expect("first half");
        assert!(backend.release_all().is_confirmed_neutral());
        backend.execute(&half).expect("second half after reset");
        assert_eq!(backend.sink().events.len(), 0);
    }
}
