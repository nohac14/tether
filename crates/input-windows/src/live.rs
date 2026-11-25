//! Serialized Win32 input for the approved Windows test build.
#![allow(unsafe_code)]

use super::{WindowsEvent, WindowsMapper, WindowsProbe};
use std::mem::size_of;
use tether_input_core::{
    BackendCapabilities, BackendError, BackendErrorCategory as Error, BackendStatus, InputAction,
    InputBackend, OperationClass, PermissionState, ReleaseFailure, ReleaseReport, RetryScope,
    StatusRefreshReason,
};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_READOBJECTS, GetProcessWindowStation,
    GetThreadDesktop, GetUserObjectInformationW, OpenInputDesktop, UOI_NAME,
};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{INPUT, SendInput};

trait Sink: Send {
    fn active(&self) -> bool;
    fn insert(&mut self, events: &[WindowsEvent]) -> usize;
}

/// The system sink is used only through the dispatcher's worker.
pub struct Win32;

impl Sink for Win32 {
    fn active(&self) -> bool {
        active_default_desktop()
    }

    fn insert(&mut self, events: &[WindowsEvent]) -> usize {
        let records: Vec<INPUT> = events.iter().map(WindowsEvent::to_input).collect();
        let size = i32::try_from(size_of::<INPUT>()).expect("Win32 INPUT ABI size");
        // SAFETY: contiguous initialized INPUT records with the generated ABI.
        unsafe { SendInput(&records, size) as usize }
    }
}

/// Native backend with a separate ledger of key/button releases.
pub struct WindowsBackend<S = Win32> {
    sink: S,
    mapper: WindowsMapper,
    status: BackendStatus,
    ready_caps: BackendCapabilities,
    held: Vec<WindowsEvent>,
}

impl WindowsBackend {
    #[must_use]
    pub fn new() -> Self {
        Self::with_sink(Win32)
    }

    #[must_use]
    pub fn desktop_available() -> bool {
        active_default_desktop()
    }
}

impl Default for WindowsBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> WindowsBackend<S> {
    fn with_sink(sink: S) -> Self
    where
        S: Sink,
    {
        let mapper = WindowsMapper::from_probe(WindowsProbe::inert_ready());
        let mut status = mapper.status();
        status.capabilities.logical_commands.clear();
        status.capabilities.media_commands.clear();
        let ready_caps = status.capabilities.clone();
        let mut this = Self {
            sink,
            mapper,
            status,
            ready_caps,
            held: Vec::new(),
        };
        this.refresh_status(StatusRefreshReason::Startup);
        this
    }

    fn available(&mut self, active: bool) {
        let available = active && self.status.fault.is_none();
        if self.status.available == available {
            return;
        }
        self.status.revision = self.status.revision.saturating_add(1);
        self.status.available = available;
        self.status.permission = if available {
            PermissionState::NotRequired
        } else {
            PermissionState::Unavailable
        };
        self.status.capabilities = if available {
            self.ready_caps.clone()
        } else {
            BackendCapabilities::unavailable()
        };
    }

    fn fault(&mut self, category: Error) {
        self.status.fault = Some(category);
        self.available(false);
    }

    fn update_held(&mut self, events: &[WindowsEvent], complete: bool) {
        for event in events {
            let Some((release, down)) = release_event(*event) else {
                continue;
            };
            if down {
                if !self.held.contains(&release) {
                    self.held.push(release);
                }
            } else if complete {
                self.held.retain(|held| *held != release);
            }
        }
    }
}

impl<S: Sink> InputBackend for WindowsBackend<S> {
    fn status(&self) -> BackendStatus {
        self.status.clone()
    }

    fn refresh_status(&mut self, _: StatusRefreshReason) -> BackendStatus {
        self.available(self.sink.active());
        self.status()
    }

    fn execute(&mut self, action: &InputAction) -> Result<(), BackendError> {
        let class = action.class();
        if !self.sink.active() || self.status.fault.is_some() {
            self.available(false);
            return Err(failure(Error::InvalidPlatformState, class));
        }
        if self.status.validate_action(action).is_err() {
            return Err(failure(Error::Unsupported, class));
        }
        let events = self.mapper.map(action)?;
        if events.is_empty() {
            return Ok(());
        }
        if matches!(
            action,
            InputAction::MouseButton(tether_input_core::ButtonAction::Click { .. })
                | InputAction::TextEdit(_)
        ) && events
            .iter()
            .filter_map(|event| release_event(*event))
            .any(|(up, down)| down && self.held.contains(&up))
        {
            return Err(failure(Error::Unsupported, class));
        }
        let complete = self.sink.insert(&events) == events.len();
        self.update_held(&events, complete);
        if complete {
            Ok(())
        } else {
            // The API reports a count, without enough information to prove
            // which down events reached the queue. Retain possible releases.
            self.fault(Error::PartialStateChange);
            Err(failure(Error::PartialStateChange, class))
        }
    }

    fn release_all(&mut self) -> ReleaseReport {
        self.mapper.reset_fractional_state();
        let mut report = ReleaseReport::neutral();
        let active = self.sink.active();
        for modifier in [false, true] {
            for index in (0..self.held.len()).rev() {
                let event = self.held[index];
                let category = kind(event);
                if (category == ReleaseFailure::ModifierKey) != modifier {
                    continue;
                }
                report.attempted += 1;
                if active && self.sink.insert(&[event]) == 1 {
                    self.held.remove(index);
                    report.released += 1;
                } else {
                    report.failures.push(category);
                }
            }
        }
        report.neutral = self.held.is_empty();
        if !report.neutral {
            self.fault(Error::PartialStateChange);
        }
        report
    }
}

fn failure(category: Error, class: OperationClass) -> BackendError {
    BackendError::new(category, class, RetryScope::Never)
}

fn release_event(event: WindowsEvent) -> Option<(WindowsEvent, bool)> {
    use WindowsEvent::{MouseButton, RelativeMotion, ScanCode, UnicodeCodeUnit, VirtualKey, Wheel};
    Some(match event {
        RelativeMotion { .. } | Wheel { .. } => return None,
        MouseButton { flag, data } => {
            let down = matches!(flag, 0x0002 | 0x0008 | 0x0020 | 0x0080);
            (
                MouseButton {
                    flag: if down { flag << 1 } else { flag },
                    data,
                },
                down,
            )
        }
        ScanCode {
            scan_code,
            extended,
            down,
        } => (
            ScanCode {
                scan_code,
                extended,
                down: false,
            },
            down,
        ),
        UnicodeCodeUnit { code_unit, down } => (
            UnicodeCodeUnit {
                code_unit,
                down: false,
            },
            down,
        ),
        VirtualKey { virtual_key, down } => (
            VirtualKey {
                virtual_key,
                down: false,
            },
            down,
        ),
    })
}

fn kind(event: WindowsEvent) -> ReleaseFailure {
    match event {
        WindowsEvent::MouseButton { .. } => ReleaseFailure::MouseButton,
        WindowsEvent::ScanCode {
            scan_code: 0x1d | 0x2a | 0x36 | 0x38 | 0x5b | 0x5c,
            ..
        } => ReleaseFailure::ModifierKey,
        _ => ReleaseFailure::NonModifierKey,
    }
}

fn active_default_desktop() -> bool {
    // SAFETY: process station and thread desktop are borrowed handle results.
    // The input desktop is a separate owned handle closed below.
    let (station, thread, input) = unsafe {
        let Ok(station) = GetProcessWindowStation() else {
            return false;
        };
        let Ok(thread) = GetThreadDesktop(GetCurrentThreadId()) else {
            return false;
        };
        let Ok(input) = OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS)
        else {
            return false;
        };
        (station, thread, input)
    };
    let active = object_named(HANDLE(station.0), "WinSta0")
        && object_named(HANDLE(thread.0), "Default")
        && object_named(HANDLE(input.0), "Default");
    // SAFETY: closes the successful OpenInputDesktop result exactly once.
    let closed = unsafe { CloseDesktop(input) }.is_ok();
    active && closed
}

fn object_named(handle: HANDLE, expected: &str) -> bool {
    let mut result = [0u16; 32];
    // SAFETY: result is an initialized writable buffer of the supplied byte length.
    let read = unsafe {
        GetUserObjectInformationW(handle, UOI_NAME, Some(result.as_mut_ptr().cast()), 64, None)
    };
    read.is_ok()
        && result
            .iter()
            .take_while(|&&unit| unit != 0)
            .copied()
            .eq(expected.encode_utf16())
}

#[cfg(test)]
mod tests;
