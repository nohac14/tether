//! Opt-in Windows acceptance spike. Run only in an isolated interactive lab.

use std::process::ExitCode;

#[cfg(windows)]
fn main() -> ExitCode {
    const ACK: &str = "I_UNDERSTAND_TETHER_INJECTS_INPUT";
    let target = std::env::var("TETHER_MANUAL_TEST_TARGET");
    if std::env::var("TETHER_MANUAL_TEST_ACK").as_deref() != Ok(ACK)
        || target.as_deref() != Ok("isolated-windows-input")
        || std::env::args_os().nth(1).as_deref() != Some(std::ffi::OsStr::new("--move-pointer"))
    {
        eprintln!(
            "Guarded motion requires explicit opt-in in an isolated Windows session. It moves the workstation pointer."
        );
        return ExitCode::from(2);
    }
    eprintln!("Sending one relative mouse movement to the isolated Windows session.");
    motion()
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn motion() -> ExitCode {
    use tether_input_windows::WindowsEvent;
    use windows::Win32::UI::Input::KeyboardAndMouse::{INPUT, SendInput};

    let records = [WindowsEvent::RelativeMotion { dx: 1, dy: 0 }.to_input()];
    let size = i32::try_from(std::mem::size_of::<INPUT>()).expect("Win32 INPUT fits in i32");
    // SAFETY: the binding receives exactly one initialized INPUT with its
    // matching ABI size. Main requires explicit opt-in to manipulate input.
    let inserted = unsafe { SendInput(&records, size) };
    if inserted == 1 {
        println!("Windows accepted one relative motion record.");
        ExitCode::SUCCESS
    } else {
        eprintln!("Windows did not accept the relative motion record.");
        ExitCode::FAILURE
    }
}

#[cfg(not(windows))]
fn main() -> ExitCode {
    eprintln!("The guarded motion probe requires Windows.");
    ExitCode::from(2)
}
