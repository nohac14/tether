use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

/// Maximum persisted interface approvals.
pub const MAX_APPROVED_INTERFACES: usize = 32;

/// Maximum UTF-8 bytes in one opaque interface identity.
pub const MAX_INTERFACE_ID_BYTES: usize = 256;

/// Validation failure for typed desktop settings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingsValidationError {
    EmptyInterfaceId,
    InterfaceIdTooLong,
    TooManyApprovedInterfaces,
    DuplicateInterfaceId,
    InvalidServerPort,
    InvalidStartupNetwork,
}

impl fmt::Display for SettingsValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyInterfaceId => "approved interface identity is empty",
            Self::InterfaceIdTooLong => "approved interface identity is too long",
            Self::TooManyApprovedInterfaces => "too many approved interface identities",
            Self::DuplicateInterfaceId => "approved interface identity is duplicated",
            Self::InvalidServerPort => "server port must be non-zero",
            Self::InvalidStartupNetwork => {
                "automatic listening needs one interface and a user port"
            }
        })
    }
}

impl std::error::Error for SettingsValidationError {}

/// Opaque desktop-derived interface identity with redacted diagnostics.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ApprovedInterfaceId(String);

impl ApprovedInterfaceId {
    /// Validates an opaque stable identity.
    ///
    /// # Errors
    ///
    /// Returns a closed validation category for empty or oversized identities.
    pub fn new(value: String) -> Result<Self, SettingsValidationError> {
        if value.is_empty() {
            Err(SettingsValidationError::EmptyInterfaceId)
        } else if value.len() > MAX_INTERFACE_ID_BYTES {
            Err(SettingsValidationError::InterfaceIdTooLong)
        } else {
            Ok(Self(value))
        }
    }

    /// Exposes the identity only to the local interface-selection boundary.
    #[must_use]
    pub fn expose_for_matching(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApprovedInterfaceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApprovedInterfaceId(<redacted>)")
    }
}

impl<'de> Deserialize<'de> for ApprovedInterfaceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Persisted diagnostic verbosity. Input content remains forbidden at all levels.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
}

/// Validated current desktop-settings payload.
#[derive(Clone, Eq, PartialEq)]
pub struct DesktopSettings {
    server_port: Option<u16>,
    approved_interfaces: Vec<ApprovedInterfaceId>,
    auto_listen: bool,
    start_at_login: bool,
    log_level: LogLevel,
    startup_network: Option<NetworkFingerprint>,
}

/// Bounded, opaque hash of local network continuity evidence.
#[derive(Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(transparent)]
pub struct NetworkFingerprint([u8; 32]);

impl NetworkFingerprint {
    #[must_use]
    pub const fn new(digest: [u8; 32]) -> Self {
        Self(digest)
    }
}

impl fmt::Debug for NetworkFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("NetworkFingerprint(<redacted>)")
    }
}

impl DesktopSettings {
    /// Builds validated settings. A missing port means the networking component
    /// has not yet selected and persisted its reviewed product port.
    ///
    /// # Errors
    ///
    /// Rejects zero ports, collection overflow, and duplicate identities.
    pub fn new(
        server_port: Option<u16>,
        mut approved_interfaces: Vec<ApprovedInterfaceId>,
        auto_listen: bool,
        start_at_login: bool,
        log_level: LogLevel,
    ) -> Result<Self, SettingsValidationError> {
        if server_port == Some(0) {
            return Err(SettingsValidationError::InvalidServerPort);
        }
        if approved_interfaces.len() > MAX_APPROVED_INTERFACES {
            return Err(SettingsValidationError::TooManyApprovedInterfaces);
        }
        if approved_interfaces
            .iter()
            .any(|identity| identity.0.is_empty())
        {
            return Err(SettingsValidationError::EmptyInterfaceId);
        }
        if approved_interfaces
            .iter()
            .any(|identity| identity.0.len() > MAX_INTERFACE_ID_BYTES)
        {
            return Err(SettingsValidationError::InterfaceIdTooLong);
        }
        approved_interfaces.sort();
        if approved_interfaces
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        {
            return Err(SettingsValidationError::DuplicateInterfaceId);
        }
        Ok(Self {
            server_port,
            approved_interfaces,
            auto_listen,
            start_at_login,
            log_level,
            startup_network: None,
        })
    }

    #[must_use]
    pub const fn server_port(&self) -> Option<u16> {
        self.server_port
    }

    #[must_use]
    pub fn approved_interfaces(&self) -> &[ApprovedInterfaceId] {
        &self.approved_interfaces
    }

    #[must_use]
    pub const fn auto_listen(&self) -> bool {
        self.auto_listen
    }

    #[must_use]
    pub const fn start_at_login(&self) -> bool {
        self.start_at_login
    }

    #[must_use]
    pub const fn log_level(&self) -> LogLevel {
        self.log_level
    }

    #[must_use]
    pub const fn startup_network(&self) -> Option<NetworkFingerprint> {
        self.startup_network
    }

    /// # Errors
    /// A saved network requires exactly one interface and a non-privileged port.
    pub fn with_startup_network(
        mut self,
        network: Option<NetworkFingerprint>,
    ) -> Result<Self, SettingsValidationError> {
        if network.is_some()
            && (self.approved_interfaces.len() != 1
                || self.server_port.is_none_or(|port| port < 1024))
        {
            return Err(SettingsValidationError::InvalidStartupNetwork);
        }
        self.startup_network = network;
        self.auto_listen = network.is_some();
        Ok(self)
    }
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            server_port: None,
            approved_interfaces: Vec::new(),
            auto_listen: false,
            start_at_login: false,
            log_level: LogLevel::Info,
            startup_network: None,
        }
    }
}

impl fmt::Debug for DesktopSettings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DesktopSettings")
            .field("server_port_configured", &self.server_port.is_some())
            .field("approved_interface_count", &self.approved_interfaces.len())
            .field("auto_listen", &self.auto_listen)
            .field("start_at_login", &self.start_at_login)
            .field("log_level", &self.log_level)
            .field("startup_network", &self.startup_network)
            .finish()
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SettingsPayloadV2 {
    pub server_port: Option<u16>,
    pub approved_interface_ids: Vec<ApprovedInterfaceId>,
    pub auto_listen: bool,
    pub start_at_login: bool,
    pub log_level: LogLevel,
}

impl From<&DesktopSettings> for SettingsPayloadV2 {
    fn from(settings: &DesktopSettings) -> Self {
        Self {
            server_port: settings.server_port,
            approved_interface_ids: settings.approved_interfaces.clone(),
            auto_listen: settings.auto_listen,
            start_at_login: settings.start_at_login,
            log_level: settings.log_level,
        }
    }
}

impl TryFrom<SettingsPayloadV2> for DesktopSettings {
    type Error = SettingsValidationError;

    fn try_from(payload: SettingsPayloadV2) -> Result<Self, Self::Error> {
        Self::new(
            payload.server_port,
            payload.approved_interface_ids,
            payload.auto_listen,
            payload.start_at_login,
            payload.log_level,
        )
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SettingsPayloadV1 {
    pub server_port: Option<u16>,
    pub approved_interface_ids: Vec<ApprovedInterfaceId>,
    pub auto_listen: bool,
}

impl TryFrom<SettingsPayloadV1> for DesktopSettings {
    type Error = SettingsValidationError;

    fn try_from(payload: SettingsPayloadV1) -> Result<Self, Self::Error> {
        Self::new(
            payload.server_port,
            payload.approved_interface_ids,
            payload.auto_listen,
            false,
            LogLevel::Info,
        )
    }
}
