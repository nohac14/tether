//! Bounded local HTTP and WebSocket transport boundary.

pub mod assets;
pub mod http1;
pub mod interfaces;
pub mod supervisor;
#[cfg(windows)]
pub mod windows_tcp;
