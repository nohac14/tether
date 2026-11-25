//! Receive local Windows lock, disconnect, suspend and logoff at the UI boundary.
#![allow(unsafe_code)]

use std::io;
use tether_agent_core::windows_test::StopHandle;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::RemoteDesktop::{
    NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification, WTSUnRegisterSessionNotification,
};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    MB_ICONERROR, MB_OK, MessageBoxW, PBT_APMSUSPEND, WM_NCDESTROY, WM_POWERBROADCAST,
    WM_QUERYENDSESSION, WM_WTSSESSION_CHANGE,
};
use windows::core::w;

const SUBCLASS_ID: usize = 1;

pub(crate) fn show_startup_error() {
    // SAFETY: both strings are static UTF-16 constants, and no owner is needed.
    unsafe {
        MessageBoxW(
            None,
            w!(
                "Tether could not open. Verify that Microsoft Edge WebView2 Runtime is installed, then try again from your unlocked desktop."
            ),
            w!("Tether"),
            MB_OK | MB_ICONERROR,
        );
    }
}

pub(super) fn attach(hwnd: HWND, stop: StopHandle) -> io::Result<()> {
    let data = Box::into_raw(Box::new(stop));
    // SAFETY: called on this window's owner thread. The box stays live until
    // the subclass is removed on this thread or receives WM_NCDESTROY.
    unsafe {
        if !SetWindowSubclass(hwnd, Some(receive), SUBCLASS_ID, data as usize).as_bool() {
            drop(Box::from_raw(data));
            return Err(io::Error::other("Could not monitor the Windows session"));
        }
        if WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION).is_err() {
            if RemoveWindowSubclass(hwnd, Some(receive), SUBCLASS_ID).as_bool() {
                drop(Box::from_raw(data));
            }
            return Err(io::Error::other("Could not monitor Windows session lock"));
        }
    }
    Ok(())
}

unsafe extern "system" fn receive(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    let data = data as *mut StopHandle;
    // WTS console/remote disconnect, logoff and lock are the stop categories.
    let stopping = match message {
        WM_WTSSESSION_CHANGE => matches!(wparam.0, 2 | 4 | 6 | 7),
        WM_POWERBROADCAST => wparam.0 == PBT_APMSUSPEND as usize,
        WM_QUERYENDSESSION | WM_NCDESTROY => true,
        _ => false,
    };
    if stopping {
        // SAFETY: the subclass owns this still-live box, including on the
        // first WM_NCDESTROY. Requests only touch thread-safe core signals.
        unsafe { (&*data).stop() };
    }
    if message == WM_NCDESTROY {
        // SAFETY: remove before freeing the callback data. This final window
        // message runs synchronously on the creating GUI thread.
        unsafe {
            let _ = WTSUnRegisterSessionNotification(hwnd);
            if RemoveWindowSubclass(hwnd, Some(receive), id).as_bool() {
                drop(Box::from_raw(data));
            }
        }
    }
    // SAFETY: forward the unmodified Win32 message to its normal handlers.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
