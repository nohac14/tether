use crate::{LogicalCommand, PhysicalKey};
use std::fmt;

/// Canonical USB HID Keyboard/Keypad usage page used by protocol version one.
pub const HID_KEYBOARD_USAGE_PAGE: u16 = 0x07;

/// Inclusive allowlisted usage interval with one semantic class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HidUsageRange {
    pub first: u8,
    pub last: u8,
    pub class: HidUsageClass,
}

impl HidUsageRange {
    /// Returns whether this interval contains `usage`.
    #[must_use]
    pub const fn contains(self, usage: u8) -> bool {
        usage >= self.first && usage <= self.last
    }
}

/// Stable grouping for allowlist review; it is not a native key-code class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HidUsageClass {
    StandardKeyboard,
    ExtendedKeyboard,
    ExtendedKeypad,
    Modifier,
}

/// Explicit protocol-version-one Keyboard/Keypad allowlist.
///
/// Reserved/error usages, Keyboard Power (`0x66`), the reserved `0xA5..=0xAF`
/// and `0xDE..=0xDF` intervals, and values above one byte are excluded.
pub const ALLOWLISTED_HID_USAGE_RANGES: [HidUsageRange; 4] = [
    HidUsageRange {
        first: 0x04,
        last: 0x65,
        class: HidUsageClass::StandardKeyboard,
    },
    HidUsageRange {
        first: 0x67,
        last: 0xA4,
        class: HidUsageClass::ExtendedKeyboard,
    },
    HidUsageRange {
        first: 0xB0,
        last: 0xDD,
        class: HidUsageClass::ExtendedKeypad,
    },
    HidUsageRange {
        first: 0xE0,
        last: 0xE7,
        class: HidUsageClass::Modifier,
    },
];

/// Returns whether a usage belongs to the closed version-one allowlist.
#[must_use]
pub const fn is_allowlisted_keyboard_usage(usage: u16) -> bool {
    let [usage, high] = usage.to_le_bytes();
    if high != 0 {
        return false;
    }
    let mut index = 0;
    while index < ALLOWLISTED_HID_USAGE_RANGES.len() {
        if ALLOWLISTED_HID_USAGE_RANGES[index].contains(usage) {
            return true;
        }
        index += 1;
    }
    false
}

/// Desktop shortcut convention selected by the active native backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopShortcutPlatform {
    Windows,
    MacOs,
    Linux,
}

/// Maximum physical keys in a reviewed logical-command chord.
pub const MAX_CANONICAL_CHORD_KEYS: usize = 3;

/// Reviewed physical-key chord in deterministic press order.
///
/// Release order is always the reverse. Standard diagnostics redact the key
/// identities so a command stream cannot enter ordinary logs.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct CanonicalChord {
    usages: [u8; MAX_CANONICAL_CHORD_KEYS],
    length: u8,
}

impl CanonicalChord {
    const fn new(usages: [u8; MAX_CANONICAL_CHORD_KEYS], length: u8) -> Self {
        Self { usages, length }
    }

    /// Returns the number of physical keys in the chord.
    #[must_use]
    pub const fn len(self) -> usize {
        self.length as usize
    }

    /// Returns whether the chord contains no keys.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.length == 0
    }

    /// Iterates keys in deterministic down order.
    pub fn press_order(self) -> impl DoubleEndedIterator<Item = PhysicalKey> {
        self.usages
            .into_iter()
            .take(self.len())
            .map(PhysicalKey::from_allowlisted_usage)
    }

    /// Iterates keys in deterministic up order.
    pub fn release_order(self) -> impl Iterator<Item = PhysicalKey> {
        self.press_order().rev()
    }
}

impl fmt::Debug for CanonicalChord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CanonicalChord")
            .field("key_count", &self.length)
            .field("keys", &"<redacted>")
            .finish_non_exhaustive()
    }
}

/// Closed command list used to prove every platform table is exhaustive.
pub const LOGICAL_COMMANDS: [LogicalCommand; 11] = [
    LogicalCommand::Copy,
    LogicalCommand::Cut,
    LogicalCommand::Paste,
    LogicalCommand::Undo,
    LogicalCommand::Redo,
    LogicalCommand::SelectAll,
    LogicalCommand::Find,
    LogicalCommand::Save,
    LogicalCommand::CloseTab,
    LogicalCommand::SwitchApplication,
    LogicalCommand::TaskManagerEquivalent,
];

const A: u8 = 0x04;
const C: u8 = 0x06;
const F: u8 = 0x09;
const S: u8 = 0x16;
const V: u8 = 0x19;
const W: u8 = 0x1A;
const X: u8 = 0x1B;
const Y: u8 = 0x1C;
const Z: u8 = 0x1D;
const ESCAPE: u8 = 0x29;
const TAB: u8 = 0x2B;
const LEFT_CONTROL: u8 = 0xE0;
const LEFT_SHIFT: u8 = 0xE1;
const LEFT_ALT: u8 = 0xE2;
const LEFT_GUI: u8 = 0xE3;

const fn chord2(modifier: u8, key: u8) -> CanonicalChord {
    CanonicalChord::new([modifier, key, 0], 2)
}

const fn chord3(first_modifier: u8, second_modifier: u8, key: u8) -> CanonicalChord {
    CanonicalChord::new([first_modifier, second_modifier, key], 3)
}

/// Returns the reviewed conventional chord for a logical command.
///
/// A returned chord is a translation plan, not a claim that the active
/// backend or focused application supports it. The backend advertises a
/// logical command only after its native mapping is validated. Linux has no
/// desktop-independent task-manager equivalent, so that entry is absent.
#[must_use]
pub const fn canonical_logical_chord(
    platform: DesktopShortcutPlatform,
    command: LogicalCommand,
) -> Option<CanonicalChord> {
    let primary = match platform {
        DesktopShortcutPlatform::MacOs => LEFT_GUI,
        DesktopShortcutPlatform::Windows | DesktopShortcutPlatform::Linux => LEFT_CONTROL,
    };
    match command {
        LogicalCommand::Copy => Some(chord2(primary, C)),
        LogicalCommand::Cut => Some(chord2(primary, X)),
        LogicalCommand::Paste => Some(chord2(primary, V)),
        LogicalCommand::Undo => Some(chord2(primary, Z)),
        LogicalCommand::Redo => match platform {
            DesktopShortcutPlatform::Windows => Some(chord2(LEFT_CONTROL, Y)),
            DesktopShortcutPlatform::MacOs => Some(chord3(LEFT_GUI, LEFT_SHIFT, Z)),
            DesktopShortcutPlatform::Linux => Some(chord3(LEFT_CONTROL, LEFT_SHIFT, Z)),
        },
        LogicalCommand::SelectAll => Some(chord2(primary, A)),
        LogicalCommand::Find => Some(chord2(primary, F)),
        LogicalCommand::Save => Some(chord2(primary, S)),
        LogicalCommand::CloseTab => Some(chord2(primary, W)),
        LogicalCommand::SwitchApplication => match platform {
            DesktopShortcutPlatform::MacOs => Some(chord2(LEFT_GUI, TAB)),
            DesktopShortcutPlatform::Windows | DesktopShortcutPlatform::Linux => {
                Some(chord2(LEFT_ALT, TAB))
            }
        },
        LogicalCommand::TaskManagerEquivalent => match platform {
            DesktopShortcutPlatform::Windows => Some(chord3(LEFT_CONTROL, LEFT_SHIFT, ESCAPE)),
            DesktopShortcutPlatform::MacOs => Some(chord3(LEFT_ALT, LEFT_GUI, ESCAPE)),
            DesktopShortcutPlatform::Linux => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hid_allowlist_boundaries_are_exact_and_non_overlapping() {
        let accepted = [0x04, 0x65, 0x67, 0xA4, 0xB0, 0xDD, 0xE0, 0xE7];
        let rejected = [0x00, 0x03, 0x66, 0xA5, 0xAF, 0xDE, 0xDF, 0xE8, 0x100];
        for usage in accepted {
            assert!(is_allowlisted_keyboard_usage(usage));
        }
        for usage in rejected {
            assert!(!is_allowlisted_keyboard_usage(usage));
        }
        for windows in ALLOWLISTED_HID_USAGE_RANGES.windows(2) {
            assert!(windows[0].last < windows[1].first);
        }
    }

    #[test]
    fn every_logical_mapping_is_allowlisted_and_releases_in_reverse() {
        for platform in [
            DesktopShortcutPlatform::Windows,
            DesktopShortcutPlatform::MacOs,
            DesktopShortcutPlatform::Linux,
        ] {
            for command in LOGICAL_COMMANDS {
                let Some(chord) = canonical_logical_chord(platform, command) else {
                    assert_eq!(platform, DesktopShortcutPlatform::Linux);
                    assert_eq!(command, LogicalCommand::TaskManagerEquivalent);
                    continue;
                };
                assert!(!chord.is_empty());
                let pressed = chord.press_order().collect::<Vec<_>>();
                assert!(
                    pressed
                        .iter()
                        .all(|key| is_allowlisted_keyboard_usage(u16::from(key.usage())))
                );
                assert_eq!(
                    chord.release_order().collect::<Vec<_>>(),
                    pressed.into_iter().rev().collect::<Vec<_>>()
                );
            }
        }
    }

    #[test]
    fn platform_specific_chords_are_stable() {
        let usages = |platform, command| {
            canonical_logical_chord(platform, command)
                .expect("mapping")
                .press_order()
                .map(PhysicalKey::usage)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            usages(DesktopShortcutPlatform::Windows, LogicalCommand::Redo),
            [LEFT_CONTROL, Y]
        );
        assert_eq!(
            usages(DesktopShortcutPlatform::MacOs, LogicalCommand::Redo),
            [LEFT_GUI, LEFT_SHIFT, Z]
        );
        assert_eq!(
            usages(
                DesktopShortcutPlatform::Windows,
                LogicalCommand::TaskManagerEquivalent
            ),
            [LEFT_CONTROL, LEFT_SHIFT, ESCAPE]
        );
        assert_eq!(
            canonical_logical_chord(
                DesktopShortcutPlatform::Linux,
                LogicalCommand::TaskManagerEquivalent
            ),
            None
        );
    }

    #[test]
    fn chord_diagnostics_do_not_disclose_key_identities() {
        let chord = canonical_logical_chord(
            DesktopShortcutPlatform::Windows,
            LogicalCommand::TaskManagerEquivalent,
        )
        .expect("mapping");
        let debug = format!("{chord:?}");
        assert!(debug.contains("key_count"));
        for forbidden in ["224", "225", "41", "0xE0", "Escape"] {
            assert!(!debug.contains(forbidden));
        }
    }
}
