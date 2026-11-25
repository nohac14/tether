//! Validated interface inventory, foreground approval and listener planning.
//!
//! Native providers are responsible for translating actual OS records. The core
//! owns this inventory and refreshes it before binding and accepting sockets.

mod prefix;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::WindowsInterfaceProvider;

use std::collections::BTreeMap;
use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::num::{NonZeroU16, NonZeroU32, NonZeroU64};
use std::sync::atomic::{AtomicU64, Ordering};
use tether_config::{
    ApprovedInterfaceId, DesktopSettings, MAX_APPROVED_INTERFACES, NetworkFingerprint,
};

pub const MAX_INTERFACES: usize = 64;
pub const MAX_ADDRESSES_PER_INTERFACE: usize = 32;
pub const MAX_LISTENERS: usize = 64;
const MAX_DISPLAY_NAME_BYTES: usize = 128;
static NEXT_REVISION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterfaceError {
    InvalidName,
    InvalidPrefix,
    InventoryLimit,
    DuplicateInterface,
    DuplicateAddress,
    RevisionExhausted,
    StaleInventory,
    InvalidSelection,
    AddressUnavailable,
    ListenerLimit,
    ProviderUnavailable,
    ProviderChanged,
    ProviderTimedOut,
    RefreshInProgress,
    RefreshNotStarted,
    AutoListenUnavailable,
}

impl fmt::Display for InterfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidName => "invalid interface display name",
            Self::InvalidPrefix => "invalid assigned network prefix",
            Self::InventoryLimit => "interface inventory exceeds its limits",
            Self::DuplicateInterface => "interface inventory has a duplicate identity",
            Self::DuplicateAddress => {
                "an address occurs more than once in the selection or interface"
            }
            Self::RevisionExhausted => "interface inventory revision exhausted",
            Self::StaleInventory => "interface inventory changed",
            Self::InvalidSelection => "invalid interface selection",
            Self::AddressUnavailable => "a selected interface has no usable local address",
            Self::ListenerLimit => "interface selection exceeds the listener limit",
            Self::ProviderUnavailable => {
                "the operating system could not provide a complete interface inventory"
            }
            Self::ProviderChanged => "operating system interface records changed during collection",
            Self::ProviderTimedOut => "the operating system interface query timed out",
            Self::RefreshInProgress => "an interface refresh is already in progress",
            Self::RefreshNotStarted => "no interface refresh is in progress",
            Self::AutoListenUnavailable => {
                "automatic listening requires confirmed network continuity and stable approvals"
            }
        })
    }
}

impl std::error::Error for InterfaceError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityPersistence {
    Stable,
    SessionOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterfaceKind {
    Ethernet,
    WiFi,
    Loopback,
    Tunnel,
    Virtual,
    Bridge,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkState {
    Up,
    PointToPoint,
    Down,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkProfile {
    Private,
    Public,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressState {
    Preferred,
    Temporary,
    Ineligible,
    Unknown,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct AssignedAddress {
    address: IpAddr,
    prefix_len: u8,
    state: AddressState,
}

impl AssignedAddress {
    /// Validates the prefix reported by the provider and retains its eligibility.
    ///
    /// # Errors
    /// Returns an error for a zero or out-of-range prefix.
    pub fn new(
        address: IpAddr,
        prefix_len: u8,
        state: AddressState,
    ) -> Result<Self, InterfaceError> {
        let max_prefix = if address.is_ipv4() { 32 } else { 128 };
        if !(1..=max_prefix).contains(&prefix_len) {
            return Err(InterfaceError::InvalidPrefix);
        }
        Ok(Self {
            address,
            prefix_len,
            state,
        })
    }

    #[must_use]
    pub const fn address(&self) -> IpAddr {
        self.address
    }

    #[must_use]
    pub const fn prefix_len(&self) -> u8 {
        self.prefix_len
    }

    #[must_use]
    pub const fn state(&self) -> AddressState {
        self.state
    }

    #[must_use]
    pub fn is_usable(&self) -> bool {
        self.state == AddressState::Preferred
            && prefix::is_usable_host(self.address, self.prefix_len)
    }
}

/// Provider classification of the interface. Labels never confer authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterfaceStatus {
    pub kind: InterfaceKind,
    pub link: LinkState,
    pub profile: NetworkProfile,
}

/// Current OS indexes for change detection. The opaque ID carries persistence.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct InterfaceIndexes {
    pub ipv4: Option<NonZeroU32>,
    pub ipv6: Option<NonZeroU32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultExclusion {
    LinkUnavailable,
    NonDefaultKind,
    PointToPoint,
    PublicProfile,
    UnknownProfile,
    NoUsableAddress,
}

#[derive(Clone, Eq, PartialEq)]
pub struct Interface {
    id: ApprovedInterfaceId,
    name: Box<str>,
    persistence: IdentityPersistence,
    indexes: InterfaceIndexes,
    status: InterfaceStatus,
    addresses: Vec<AssignedAddress>,
}

impl Interface {
    /// Creates a bounded, canonical inventory record.
    ///
    /// # Errors
    /// Rejects invalid display names, too many addresses or a repeated literal
    /// address. Providers must supply the same complete record on every poll.
    pub fn new(
        id: ApprovedInterfaceId,
        name: &str,
        persistence: IdentityPersistence,
        indexes: InterfaceIndexes,
        status: InterfaceStatus,
        mut addresses: Vec<AssignedAddress>,
    ) -> Result<Self, InterfaceError> {
        if name.is_empty()
            || name.len() > MAX_DISPLAY_NAME_BYTES
            || name.chars().any(|ch| {
                ch.is_control()
                    || matches!(
                        ch,
                        '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
                    )
            })
        {
            return Err(InterfaceError::InvalidName);
        }
        if addresses.len() > MAX_ADDRESSES_PER_INTERFACE {
            return Err(InterfaceError::InventoryLimit);
        }
        addresses.sort_unstable_by_key(AssignedAddress::address);
        if addresses
            .windows(2)
            .any(|pair| pair[0].address == pair[1].address)
        {
            return Err(InterfaceError::DuplicateAddress);
        }
        Ok(Self {
            id,
            name: name.into(),
            persistence,
            indexes,
            status,
            addresses,
        })
    }

    #[must_use]
    pub const fn id(&self) -> &ApprovedInterfaceId {
        &self.id
    }

    /// Displays the name as untrusted text in local administration.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn persistence(&self) -> IdentityPersistence {
        self.persistence
    }

    #[must_use]
    pub const fn status(&self) -> InterfaceStatus {
        self.status
    }

    #[must_use]
    pub const fn indexes(&self) -> InterfaceIndexes {
        self.indexes
    }

    #[must_use]
    pub fn addresses(&self) -> &[AssignedAddress] {
        &self.addresses
    }

    /// Returns the first reason this interface is omitted from default proposals.
    #[must_use]
    pub fn default_exclusion(&self) -> Option<DefaultExclusion> {
        match self.status.link {
            LinkState::Down | LinkState::Unknown => return Some(DefaultExclusion::LinkUnavailable),
            LinkState::PointToPoint => return Some(DefaultExclusion::PointToPoint),
            LinkState::Up => {}
        }
        if !matches!(
            self.status.kind,
            InterfaceKind::Ethernet | InterfaceKind::WiFi
        ) {
            return Some(DefaultExclusion::NonDefaultKind);
        }
        match self.status.profile {
            NetworkProfile::Public => return Some(DefaultExclusion::PublicProfile),
            NetworkProfile::Unknown => return Some(DefaultExclusion::UnknownProfile),
            NetworkProfile::Private => {}
        }
        self.usable_addresses()
            .next()
            .is_none()
            .then_some(DefaultExclusion::NoUsableAddress)
    }

    fn usable_addresses(&self) -> impl Iterator<Item = AssignedAddress> + '_ {
        let link_available = matches!(self.status.link, LinkState::Up | LinkState::PointToPoint)
            && self.status.kind != InterfaceKind::Loopback;
        self.addresses
            .iter()
            .copied()
            .filter(move |address| link_available && address.is_usable())
    }
}

/// One complete inventory. The revision is allocated by the inventory owner.
pub struct InterfaceSnapshot {
    startup_networks: BTreeMap<ApprovedInterfaceId, NetworkFingerprint>,
    revision: u64,
    records: Vec<Interface>,
}

/// Preserve revisions across unchanged polls. A provider error must retire the
/// current snapshot, even if the next successful poll has identical rows.
#[derive(Default)]
pub struct InterfaceInventory {
    current: Option<InterfaceSnapshot>,
}

impl InterfaceInventory {
    #[must_use]
    pub const fn current(&self) -> Option<&InterfaceSnapshot> {
        self.current.as_ref()
    }

    pub fn retire(&mut self) {
        self.current = None;
    }

    /// Publishes a complete collection or retires it on provider/validation failure.
    ///
    /// # Errors
    /// Rejects over-limit inventories, duplicate records, provider failure and
    /// revision exhaustion. Every such failure retires the previous snapshot.
    pub fn refresh(
        &mut self,
        records: Result<Vec<Interface>, InterfaceError>,
    ) -> Result<&InterfaceSnapshot, InterfaceError> {
        let prepared = records.and_then(|mut records| {
            if records.len() > MAX_INTERFACES {
                return Err(InterfaceError::InventoryLimit);
            }
            records.sort_unstable_by(|a, b| a.id.cmp(&b.id));
            if records.windows(2).any(|pair| pair[0].id == pair[1].id) {
                return Err(InterfaceError::DuplicateInterface);
            }
            if let Some(previous) = &self.current
                && previous.records == records
            {
                return Ok(None);
            }
            let revision = NEXT_REVISION
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
                    old.checked_add(1)
                })
                .map_err(|_| InterfaceError::RevisionExhausted)?;
            Ok(Some(InterfaceSnapshot {
                revision,
                records,
                startup_networks: BTreeMap::new(),
            }))
        });
        match prepared {
            Ok(Some(snapshot)) => Ok(self.current.insert(snapshot)),
            Ok(None) => self.current.as_ref().ok_or(InterfaceError::StaleInventory),
            Err(error) => {
                self.retire();
                Err(error)
            }
        }
    }
}

impl InterfaceInventory {
    #[cfg(any(windows, test))]
    pub(crate) fn update_startup_networks(
        &mut self,
        networks: BTreeMap<ApprovedInterfaceId, NetworkFingerprint>,
    ) {
        if let Some(snapshot) = &mut self.current {
            snapshot.startup_networks = networks;
        }
    }
}

impl InterfaceSnapshot {
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub fn records(&self) -> &[Interface] {
        &self.records
    }

    #[must_use]
    pub fn startup_network(&self, id: &ApprovedInterfaceId) -> Option<NetworkFingerprint> {
        self.startup_networks.get(id).copied()
    }

    pub(crate) fn matches_network(
        &self,
        requirement: Option<&(ApprovedInterfaceId, NetworkFingerprint)>,
    ) -> bool {
        requirement.is_none_or(|(id, network)| self.startup_network(id) == Some(*network))
    }

    /// Reconstructs an approved private binding with a fresh, matching fingerprint.
    ///
    /// # Errors
    /// Requires a single approved interface and a matching saved fingerprint.
    pub fn restore_startup(
        &self,
        settings: &DesktopSettings,
    ) -> Result<ApprovedSelection, InterfaceError> {
        let [id] = settings.approved_interfaces() else {
            return Err(InterfaceError::AutoListenUnavailable);
        };
        let network = settings
            .startup_network()
            .ok_or(InterfaceError::AutoListenUnavailable)?;
        if self.startup_network(id) != Some(network) {
            return Err(InterfaceError::AutoListenUnavailable);
        }
        let mut selection = self.restore(settings, AutoListenContext::Confirmed)?;
        selection.required_network = Some((id.clone(), network));
        Ok(selection)
    }

    /// Records explicit local approval for the current records and selected port.
    /// The caller must obtain foreground consent before calling.
    ///
    /// # Errors
    /// Rejects a stale revision, invalid selection or unusable address.
    pub fn approve(
        &self,
        expected_revision: u64,
        ids: &[ApprovedInterfaceId],
        port: NonZeroU16,
    ) -> Result<ApprovedSelection, InterfaceError> {
        if self.revision != expected_revision {
            return Err(InterfaceError::StaleInventory);
        }
        if ids.is_empty() || ids.len() > MAX_APPROVED_INTERFACES {
            return Err(InterfaceError::InvalidSelection);
        }
        let mut indexes = ids
            .iter()
            .map(|id| {
                self.records
                    .binary_search_by(|record| record.id.cmp(id))
                    .map_err(|_| InterfaceError::InvalidSelection)
            })
            .collect::<Result<Vec<_>, _>>()?;
        indexes.sort_unstable();
        if indexes.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(InterfaceError::InvalidSelection);
        }
        let mut selected = Vec::with_capacity(indexes.len());
        for index in indexes {
            let record = &self.records[index];
            if record.usable_addresses().next().is_none() {
                return Err(InterfaceError::AddressUnavailable);
            }
            selected.push(record.clone());
        }
        Ok(ApprovedSelection {
            revision: self.revision,
            selected,
            port,
            required_network: None,
        })
    }

    /// Reconstructs an auto-listen selection from persisted settings and a core
    /// decision based on independent network continuity evidence.
    ///
    /// # Errors
    /// Rejects an absent opt-in, unconfirmed continuity or missing/ineligible IDs.
    pub fn restore(
        &self,
        settings: &DesktopSettings,
        continuity: AutoListenContext,
    ) -> Result<ApprovedSelection, InterfaceError> {
        if !settings.auto_listen() || continuity != AutoListenContext::Confirmed {
            return Err(InterfaceError::AutoListenUnavailable);
        }
        let port = settings
            .server_port()
            .and_then(NonZeroU16::new)
            .ok_or(InterfaceError::AutoListenUnavailable)?;
        let approval = self.approve(self.revision, settings.approved_interfaces(), port)?;
        if !approval.supports_auto_listen() {
            return Err(InterfaceError::AutoListenUnavailable);
        }
        Ok(approval)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AutoListenContext {
    Unknown,
    Changed,
    Confirmed,
}

pub struct ApprovedSelection {
    revision: u64,
    selected: Vec<Interface>,
    port: NonZeroU16,
    pub(crate) required_network: Option<(ApprovedInterfaceId, NetworkFingerprint)>,
}

impl ApprovedSelection {
    fn supports_auto_listen(&self) -> bool {
        self.selected.iter().all(|interface| {
            interface.persistence == IdentityPersistence::Stable
                && interface.default_exclusion().is_none()
        })
    }

    /// Produces settings for the config actor to save with its own revision.
    /// Returns the number of selected interfaces that could only be approved
    /// for this process.
    ///
    /// # Errors
    /// Rejects auto-listen opt-in for a session-only or non-default selection.
    pub fn project_settings(
        &self,
        previous: &DesktopSettings,
        enable_auto_listen: bool,
    ) -> Result<(DesktopSettings, usize), InterfaceError> {
        if enable_auto_listen && !self.supports_auto_listen() {
            return Err(InterfaceError::AutoListenUnavailable);
        }
        let ids: Vec<_> = self
            .selected
            .iter()
            .filter(|interface| interface.persistence == IdentityPersistence::Stable)
            .map(|interface| interface.id.clone())
            .collect();
        let session_only = self.selected.len() - ids.len();
        let settings = DesktopSettings::new(
            Some(self.port.get()),
            ids,
            enable_auto_listen,
            previous.start_at_login(),
            previous.log_level(),
        )
        .map_err(|_| InterfaceError::InvalidSelection)?;
        Ok((settings, session_only))
    }
}

#[derive(Clone, Copy)]
struct ListenerAddress {
    socket: SocketAddr,
    prefix_len: u8,
}

/// Complete set of literal addresses and their acceptance policy.
pub struct BindingPlan {
    revision: u64,
    generation: NonZeroU64,
    addresses: Vec<ListenerAddress>,
}

impl BindingPlan {
    /// Derives a complete plan from fresh inventory and prior explicit approval.
    ///
    /// # Errors
    /// Rejects stale approval, ambiguous addresses and oversized selections.
    pub fn new(
        current: &InterfaceSnapshot,
        approval: &ApprovedSelection,
        generation: NonZeroU64,
    ) -> Result<Self, InterfaceError> {
        if current.revision != approval.revision
            || !current.matches_network(approval.required_network.as_ref())
        {
            return Err(InterfaceError::StaleInventory);
        }
        let mut addresses = Vec::new();
        for selected in &approval.selected {
            let record = current
                .records
                .binary_search_by(|candidate| candidate.id.cmp(&selected.id))
                .ok()
                .and_then(|index| current.records.get(index))
                .filter(|record| *record == selected)
                .ok_or(InterfaceError::StaleInventory)?;
            for address in record.usable_addresses() {
                if addresses.len() == MAX_LISTENERS {
                    return Err(InterfaceError::ListenerLimit);
                }
                if current.records.iter().any(|other| {
                    other.id != record.id
                        && other
                            .addresses
                            .binary_search_by_key(&address.address, AssignedAddress::address)
                            .is_ok()
                }) {
                    return Err(InterfaceError::DuplicateAddress);
                }
                addresses.push(ListenerAddress {
                    socket: SocketAddr::new(address.address, approval.port.get()),
                    prefix_len: address.prefix_len,
                });
            }
        }
        addresses.sort_unstable_by_key(|entry| entry.socket);
        Ok(Self {
            revision: current.revision,
            generation,
            addresses,
        })
    }

    /// The supervisor requests one literal socket per address for the shared port.
    pub fn bind_addresses(&self) -> impl ExactSizeIterator<Item = SocketAddr> + '_ {
        self.addresses.iter().map(|entry| entry.socket)
    }

    /// Checks that a prepared plan still matches published interface authority.
    #[must_use]
    pub fn is_current(
        &self,
        inventory: &InterfaceInventory,
        active_generation: NonZeroU64,
    ) -> bool {
        self.generation == active_generation
            && inventory
                .current()
                .is_some_and(|current| current.revision == self.revision)
    }

    /// Returns the canonical peer IP to use for quotas after all checks pass.
    #[must_use]
    pub fn admit_peer(
        &self,
        inventory: &InterfaceInventory,
        active_generation: NonZeroU64,
        local: SocketAddr,
        peer: SocketAddr,
    ) -> Option<IpAddr> {
        if !self.is_current(inventory, active_generation)
            || peer.port() == 0
            || matches!(peer, SocketAddr::V6(addr) if addr.scope_id() != 0)
        {
            return None;
        }
        let entry = self
            .addresses
            .binary_search_by_key(&local, |entry| entry.socket)
            .ok()
            .and_then(|index| self.addresses.get(index))?;
        let peer = peer.ip().to_canonical();
        prefix::on_link(local.ip(), entry.prefix_len, peer).then_some(peer)
    }
}

macro_rules! redacted_debug {
    ($($type:ty),+ $(,)?) => {$(
        impl fmt::Debug for $type {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(stringify!($type), "(<redacted>)"))
            }
        }
    )+};
}

redacted_debug!(
    AssignedAddress,
    InterfaceIndexes,
    Interface,
    InterfaceInventory,
    InterfaceSnapshot,
    ApprovedSelection,
    BindingPlan
);

#[cfg(test)]
mod tests;
