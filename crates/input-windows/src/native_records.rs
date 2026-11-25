//! Win32 record construction. It is safe to exercise without injecting input.

use super::WindowsEvent;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, KEYEVENTF_UNICODE,
    MOUSE_EVENT_FLAGS, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_MOVE, MOUSEEVENTF_WHEEL, MOUSEINPUT,
    VIRTUAL_KEY,
};

impl WindowsEvent {
    /// Converts an ordered mapper record to its Win32 `INPUT` ABI shape.
    #[must_use]
    pub fn to_input(&self) -> INPUT {
        match *self {
            Self::RelativeMotion { dx, dy } => mouse(dx, dy, 0, MOUSEEVENTF_MOVE),
            Self::MouseButton { flag, data } => mouse(0, 0, data, MOUSE_EVENT_FLAGS(flag)),
            Self::Wheel { horizontal, delta } => {
                let flags = if horizontal {
                    MOUSEEVENTF_HWHEEL
                } else {
                    MOUSEEVENTF_WHEEL
                };
                mouse(0, 0, u32::from_ne_bytes(delta.to_ne_bytes()), flags)
            }
            Self::ScanCode {
                scan_code,
                extended,
                down,
            } => {
                let flags = if extended {
                    KEYEVENTF_SCANCODE | KEYEVENTF_EXTENDEDKEY
                } else {
                    KEYEVENTF_SCANCODE
                };
                keyboard(0, scan_code, flags, down)
            }
            Self::UnicodeCodeUnit { code_unit, down } => {
                keyboard(0, code_unit, KEYEVENTF_UNICODE, down)
            }
            Self::VirtualKey { virtual_key, down } => {
                keyboard(virtual_key, 0, KEYBD_EVENT_FLAGS(0), down)
            }
        }
    }
}

fn mouse(dx: i32, dy: i32, data: u32, flags: MOUSE_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: data,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn keyboard(virtual_key: u16, scan: u16, mut flags: KEYBD_EVENT_FLAGS, down: bool) -> INPUT {
    if !down {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(virtual_key),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WindowsMapper, WindowsProbe};
    use tether_input_core::{ButtonAction, InputAction, MouseButton, SensitiveText};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, VK_VOLUME_MUTE,
    };

    #[allow(unsafe_code)]
    fn fields_mouse(input: INPUT) -> MOUSEINPUT {
        assert_eq!(input.r#type, INPUT_MOUSE);
        // SAFETY: the only callers feed the constructor's mouse branch,
        // which initializes the active `mi` union member.
        unsafe { input.Anonymous.mi }
    }

    #[allow(unsafe_code)]
    fn fields_key(input: INPUT) -> KEYBDINPUT {
        assert_eq!(input.r#type, INPUT_KEYBOARD);
        // SAFETY: the only callers feed the constructor's keyboard branch,
        // which initializes the active `ki` union member.
        unsafe { input.Anonymous.ki }
    }

    #[test]
    fn relative_motion_and_both_signed_wheel_axes_have_distinct_flags() {
        let moved = fields_mouse(WindowsEvent::RelativeMotion { dx: -18, dy: 23 }.to_input());
        assert_eq!(
            (
                moved.dx,
                moved.dy,
                moved.mouseData,
                moved.time,
                moved.dwExtraInfo
            ),
            (-18, 23, 0, 0, 0)
        );
        assert_eq!(moved.dwFlags, MOUSEEVENTF_MOVE);
        for (horizontal, expected) in [(true, MOUSEEVENTF_HWHEEL), (false, MOUSEEVENTF_WHEEL)] {
            let wheel = fields_mouse(
                WindowsEvent::Wheel {
                    horizontal,
                    delta: -120,
                }
                .to_input(),
            );
            assert_eq!(wheel.dwFlags, expected);
            assert_eq!(i32::from_ne_bytes(wheel.mouseData.to_ne_bytes()), -120);
        }
    }

    #[test]
    fn additional_buttons_lower_to_separate_down_and_up_records() {
        let mut mapper = WindowsMapper::from_probe(WindowsProbe::inert_ready());
        for (button, code) in [(MouseButton::Back, 1), (MouseButton::Forward, 2)] {
            for (action, expected) in [
                (ButtonAction::Down(button), MOUSEEVENTF_XDOWN),
                (ButtonAction::Up(button), MOUSEEVENTF_XUP),
            ] {
                let plan = mapper.map(&InputAction::MouseButton(action)).unwrap();
                let button = fields_mouse(plan[0].to_input());
                assert_eq!(button.dwFlags, expected);
                assert_eq!(button.mouseData, code);
            }
        }
    }

    #[test]
    fn scan_virtual_key_and_utf16_abi_fields_preserve_modes_and_order() {
        let scan = fields_key(
            WindowsEvent::ScanCode {
                scan_code: 0x4b,
                extended: true,
                down: false,
            }
            .to_input(),
        );
        assert_eq!(scan.wVk, VIRTUAL_KEY(0));
        assert_eq!(scan.wScan, 0x4b);
        assert_eq!(
            scan.dwFlags,
            KEYEVENTF_SCANCODE | KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP
        );
        let media = fields_key(
            WindowsEvent::VirtualKey {
                virtual_key: VK_VOLUME_MUTE.0,
                down: true,
            }
            .to_input(),
        );
        assert_eq!(media.wVk, VK_VOLUME_MUTE);
        assert_eq!((media.wScan, media.dwFlags.0), (0, 0));

        let text = SensitiveText::new("A🐾".to_owned()).unwrap();
        let plan = WindowsMapper::from_probe(WindowsProbe::inert_ready())
            .map(&InputAction::TextInsert(text))
            .unwrap();
        let expected: Vec<_> = "A🐾"
            .encode_utf16()
            .flat_map(|unit| {
                [
                    (unit, KEYEVENTF_UNICODE),
                    (unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP),
                ]
            })
            .collect();
        let observed: Vec<_> = plan
            .iter()
            .map(|record| {
                let stored = fields_key(record.to_input());
                assert_eq!(stored.wVk, VIRTUAL_KEY(0));
                (stored.wScan, stored.dwFlags)
            })
            .collect();
        assert_eq!(observed, expected);
    }
}
