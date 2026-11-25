//! Application lifecycle and service orchestration boundary.

pub mod control;
pub mod listener_safety;
pub mod pairing;
#[cfg(all(windows, feature = "windows-test"))]
pub mod windows_test;
