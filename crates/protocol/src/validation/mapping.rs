//! Closed semantic mappings; numeric discriminants never reach a native backend.

use super::ValidationError;
use crate::wire;
use tether_input_core as input;

macro_rules! mapping {
    ($decode:ident, $encode:ident, $wire:ident, $domain:ident, {$($w:ident => $d:ident),+ $(,)?}) => {
        pub(super) fn $decode(value: i32) -> Result<input::$domain, ValidationError> {
            match wire::$wire::try_from(value) {
                $(Ok(wire::$wire::$w) => Ok(input::$domain::$d),)+
                _ => Err(ValidationError::Unsupported),
            }
        }
        pub(super) fn $encode(value: input::$domain) -> i32 {
            match value { $(input::$domain::$d => wire::$wire::$w.into(),)+ }
        }
    };
}

mapping!(button, wire_button, MouseButton, MouseButton, {
    Left => Left, Right => Right, Middle => Middle, Back => Back, Forward => Forward
});
mapping!(text_mode, wire_text_mode, TextMode, TextMode, {
    ActiveLayoutMappable => ActiveLayoutMappable,
    NativeUnicodeBestEffort => NativeUnicodeBestEffort, EiUtf8 => EiUtf8
});
mapping!(logical, wire_logical, LogicalCommand, LogicalCommand, {
    Copy => Copy, Cut => Cut, PasteShortcut => Paste, Undo => Undo, Redo => Redo,
    SelectAll => SelectAll, Save => Save, Find => Find, CloseTab => CloseTab,
    SwitchApplication => SwitchApplication, TaskManagerEquivalent => TaskManagerEquivalent
});
mapping!(media, wire_media, MediaCommand, MediaCommand, {
    PlayPause => PlayPause, NextTrack => NextTrack, PreviousTrack => PreviousTrack,
    VolumeUp => VolumeUp, VolumeDown => VolumeDown, Mute => Mute
});

pub(super) fn edit(value: i32) -> Result<input::TextEditCommand, ValidationError> {
    match wire::TextEditCommand::try_from(value) {
        Ok(wire::TextEditCommand::DeleteBackward) => Ok(input::TextEditCommand::DeleteBackward),
        Ok(wire::TextEditCommand::DeleteForward) => Ok(input::TextEditCommand::DeleteForward),
        Ok(wire::TextEditCommand::InsertLineBreak) => Ok(input::TextEditCommand::InsertLineBreak),
        _ => Err(ValidationError::Unsupported),
    }
}
