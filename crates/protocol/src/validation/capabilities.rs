use super::{Limits, ValidationError, mapping, required};
use crate::wire;
use std::collections::BTreeSet;
use tether_input_core as input;

/// Converts a complete, coherent wire snapshot into platform-neutral capabilities.
///
/// # Errors
/// Rejects unknown/duplicate enums, stale revisions, changed limits and dishonest
/// availability claims. Does not certify individual native HID mappings.
pub fn validate_capabilities(
    value: &wire::CapabilitySnapshot,
    limits: Limits,
    minimum_revision: u64,
) -> Result<input::BackendCapabilities, ValidationError> {
    if value.revision == 0 || value.revision < minimum_revision {
        return Err(ValidationError::InvalidContext);
    }
    if Limits::from_wire(required(value.limits.as_ref())?)? != limits {
        return Err(ValidationError::InvalidLimits);
    }
    let permission = permission(value.permission_state)?;
    if value.user_authorization_required
        != matches!(
            permission,
            input::PermissionState::Denied | input::PermissionState::PromptRequired
        )
    {
        return Err(ValidationError::MalformedMessage);
    }
    let caps = input::BackendCapabilities {
        relative_pointer: state(value.pointer_available),
        atomic_click: state(value.atomic_click_available),
        vertical_scroll: state(value.vertical_scroll_available),
        horizontal_scroll: state(value.horizontal_scroll_available),
        physical_keys: state(value.physical_keys_available),
        mouse_buttons: collect(&value.mouse_buttons, mapping::button)?,
        text_modes: collect(&value.text_modes, mapping::text_mode)?,
        logical_commands: collect(&value.logical_commands, mapping::logical)?,
        media_commands: collect(&value.media_commands, mapping::media)?,
    };
    if (!value.backend_initialized
        || !matches!(
            permission,
            input::PermissionState::Granted | input::PermissionState::NotRequired
        ))
        && !caps.is_empty()
    {
        return Err(ValidationError::MalformedMessage);
    }
    if value.atomic_click_available && caps.mouse_buttons.is_empty() {
        return Err(ValidationError::MalformedMessage);
    }
    Ok(caps)
}

fn state(value: bool) -> input::CapabilityState {
    if value {
        input::CapabilityState::Available
    } else {
        input::CapabilityState::Unavailable
    }
}

fn collect<T: Ord>(
    values: &[i32],
    convert: fn(i32) -> Result<T, ValidationError>,
) -> Result<BTreeSet<T>, ValidationError> {
    let mut set = BTreeSet::new();
    for &value in values {
        if !set.insert(convert(value)?) {
            return Err(ValidationError::Unsupported);
        }
    }
    Ok(set)
}

fn permission(value: i32) -> Result<input::PermissionState, ValidationError> {
    match wire::PermissionState::try_from(value) {
        Ok(wire::PermissionState::NotRequired) => Ok(input::PermissionState::NotRequired),
        Ok(wire::PermissionState::Granted) => Ok(input::PermissionState::Granted),
        Ok(wire::PermissionState::Denied) => Ok(input::PermissionState::Denied),
        Ok(wire::PermissionState::PromptRequired) => Ok(input::PermissionState::PromptRequired),
        Ok(wire::PermissionState::Unavailable) => Ok(input::PermissionState::Unavailable),
        _ => Err(ValidationError::Unsupported),
    }
}

/// Builds a conservative snapshot from cached native facts, with no native calls.
///
/// # Errors
/// Rejects missing revisions and inconsistent backend capability claims.
pub fn capability_snapshot(
    status: &input::BackendStatus,
    limits: Limits,
) -> Result<wire::CapabilitySnapshot, ValidationError> {
    let permission = match status.permission {
        input::PermissionState::NotRequired => wire::PermissionState::NotRequired,
        input::PermissionState::Granted => wire::PermissionState::Granted,
        input::PermissionState::Denied => wire::PermissionState::Denied,
        input::PermissionState::PromptRequired => wire::PermissionState::PromptRequired,
        input::PermissionState::Unavailable | input::PermissionState::TransientlyUnknown => {
            wire::PermissionState::Unavailable
        }
    };
    let empty = input::BackendCapabilities::unavailable();
    let caps = if status.initialized
        && status.available
        && status.fault.is_none()
        && matches!(
            status.permission,
            input::PermissionState::Granted | input::PermissionState::NotRequired
        ) {
        &status.capabilities
    } else {
        &empty
    };
    let snapshot = wire::CapabilitySnapshot {
        revision: status.revision,
        backend_initialized: status.initialized,
        permission_state: permission.into(),
        user_authorization_required: matches!(
            permission,
            wire::PermissionState::Denied | wire::PermissionState::PromptRequired
        ),
        pointer_available: caps.relative_pointer.is_available(),
        physical_keys_available: caps.physical_keys.is_available(),
        vertical_scroll_available: caps.vertical_scroll.is_available(),
        horizontal_scroll_available: caps.horizontal_scroll.is_available(),
        atomic_click_available: caps.atomic_click.is_available(),
        mouse_buttons: caps
            .mouse_buttons
            .iter()
            .copied()
            .map(mapping::wire_button)
            .collect(),
        text_modes: caps
            .text_modes
            .iter()
            .copied()
            .map(mapping::wire_text_mode)
            .collect(),
        logical_commands: caps
            .logical_commands
            .iter()
            .copied()
            .map(mapping::wire_logical)
            .collect(),
        media_commands: caps
            .media_commands
            .iter()
            .copied()
            .map(mapping::wire_media)
            .collect(),
        limits: Some(limits.to_wire()),
    };
    validate_capabilities(&snapshot, limits, 1)?;
    Ok(snapshot)
}
