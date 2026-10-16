//! Wayland `RemoteDesktop` portal and EI translation spike.
//!
//! The mapper consumes a simulated portal/EI negotiation snapshot and emits
//! inert protocol plans. Normal tests never open a portal or an EI socket and
//! never inject native input.

use std::collections::BTreeSet;
use std::fmt;
use tether_input_core::{
    BackendCapabilities, BackendError, BackendErrorCategory, BackendKind, BackendStatus,
    ButtonAction, CapabilityState, ClickCount, DesktopShortcutPlatform, FIXED_POINT_SCALE,
    InputAction, KeyAction, LOGICAL_COMMANDS, MediaCommand, MouseButton, OperationClass,
    PermissionState, PhysicalKey, RetryScope, ScrollAxis, StatusRefreshReason, TextEditCommand,
    TextMode, canonical_logical_chord,
};

const MAX_EI_TEXT_BYTES: usize = 254;

/// EI device lifecycle relevant to input admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceLifecycle {
    Active,
    Paused,
    Removed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortalSessionState {
    Absent,
    Created,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EiConnectionState {
    Disconnected,
    Connected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestoreTokenState {
    Absent,
    Present,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WaylandCapability {
    Pointer,
    Buttons,
    Scroll,
    Keyboard,
    Text,
}

/// Portal and EI capabilities observed during a bounded negotiation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaylandProbe {
    pub portal_version: u32,
    pub session: PortalSessionState,
    pub permission: PermissionState,
    pub restore_token: RestoreTokenState,
    pub connection: EiConnectionState,
    pub device: DeviceLifecycle,
    pub capabilities: BTreeSet<WaylandCapability>,
}

impl WaylandProbe {
    #[must_use]
    pub fn inert_ready() -> Self {
        Self {
            portal_version: 2,
            session: PortalSessionState::Created,
            permission: PermissionState::Granted,
            restore_token: RestoreTokenState::Present,
            connection: EiConnectionState::Connected,
            device: DeviceLifecycle::Active,
            capabilities: BTreeSet::from([
                WaylandCapability::Pointer,
                WaylandCapability::Buttons,
                WaylandCapability::Scroll,
                WaylandCapability::Keyboard,
                WaylandCapability::Text,
            ]),
        }
    }

    fn has(&self, capability: WaylandCapability) -> bool {
        self.capabilities.contains(&capability)
    }
}

/// Pure description of future EI sender requests.
#[derive(Clone, Eq, PartialEq)]
pub enum WaylandEvent {
    PointerRelative {
        dx: i32,
        dy: i32,
    },
    Button {
        evdev_code: u32,
        down: bool,
    },
    ScrollDiscrete {
        horizontal_v120: i32,
        vertical_v120: i32,
    },
    ScrollStop {
        horizontal: bool,
        vertical: bool,
        cancel: bool,
    },
    Key {
        evdev_code: u32,
        down: bool,
    },
    TextUtf8(Vec<u8>),
    Frame,
    StopEmulating,
}

impl fmt::Debug for WaylandEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("WaylandEvent(<redacted>)")
    }
}

/// Pure portal-negotiated EI action translator.
pub struct WaylandMapper {
    probe: WaylandProbe,
    status: BackendStatus,
    horizontal_scroll_remainder: i64,
    vertical_scroll_remainder: i64,
}

impl WaylandMapper {
    #[must_use]
    pub fn from_probe(probe: WaylandProbe) -> Self {
        let status = status_from_probe(&probe, 1);
        Self {
            probe,
            status,
            horizontal_scroll_remainder: 0,
            vertical_scroll_remainder: 0,
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

    /// Applies an observed EI device lifecycle transition.
    #[must_use]
    pub fn apply_device_lifecycle(&mut self, lifecycle: DeviceLifecycle) -> BackendStatus {
        self.probe.device = lifecycle;
        self.status = status_from_probe(&self.probe, self.status.revision.saturating_add(1));
        self.status()
    }

    /// Returns a framed scroll-stop/cancel plan for session cleanup.
    #[must_use]
    pub fn scroll_stop_plan(axis: ScrollAxis, cancel: bool) -> Vec<WaylandEvent> {
        vec![
            WaylandEvent::ScrollStop {
                horizontal: axis == ScrollAxis::Horizontal,
                vertical: axis == ScrollAxis::Vertical,
                cancel,
            },
            WaylandEvent::Frame,
        ]
    }

    /// Returns the terminal emulation-stop plan used after input release.
    #[must_use]
    pub fn stop_emulating_plan() -> Vec<WaylandEvent> {
        vec![WaylandEvent::StopEmulating]
    }

    /// Clears fixed-point-to-v120 residue during release/session transitions.
    pub fn clear_scroll_residue(&mut self) {
        self.horizontal_scroll_remainder = 0;
        self.vertical_scroll_remainder = 0;
    }

    /// Translates one action without opening a portal or EI connection.
    ///
    /// # Errors
    ///
    /// Returns a safe unsupported/unavailable error before emission if the
    /// negotiated device lacks the required interface or key mapping.
    pub fn map(&mut self, action: &InputAction) -> Result<Vec<WaylandEvent>, BackendError> {
        if !self.status.available {
            return Err(unavailable(action.class()));
        }
        match action {
            InputAction::PointerMove(delta) if self.probe.has(WaylandCapability::Pointer) => {
                Ok(vec![
                    WaylandEvent::PointerRelative {
                        dx: delta.x(),
                        dy: delta.y(),
                    },
                    WaylandEvent::Frame,
                ])
            }
            InputAction::Scroll(delta) if self.probe.has(WaylandCapability::Scroll) => {
                self.map_scroll(delta.axis(), delta.amount())
            }
            InputAction::MouseButton(action) if self.probe.has(WaylandCapability::Buttons) => {
                Ok(map_button(*action))
            }
            InputAction::PhysicalKey(action) if self.probe.has(WaylandCapability::Keyboard) => {
                map_key(*action).map(|event| vec![event, WaylandEvent::Frame])
            }
            InputAction::TextInsert(text) if self.probe.has(WaylandCapability::Text) => {
                Ok(map_text_chunks(text.expose_for_injection()))
            }
            InputAction::TextEdit(command) if self.probe.has(WaylandCapability::Keyboard) => {
                let key = text_edit_key(*command);
                Ok(vec![
                    map_key(KeyAction::Down(key))?,
                    map_key(KeyAction::Up(key))?,
                    WaylandEvent::Frame,
                ])
            }
            InputAction::LogicalCommand(command) if self.probe.has(WaylandCapability::Keyboard) => {
                map_logical(*command)
            }
            InputAction::MediaCommand(command) if self.probe.has(WaylandCapability::Keyboard) => {
                Ok(map_media(*command))
            }
            _ => Err(unsupported(action.class())),
        }
    }

    fn map_scroll(
        &mut self,
        axis: ScrollAxis,
        fixed_amount: i32,
    ) -> Result<Vec<WaylandEvent>, BackendError> {
        let remainder = match axis {
            ScrollAxis::Horizontal => self.horizontal_scroll_remainder,
            ScrollAxis::Vertical => self.vertical_scroll_remainder,
        };
        let numerator = i64::from(fixed_amount)
            .saturating_mul(120)
            .saturating_add(remainder);
        let v120 = numerator / i64::from(FIXED_POINT_SCALE);
        let new_remainder = numerator % i64::from(FIXED_POINT_SCALE);
        let v120 = i32::try_from(v120).map_err(|_| unavailable(OperationClass::Scroll))?;
        match axis {
            ScrollAxis::Horizontal => self.horizontal_scroll_remainder = new_remainder,
            ScrollAxis::Vertical => self.vertical_scroll_remainder = new_remainder,
        }
        if v120 == 0 {
            return Ok(Vec::new());
        }
        let (horizontal_v120, vertical_v120) = match axis {
            ScrollAxis::Horizontal => (v120, 0),
            ScrollAxis::Vertical => (0, v120),
        };
        Ok(vec![
            WaylandEvent::ScrollDiscrete {
                horizontal_v120,
                vertical_v120,
            },
            WaylandEvent::Frame,
        ])
    }
}

fn status_from_probe(probe: &WaylandProbe, revision: u64) -> BackendStatus {
    let permission_ready = matches!(probe.permission, PermissionState::Granted);
    let active = probe.portal_version >= 2
        && probe.session == PortalSessionState::Created
        && permission_ready
        && probe.connection == EiConnectionState::Connected
        && probe.device == DeviceLifecycle::Active;
    let any_input = !probe.capabilities.is_empty();
    let available = active && any_input;
    let logical_commands = if active && probe.has(WaylandCapability::Keyboard) {
        LOGICAL_COMMANDS
            .into_iter()
            .filter(|command| {
                canonical_logical_chord(DesktopShortcutPlatform::Linux, *command).is_some()
            })
            .collect()
    } else {
        BTreeSet::new()
    };
    BackendStatus {
        revision,
        kind: BackendKind::WaylandEi,
        initialized: probe.portal_version >= 2 && probe.session == PortalSessionState::Created,
        available,
        permission: probe.permission,
        capabilities: if available {
            BackendCapabilities {
                relative_pointer: capability(probe.has(WaylandCapability::Pointer)),
                mouse_buttons: if probe.has(WaylandCapability::Buttons) {
                    all_mouse_buttons()
                } else {
                    BTreeSet::new()
                },
                atomic_click: capability(probe.has(WaylandCapability::Buttons)),
                vertical_scroll: capability(probe.has(WaylandCapability::Scroll)),
                horizontal_scroll: capability(probe.has(WaylandCapability::Scroll)),
                // Device-wide keyboard availability is separate from per-HID
                // evdev mapping failures (F-08).
                physical_keys: capability(probe.has(WaylandCapability::Keyboard)),
                text_modes: if probe.has(WaylandCapability::Text) {
                    BTreeSet::from([TextMode::EiUtf8])
                } else {
                    BTreeSet::new()
                },
                logical_commands,
                media_commands: if probe.has(WaylandCapability::Keyboard) {
                    all_media_commands()
                } else {
                    BTreeSet::new()
                },
            }
        } else {
            BackendCapabilities::unavailable()
        },
        fault: None,
    }
}

const fn capability(enabled: bool) -> CapabilityState {
    if enabled {
        CapabilityState::Available
    } else {
        CapabilityState::Unavailable
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

fn unavailable(operation: OperationClass) -> BackendError {
    BackendError::new(
        BackendErrorCategory::TemporarilyUnavailable,
        operation,
        RetryScope::AfterStatusRefresh,
    )
}

const fn button_code(button: MouseButton) -> u32 {
    match button {
        MouseButton::Left => 0x110,
        MouseButton::Right => 0x111,
        MouseButton::Middle => 0x112,
        MouseButton::Back => 0x116,
        MouseButton::Forward => 0x115,
    }
}

fn map_button(action: ButtonAction) -> Vec<WaylandEvent> {
    let mut events = match action {
        ButtonAction::Down(button) => vec![WaylandEvent::Button {
            evdev_code: button_code(button),
            down: true,
        }],
        ButtonAction::Up(button) => vec![WaylandEvent::Button {
            evdev_code: button_code(button),
            down: false,
        }],
        ButtonAction::Click { button, count } => {
            let repeats = match count {
                ClickCount::Single => 1,
                ClickCount::Double => 2,
            };
            let mut clicks = Vec::with_capacity(repeats * 2 + 1);
            for _ in 0..repeats {
                clicks.push(WaylandEvent::Button {
                    evdev_code: button_code(button),
                    down: true,
                });
                clicks.push(WaylandEvent::Button {
                    evdev_code: button_code(button),
                    down: false,
                });
            }
            clicks
        }
    };
    events.push(WaylandEvent::Frame);
    events
}

fn map_key(action: KeyAction) -> Result<WaylandEvent, BackendError> {
    let (key, down) = match action {
        KeyAction::Down(key) => (key, true),
        KeyAction::Up(key) => (key, false),
    };
    let Some(evdev_code) = hid_to_evdev(key.usage()) else {
        return Err(unsupported(OperationClass::PhysicalKey));
    };
    Ok(WaylandEvent::Key { evdev_code, down })
}

fn map_logical(
    command: tether_input_core::LogicalCommand,
) -> Result<Vec<WaylandEvent>, BackendError> {
    let Some(chord) = canonical_logical_chord(DesktopShortcutPlatform::Linux, command) else {
        return Err(unsupported(OperationClass::LogicalCommand));
    };
    let mut events = Vec::with_capacity(chord.len().saturating_mul(2).saturating_add(1));
    for key in chord.press_order() {
        events.push(map_key(KeyAction::Down(key))?);
    }
    for key in chord.release_order() {
        events.push(map_key(KeyAction::Up(key))?);
    }
    events.push(WaylandEvent::Frame);
    Ok(events)
}

fn map_media(command: MediaCommand) -> Vec<WaylandEvent> {
    let evdev_code = match command {
        MediaCommand::PlayPause => 164,
        MediaCommand::NextTrack => 163,
        MediaCommand::PreviousTrack => 165,
        MediaCommand::VolumeUp => 115,
        MediaCommand::VolumeDown => 114,
        MediaCommand::Mute => 113,
    };
    vec![
        WaylandEvent::Key {
            evdev_code,
            down: true,
        },
        WaylandEvent::Key {
            evdev_code,
            down: false,
        },
        WaylandEvent::Frame,
    ]
}

fn map_text_chunks(text: &str) -> Vec<WaylandEvent> {
    let mut events = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let mut end = (start + MAX_EI_TEXT_BYTES).min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        if end == start {
            end = text[start..]
                .char_indices()
                .nth(1)
                .map_or(text.len(), |(offset, _)| start + offset);
        }
        events.push(WaylandEvent::TextUtf8(text.as_bytes()[start..end].to_vec()));
        events.push(WaylandEvent::Frame);
        start = end;
    }
    events
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
fn hid_to_evdev(usage: u8) -> Option<u32> {
    Some(match usage {
        0x04 => 30, 0x05 => 48, 0x06 => 46, 0x07 => 32, 0x08 => 18,
        0x09 => 33, 0x0A => 34, 0x0B => 35, 0x0C => 23, 0x0D => 36,
        0x0E => 37, 0x0F => 38, 0x10 => 50, 0x11 => 49, 0x12 => 24,
        0x13 => 25, 0x14 => 16, 0x15 => 19, 0x16 => 31, 0x17 => 20,
        0x18 => 22, 0x19 => 47, 0x1A => 17, 0x1B => 45, 0x1C => 21,
        0x1D => 44, 0x28 => 28, 0x29 => 1, 0x2A => 14, 0x2B => 15,
        0x2C => 57, 0x2D => 12, 0x4A => 102, 0x4B => 104, 0x4C => 111,
        0x4D => 107, 0x4E => 109, 0x4F => 106, 0x50 => 105, 0x51 => 108,
        0x52 => 103, 0xE0 => 29, 0xE1 => 42, 0xE2 => 56, 0xE3 => 125,
        0xE4 => 97, 0xE5 => 54, 0xE6 => 100, 0xE7 => 126,
        _ => return None,
    })
}

#[cfg(target_os = "linux")]
mod binding_contract {
    use ashpd::desktop::remote_desktop::RemoteDesktop;
    use reis::ei::{
        Context, button::Button, device::Device, keyboard::Keyboard, pointer::Pointer, text::Text,
    };

    #[allow(dead_code)]
    fn api_surface_compiles() {
        let _ = RemoteDesktop::new;
        let _ = RemoteDesktop::connect_to_eis;
        let _ = Context::new;
        let _ = Device::start_emulating;
        let _ = Device::stop_emulating;
        let _ = Device::frame;
        let _ = Pointer::motion_relative;
        let _ = Button::button;
        let _ = Keyboard::key;
        let _ = Text::utf8;
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
        events: Vec<WaylandEvent>,
        fail_next_after: Option<usize>,
    }

    impl InertConformanceSink<WaylandEvent> for RecordingSink {
        fn emit(&mut self, events: &[WaylandEvent]) -> Result<(), InertSinkError> {
            if let Some(accepted) = self.fail_next_after.take() {
                let accepted = accepted.min(events.len());
                self.events.extend_from_slice(&events[..accepted]);
                return Err(InertSinkError::new(accepted));
            }
            self.events.extend_from_slice(events);
            Ok(())
        }
    }

    impl InertConformanceMapper for WaylandMapper {
        type Event = WaylandEvent;
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
            self.clear_scroll_residue();
            Ok(())
        }
    }

    fn backend() -> InertConformanceBackend<WaylandMapper, RecordingSink> {
        InertConformanceBackend::new(
            WaylandMapper::from_probe(WaylandProbe::inert_ready()),
            RecordingSink::default(),
        )
    }

    #[test]
    fn inert_mapper_passes_shared_conformance() {
        let mut backend = backend();
        let report = run_backend_conformance(&mut backend, BackendKind::WaylandEi)
            .expect("Wayland inert conformance");
        assert!(report.checked_actions > 200);
        assert!(report.explicitly_unsupported_mappings > 0);
        assert_eq!(backend.held_counts(), (0, 0));
    }

    #[test]
    fn optional_text_capability_is_honest_and_utf8_is_framed() {
        let long = "é".repeat(180);
        let text = SensitiveText::new(long).expect("bounded fixture");
        let mut mapper = WaylandMapper::from_probe(WaylandProbe::inert_ready());
        let events = mapper
            .map(&InputAction::TextInsert(text))
            .expect("text map");
        assert!(events.len() >= 4);
        let (pairs, remainder) = events.as_chunks::<2>();
        assert_eq!(remainder.len(), 0);
        for pair in pairs {
            match (&pair[0], &pair[1]) {
                (WaylandEvent::TextUtf8(bytes), WaylandEvent::Frame) => {
                    assert_ne!(bytes.as_slice(), []);
                    assert!(bytes.len() <= MAX_EI_TEXT_BYTES);
                    assert!(std::str::from_utf8(bytes).is_ok());
                }
                _ => panic!("each text request must have its own frame"),
            }
        }

        let mut probe = WaylandProbe::inert_ready();
        probe.capabilities.remove(&WaylandCapability::Text);
        let mapper = WaylandMapper::from_probe(probe);
        assert!(mapper.status().capabilities.text_modes.is_empty());
        assert!(mapper.status().capabilities.physical_keys.is_available());
    }

    #[test]
    fn discrete_scroll_uses_v120_units_and_release_resets_residue() {
        let mut backend = backend();
        let tiny = InputAction::Scroll(
            ScrollDelta::new(ScrollAxis::Vertical, 1).expect("valid tiny delta"),
        );
        backend.execute(&tiny).expect("accumulate tiny delta");
        assert_eq!(backend.sink().events.len(), 0);
        assert!(backend.release_all().is_confirmed_neutral());

        let almost_one = InputAction::Scroll(
            ScrollDelta::new(ScrollAxis::Vertical, FIXED_POINT_SCALE - 1).expect("valid delta"),
        );
        backend.execute(&almost_one).expect("map v120 delta");
        assert_eq!(
            backend.sink().events,
            vec![
                WaylandEvent::ScrollDiscrete {
                    horizontal_v120: 0,
                    vertical_v120: 119,
                },
                WaylandEvent::Frame,
            ]
        );

        let horizontal = InputAction::Scroll(
            ScrollDelta::new(ScrollAxis::Horizontal, FIXED_POINT_SCALE / 2).expect("valid delta"),
        );
        backend.execute(&horizontal).expect("map horizontal v120");
        assert_eq!(
            backend.sink().events[2],
            WaylandEvent::ScrollDiscrete {
                horizontal_v120: 60,
                vertical_v120: 0,
            }
        );
    }

    #[test]
    fn lifecycle_pause_and_removal_fail_closed() {
        let mut mapper = WaylandMapper::from_probe(WaylandProbe::inert_ready());
        assert!(
            mapper
                .apply_device_lifecycle(DeviceLifecycle::Paused)
                .capabilities
                .is_empty()
        );
        assert!(!mapper.status().available);
        assert!(
            mapper
                .apply_device_lifecycle(DeviceLifecycle::Removed)
                .capabilities
                .is_empty()
        );
        assert!(!mapper.status().available);
    }

    #[test]
    fn portal_version_permission_and_disconnect_fail_closed() {
        let mut probe = WaylandProbe::inert_ready();
        probe.portal_version = 1;
        assert!(!WaylandMapper::from_probe(probe).status().available);
        let mut probe = WaylandProbe::inert_ready();
        probe.permission = PermissionState::Denied;
        assert!(!WaylandMapper::from_probe(probe).status().available);
        let mut probe = WaylandProbe::inert_ready();
        probe.connection = EiConnectionState::Disconnected;
        assert!(!WaylandMapper::from_probe(probe).status().available);
    }

    #[test]
    fn partial_keyboard_mapping_and_cleanup_errors_are_explicit() {
        let mut backend = backend();
        assert!(backend.status().capabilities.physical_keys.is_available());
        let unmapped = PhysicalKey::new(0xB0).expect("allowlisted fixture");
        let error = backend
            .execute(&InputAction::PhysicalKey(KeyAction::Down(unmapped)))
            .expect_err("unsupported mapping");
        assert_eq!(error.category(), BackendErrorCategory::Unsupported);
        assert_eq!(backend.sink().events.len(), 0);

        let held = PhysicalKey::new(0x04).expect("mapped fixture");
        backend
            .execute(&InputAction::PhysicalKey(KeyAction::Down(held)))
            .expect("down");
        backend.sink_mut().fail_next_after = Some(1);
        assert!(!backend.release_all().is_confirmed_neutral());
        assert!(backend.has_uncertain_native_state());
    }

    #[test]
    fn stop_and_scroll_cleanup_plans_are_explicit() {
        assert_eq!(
            WaylandMapper::stop_emulating_plan(),
            vec![WaylandEvent::StopEmulating]
        );
        assert_eq!(
            WaylandMapper::scroll_stop_plan(ScrollAxis::Horizontal, true),
            vec![
                WaylandEvent::ScrollStop {
                    horizontal: true,
                    vertical: false,
                    cancel: true
                },
                WaylandEvent::Frame,
            ]
        );
    }
}
