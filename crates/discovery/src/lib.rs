//! LAN interface discovery and mDNS/DNS-SD boundary.

mod plan;
pub use plan::{Advertisement, DiscoveryError, SERVICE_TYPE, WindowsRegistration};
