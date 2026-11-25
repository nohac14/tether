//! Windows `SendInput` adapter and inert translation plans.

use std::collections::BTreeSet;
use std::fmt;

#[cfg(windows)]
mod live;
#[cfg(windows)]
mod native_records;
#[cfg(windows)]
pub use live::WindowsBackend;
use tether_input_core::{
    BackendCapabilities, BackendError, BackendErrorCategory, BackendKind, BackendStatus,
    ButtonAction, CapabilityState, ClickCount, DesktopShortcutPlatform, FIXED_POINT_SCALE,
    InputAction, KeyAction, LOGICAL_COMMANDS, MediaCommand, MouseButton, NativeUnitQuantum,
    OperationClass, PermissionState, PhysicalKey, PointerUnitTranslator, RetryScope, ScrollAxis,
    ScrollDelta, StatusRefreshReason, TextEditCommand, TextMode, canonical_logical_chord,
};

/// Win32 `WHEEL_DELTA`, used for both wheel axes.
const WHEEL_DELTA: i64 = 120;
const POINTER_QUANTUM: NativeUnitQuantum =
    match NativeUnitQuantum::new(FIXED_POINT_SCALE.unsigned_abs()) {
        Ok(quantum) => quantum,
        Err(_) => panic!("fixed-point scale must be positive"),
    };

/// Result of bounded, non-injecting Windows environment inspection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowsProbe {
    pub send_input_available: bool,
    pub interactive_desktop: bool,
}

impl WindowsProbe {
    #[must_use]
    pub const fn inert_ready() -> Self {
        Self {
            send_input_available: true,
            interactive_desktop: true,
        }
    }
}

/// Pure event description corresponding to one future `INPUT` record.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum WindowsEvent {
    RelativeMotion {
        dx: i32,
        dy: i32,
    },
    MouseButton {
        flag: u32,
        data: u32,
    },
    Wheel {
        horizontal: bool,
        delta: i32,
    },
    ScanCode {
        scan_code: u16,
        extended: bool,
        down: bool,
    },
    UnicodeCodeUnit {
        code_unit: u16,
        down: bool,
    },
    VirtualKey {
        virtual_key: u16,
        down: bool,
    },
}

impl fmt::Debug for WindowsEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("WindowsEvent(<redacted>)")
    }
}

/// Pure Windows action translator shared by inert and future live sinks.
pub struct WindowsMapper {
    status: BackendStatus,
    pointer: PointerUnitTranslator,
    horizontal_wheel_remainder: i64,
    vertical_wheel_remainder: i64,
}

impl WindowsMapper {
    #[must_use]
    pub fn from_probe(probe: WindowsProbe) -> Self {
        let available = probe.send_input_available && probe.interactive_desktop;
        let capabilities = if available {
            BackendCapabilities {
                relative_pointer: CapabilityState::Available,
                mouse_buttons: all_mouse_buttons(),
                atomic_click: CapabilityState::Available,
                vertical_scroll: CapabilityState::Available,
                horizontal_scroll: CapabilityState::Available,
                // F-08: this describes general keyboard availability. A
                // particular unmappable HID usage is rejected by `map_key`.
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
                kind: BackendKind::Windows,
                initialized: probe.send_input_available,
                available,
                permission: PermissionState::NotRequired,
                capabilities,
                fault: None,
            },
            pointer: PointerUnitTranslator::new(POINTER_QUANTUM, POINTER_QUANTUM),
            horizontal_wheel_remainder: 0,
            vertical_wheel_remainder: 0,
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

    /// Clears sub-unit motion and wheel deltas at a release or epoch boundary.
    pub fn reset_fractional_state(&mut self) {
        self.pointer.reset();
        self.horizontal_wheel_remainder = 0;
        self.vertical_wheel_remainder = 0;
    }

    /// Translates one platform-neutral action without invoking the OS.
    ///
    /// # Errors
    ///
    /// Returns `Unsupported` before producing any event when an allowlisted HID
    /// usage has no reviewed Windows scan-code mapping.
    pub fn map(&mut self, action: &InputAction) -> Result<Vec<WindowsEvent>, BackendError> {
        match action {
            InputAction::PointerMove(delta) => {
                let pixels = self.pointer.translate(*delta);
                let (dx, dy) = (pixels.x(), pixels.y());
                if dx == 0 && dy == 0 {
                    return Ok(Vec::new());
                }
                Ok(vec![WindowsEvent::RelativeMotion { dx, dy }])
            }
            InputAction::Scroll(delta) => Ok(self.map_wheel(*delta)),
            InputAction::MouseButton(action) => Ok(map_button(*action)),
            InputAction::PhysicalKey(action) => map_key(*action).map(|event| vec![event]),
            InputAction::TextInsert(text) => Ok(map_unicode(text.expose_for_injection())),
            InputAction::TextEdit(command) => map_key(KeyAction::Down(text_edit_key(*command)))
                .and_then(|down| {
                    map_key(KeyAction::Up(text_edit_key(*command))).map(|up| vec![down, up])
                }),
            InputAction::LogicalCommand(command) => map_logical(*command),
            InputAction::MediaCommand(command) => Ok(map_media(*command)),
        }
    }

    fn map_wheel(&mut self, scroll: ScrollDelta) -> Vec<WindowsEvent> {
        let horizontal = scroll.axis() == ScrollAxis::Horizontal;
        let remainder = match scroll.axis() {
            ScrollAxis::Horizontal => &mut self.horizontal_wheel_remainder,
            ScrollAxis::Vertical => &mut self.vertical_wheel_remainder,
        };
        let total = i64::from(scroll.amount()) * WHEEL_DELTA + *remainder;
        let scale = i64::from(FIXED_POINT_SCALE);
        // A whole normalized unit maps to one detent. Keep the remainder in
        // the scaled numerator so sub-detent conversion has no rounding drift.
        let delta = i32::try_from(total / scale).expect("bounded scroll fits wheel units");
        *remainder = total % scale;
        if delta == 0 {
            return Vec::new();
        }
        vec![WindowsEvent::Wheel { horizontal, delta }]
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

fn button_record(button: MouseButton, down: bool) -> WindowsEvent {
    let (down_flag, up_flag, data) = match button {
        MouseButton::Left => (0x0002, 0x0004, 0),
        MouseButton::Right => (0x0008, 0x0010, 0),
        MouseButton::Middle => (0x0020, 0x0040, 0),
        MouseButton::Back => (0x0080, 0x0100, 1),
        MouseButton::Forward => (0x0080, 0x0100, 2),
    };
    WindowsEvent::MouseButton {
        flag: if down { down_flag } else { up_flag },
        data,
    }
}

fn map_button(action: ButtonAction) -> Vec<WindowsEvent> {
    match action {
        ButtonAction::Down(button) => vec![button_record(button, true)],
        ButtonAction::Up(button) => vec![button_record(button, false)],
        ButtonAction::Click { button, count } => {
            let repeats = match count {
                ClickCount::Single => 1,
                ClickCount::Double => 2,
            };
            let mut events = Vec::with_capacity(repeats * 2);
            for _ in 0..repeats {
                events.push(button_record(button, true));
                events.push(button_record(button, false));
            }
            events
        }
    }
}

fn map_key(action: KeyAction) -> Result<WindowsEvent, BackendError> {
    let (key, down) = match action {
        KeyAction::Down(key) => (key, true),
        KeyAction::Up(key) => (key, false),
    };
    let Some((scan_code, extended)) = hid_to_scan_code(key.usage()) else {
        return Err(unsupported(OperationClass::PhysicalKey));
    };
    Ok(WindowsEvent::ScanCode {
        scan_code,
        extended,
        down,
    })
}

fn map_unicode(text: &str) -> Vec<WindowsEvent> {
    let mut events = Vec::with_capacity(text.encode_utf16().count().saturating_mul(2));
    for code_unit in text.encode_utf16() {
        events.push(WindowsEvent::UnicodeCodeUnit {
            code_unit,
            down: true,
        });
        events.push(WindowsEvent::UnicodeCodeUnit {
            code_unit,
            down: false,
        });
    }
    events
}

fn map_logical(
    command: tether_input_core::LogicalCommand,
) -> Result<Vec<WindowsEvent>, BackendError> {
    let Some(chord) = canonical_logical_chord(DesktopShortcutPlatform::Windows, command) else {
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

fn map_media(command: MediaCommand) -> Vec<WindowsEvent> {
    let virtual_key = match command {
        MediaCommand::PlayPause => 0xB3,
        MediaCommand::NextTrack => 0xB0,
        MediaCommand::PreviousTrack => 0xB1,
        MediaCommand::VolumeUp => 0xAF,
        MediaCommand::VolumeDown => 0xAE,
        MediaCommand::Mute => 0xAD,
    };
    vec![
        WindowsEvent::VirtualKey {
            virtual_key,
            down: true,
        },
        WindowsEvent::VirtualKey {
            virtual_key,
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
fn hid_to_scan_code(usage: u8) -> Option<(u16, bool)> {
    Some(match usage {
        0x04 => (0x1E, false), 0x05 => (0x30, false), 0x06 => (0x2E, false),
        0x07 => (0x20, false), 0x08 => (0x12, false), 0x09 => (0x21, false),
        0x0A => (0x22, false), 0x0B => (0x23, false), 0x0C => (0x17, false),
        0x0D => (0x24, false), 0x0E => (0x25, false), 0x0F => (0x26, false),
        0x10 => (0x32, false), 0x11 => (0x31, false), 0x12 => (0x18, false),
        0x13 => (0x19, false), 0x14 => (0x10, false), 0x15 => (0x13, false),
        0x16 => (0x1F, false), 0x17 => (0x14, false), 0x18 => (0x16, false),
        0x19 => (0x2F, false), 0x1A => (0x11, false), 0x1B => (0x2D, false),
        0x1C => (0x15, false), 0x1D => (0x2C, false),
        0x1E..=0x26 => (u16::from(usage - 0x1E) + 0x02, false),
        0x27 => (0x0B, false), 0x28 => (0x1C, false), 0x29 => (0x01, false),
        0x2A => (0x0E, false), 0x2B => (0x0F, false), 0x2C => (0x39, false),
        0x2D => (0x0C, false), 0x2E => (0x0D, false), 0x2F => (0x1A, false),
        0x30 => (0x1B, false), 0x31 | 0x32 => (0x2B, false),
        0x33 => (0x27, false), 0x34 => (0x28, false), 0x35 => (0x29, false),
        0x36 => (0x33, false), 0x37 => (0x34, false), 0x38 => (0x35, false),
        0x39 => (0x3A, false), 0x3A..=0x43 => (u16::from(usage - 0x3A) + 0x3B, false),
        0x44 => (0x57, false), 0x45 => (0x58, false), 0x46 => (0x37, true),
        0x47 => (0x46, false), 0x49 => (0x52, true), 0x4A => (0x47, true),
        0x4B => (0x49, true), 0x4C => (0x53, true), 0x4D => (0x4F, true),
        0x4E => (0x51, true), 0x4F => (0x4D, true), 0x50 => (0x4B, true),
        0x51 => (0x50, true), 0x52 => (0x48, true),
        0x53 => (0x45, false), 0x54 => (0x35, true), 0x55 => (0x37, false),
        0x56 => (0x4A, false), 0x57 => (0x4E, false), 0x58 => (0x1C, true),
        0x59 => (0x4F, false), 0x5A => (0x50, false), 0x5B => (0x51, false),
        0x5C => (0x4B, false), 0x5D => (0x4C, false), 0x5E => (0x4D, false),
        0x5F => (0x47, false), 0x60 => (0x48, false), 0x61 => (0x49, false),
        0x62 => (0x52, false), 0x63 => (0x53, false),
        0xE0 => (0x1D, false), 0xE1 => (0x2A, false), 0xE2 => (0x38, false),
        0xE3 => (0x5B, true), 0xE4 => (0x1D, true), 0xE5 => (0x36, false),
        0xE6 => (0x38, true), 0xE7 => (0x5C, true),
        _ => return None,
    })
}

#[cfg(windows)]
mod binding_contract {
    use windows::Win32::UI::Input::KeyboardAndMouse::{INPUT, KEYBDINPUT, MOUSEINPUT, SendInput};

    #[allow(dead_code)]
    fn api_surface_compiles() {
        let _ = std::mem::size_of::<INPUT>();
        let _ = std::mem::size_of::<KEYBDINPUT>();
        let _ = std::mem::size_of::<MOUSEINPUT>();
        let _ = SendInput;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tether_input_core::{
        InertConformanceBackend, InertConformanceMapper, InertConformanceSink, InertSinkError,
        InputBackend, MAX_ABSOLUTE_FIXED_DELTA, PointerDelta, ReleaseFailure, SensitiveText,
        run_backend_conformance,
    };

    #[derive(Default)]
    struct RecordingSink {
        events: Vec<WindowsEvent>,
        fail_next_after: Option<usize>,
    }

    impl InertConformanceSink<WindowsEvent> for RecordingSink {
        fn emit(&mut self, events: &[WindowsEvent]) -> Result<(), InertSinkError> {
            if let Some(accepted) = self.fail_next_after.take() {
                let accepted = accepted.min(events.len());
                self.events.extend_from_slice(&events[..accepted]);
                return Err(InertSinkError::new(accepted));
            }
            self.events.extend_from_slice(events);
            Ok(())
        }
    }

    impl InertConformanceMapper for WindowsMapper {
        type Event = WindowsEvent;

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
            self.reset_fractional_state();
            Ok(())
        }
    }

    fn backend() -> InertConformanceBackend<WindowsMapper, RecordingSink> {
        InertConformanceBackend::new(
            WindowsMapper::from_probe(WindowsProbe::inert_ready()),
            RecordingSink::default(),
        )
    }

    #[test]
    fn inert_mapper_passes_shared_conformance() {
        let mut backend = backend();
        let report = run_backend_conformance(&mut backend, BackendKind::Windows)
            .expect("Windows inert conformance");
        assert!(report.checked_actions > 200);
        assert!(report.explicitly_unsupported_mappings > 0);
        assert_eq!(backend.held_counts(), (0, 0));
    }

    #[test]
    fn relative_motion_converts_to_pixels_and_keeps_independent_fractions() {
        let mut mapper = WindowsMapper::from_probe(WindowsProbe::inert_ready());
        let half = FIXED_POINT_SCALE / 2;
        for (dx, dy, expected) in [
            (half, -half, None),
            (half, 0, Some((1, 0))),
            (-half, -half, Some((0, -1))),
            (half, half, None),
            (0, -half, None),
            (
                MAX_ABSOLUTE_FIXED_DELTA,
                -MAX_ABSOLUTE_FIXED_DELTA,
                Some((8_192, -8_192)),
            ),
        ] {
            let action = InputAction::PointerMove(PointerDelta::new(dx, dy).expect("fixture"));
            let events = mapper.map(&action).expect("motion plan");
            match expected {
                Some((dx, dy)) => assert_eq!(events, [WindowsEvent::RelativeMotion { dx, dy }]),
                None => assert_eq!(events, []),
            }
        }
    }

    #[test]
    fn wheel_conversion_keeps_sign_and_sub_detent_precision_per_axis() {
        let mut mapper = WindowsMapper::from_probe(WindowsProbe::inert_ready());
        for (axis, amount, expected) in [
            (ScrollAxis::Horizontal, -FIXED_POINT_SCALE, -120),
            (ScrollAxis::Vertical, FIXED_POINT_SCALE - 1, 119),
            (ScrollAxis::Horizontal, 1, 0),
            (ScrollAxis::Vertical, 1, 1),
            (ScrollAxis::Horizontal, -1, 0),
            (ScrollAxis::Horizontal, -FIXED_POINT_SCALE / 2, -60),
            (ScrollAxis::Vertical, MAX_ABSOLUTE_FIXED_DELTA, 983_040),
            (ScrollAxis::Vertical, -MAX_ABSOLUTE_FIXED_DELTA, -983_040),
        ] {
            let action = InputAction::Scroll(ScrollDelta::new(axis, amount).expect("fixture"));
            let events = mapper.map(&action).expect("wheel plan");
            if expected == 0 {
                assert_eq!(events, []);
            } else {
                assert_eq!(
                    events,
                    [WindowsEvent::Wheel {
                        horizontal: axis == ScrollAxis::Horizontal,
                        delta: expected,
                    }]
                );
            }
        }
    }

    #[test]
    fn release_discards_fractional_motion_and_scroll() {
        let mut backend = backend();
        let half = FIXED_POINT_SCALE / 2;
        let motion = InputAction::PointerMove(PointerDelta::new(half, -half).expect("fixture"));
        let horizontal =
            InputAction::Scroll(ScrollDelta::new(ScrollAxis::Horizontal, 8).expect("fixture"));
        let vertical =
            InputAction::Scroll(ScrollDelta::new(ScrollAxis::Vertical, -8).expect("fixture"));
        for _ in 0..2 {
            for action in [&motion, &horizontal, &vertical] {
                backend.execute(action).expect("inert accumulation");
            }
            assert_eq!(backend.sink().events, []);
            assert!(backend.release_all().is_confirmed_neutral());
        }
    }

    #[test]
    fn unicode_supplementary_scalar_emits_ordered_surrogate_pairs() {
        let mut mapper = WindowsMapper::from_probe(WindowsProbe::inert_ready());
        let text = SensitiveText::new("😀".to_owned()).expect("fixture");
        let events = mapper
            .map(&InputAction::TextInsert(text))
            .expect("Unicode mapping");
        let (pairs, remainder) = events.as_chunks::<2>();
        assert_eq!(remainder.len(), 0);
        let units = pairs
            .iter()
            .map(|pair| match pair[0] {
                WindowsEvent::UnicodeCodeUnit {
                    code_unit,
                    down: true,
                } => code_unit,
                _ => panic!("expected Unicode down event"),
            })
            .collect::<Vec<_>>();
        assert_eq!(units, [0xD83D, 0xDE00]);
        assert!(pairs.iter().all(|pair| matches!(
            (&pair[0], &pair[1]),
            (
                WindowsEvent::UnicodeCodeUnit { code_unit: down, down: true },
                WindowsEvent::UnicodeCodeUnit { code_unit: up, down: false }
            ) if down == up
        )));
    }

    #[test]
    fn unmappable_hid_is_explicit_without_hiding_keyboard_capability() {
        let mut backend = backend();
        assert!(backend.status().capabilities.physical_keys.is_available());
        let key = PhysicalKey::new(0xB0).expect("allowlisted but unmapped fixture");
        let error = backend
            .execute(&InputAction::PhysicalKey(KeyAction::Down(key)))
            .expect_err("mapping must be rejected");
        assert_eq!(error.category(), BackendErrorCategory::Unsupported);
        assert_eq!(backend.sink().events.len(), 0);
        assert_eq!(backend.held_counts(), (0, 0));
    }

    #[test]
    fn held_state_releases_and_partial_sink_failure_propagates() {
        let mut held_backend = backend();
        let key = PhysicalKey::new(0x04).expect("mapped fixture");
        held_backend
            .execute(&InputAction::PhysicalKey(KeyAction::Down(key)))
            .expect("key down");
        assert_eq!(held_backend.held_counts(), (0, 1));
        assert!(held_backend.release_all().is_confirmed_neutral());
        assert_eq!(held_backend.held_counts(), (0, 0));

        let mut failing = backend();
        failing.sink_mut().fail_next_after = Some(1);
        let click = InputAction::MouseButton(
            ButtonAction::click(MouseButton::Left, 1).expect("valid click"),
        );
        let error = failing.execute(&click).expect_err("partial sink failure");
        assert_eq!(error.category(), BackendErrorCategory::PartialStateChange);
        assert!(failing.has_uncertain_native_state());
        assert!(!failing.release_all().is_confirmed_neutral());
    }

    #[test]
    fn unavailable_probe_fails_closed() {
        let mapper = WindowsMapper::from_probe(WindowsProbe {
            send_input_available: true,
            interactive_desktop: false,
        });
        assert!(!mapper.status().available);
        assert!(mapper.status().capabilities.is_empty());
    }
}
