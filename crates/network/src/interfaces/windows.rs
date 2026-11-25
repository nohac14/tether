//! Read-only Windows IP Helper and Network List Manager adapter.
#![allow(unsafe_code)]

use super::{
    AddressState, AssignedAddress, IdentityPersistence, Interface, InterfaceError,
    InterfaceIndexes, InterfaceInventory, InterfaceKind, InterfaceSnapshot, InterfaceStatus,
    LinkState, MAX_ADDRESSES_PER_INTERFACE, MAX_INTERFACES,
};
use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::num::NonZeroU32;
use std::ptr::{self, NonNull};
use std::slice;
use std::task::Poll;
use std::thread;
use std::time::{Duration, Instant};
use tether_config::{ApprovedInterfaceId, NetworkFingerprint};
use windows::Win32::Foundation::{ERROR_NOT_FOUND, NO_ERROR, WIN32_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIfTable2Ex, GetUnicastIpAddressTable, IF_TYPE_ETHERNET_CSMACD,
    IF_TYPE_IEEE80211, IF_TYPE_PPP, IF_TYPE_PROP_VIRTUAL, IF_TYPE_SOFTWARE_LOOPBACK,
    IF_TYPE_TUNNEL, MIB_IF_ROW2, MIB_IF_TABLE2, MIB_UNICASTIPADDRESS_ROW,
    MIB_UNICASTIPADDRESS_TABLE, MibIfTableNormalWithoutStatistics,
};
use windows::Win32::NetworkManagement::Ndis::{
    IfOperStatusUp, MediaConnectStateConnected, NET_IF_ACCESS_POINT_TO_POINT,
    NET_IF_ADMIN_STATUS_UP,
};
use windows::Win32::Networking::WinSock::{
    AF_INET, AF_INET6, AF_UNSPEC, IpDadStatePreferred, IpSuffixOriginDhcp as IP_SUFFIX_DHCP,
    IpSuffixOriginLinkLayerAddress as IP_SUFFIX_LINK_LAYER,
    IpSuffixOriginManual as IP_SUFFIX_MANUAL, IpSuffixOriginRandom as IP_SUFFIX_RANDOM,
    IpSuffixOriginWellKnown as IP_SUFFIX_WELL_KNOWN,
};

const HARDWARE_INTERFACE: u8 = 1 << 0;
const FILTER_INTERFACE: u8 = 1 << 1;
const LINK_UNAVAILABLE: u8 = (1 << 3) | (1 << 4) | (1 << 5) | (1 << 6);
const MAX_NATIVE_ADDRESSES: usize = MAX_INTERFACES * MAX_ADDRESSES_PER_INTERFACE;
const REFRESH_DEADLINE: Duration = Duration::from_secs(2);

mod continuity;
mod profile;
mod worker;

use profile::{AdapterProfiles, ProfileReader, Profiles};
use worker::SampleWorker;

#[derive(Eq, PartialEq)]
struct WindowsRecord {
    luid: u64,
    network_guid: u128,
    profiles: AdapterProfiles,
    interface: Interface,
}

#[derive(Default)]
struct NetworkSample {
    records: Vec<WindowsRecord>,
    startup_networks: BTreeMap<ApprovedInterfaceId, NetworkFingerprint>,
}

/// A serialized, read-only provider. The dedicated worker runs IP Helper and
/// Network List Manager calls. The begin/poll API keeps the supervisor
/// available for safety commands while a native query is outstanding.
#[derive(Default)]
pub struct WindowsInterfaceProvider {
    worker: Option<SampleWorker>,
    inventory: InterfaceInventory,
    previous: Vec<WindowsRecord>,
}

impl WindowsInterfaceProvider {
    /// Synchronously drives a refresh for diagnostics. The supervisor uses
    /// begin/poll so a safety transition can interrupt the wait.
    ///
    /// # Errors
    /// Rejects OS errors, worker timeouts, ambiguous or oversized tables, or
    /// successive collections that disagree on fields relevant to a listener.
    pub fn refresh(&mut self) -> Result<&InterfaceSnapshot, InterfaceError> {
        self.begin_refresh(Instant::now())?;
        loop {
            if let Poll::Ready(result) = self.poll_refresh(Instant::now()) {
                result?;
                return self
                    .inventory
                    .current()
                    .ok_or(InterfaceError::ProviderUnavailable);
            }
            thread::park_timeout(Duration::from_millis(1));
        }
    }

    /// Starts one bounded refresh without waiting for native code.
    ///
    /// # Errors
    /// Returns an error if the worker cannot start or a refresh is already open.
    pub fn begin_refresh(&mut self, now: Instant) -> Result<(), InterfaceError> {
        let worker = match &mut self.worker {
            Some(worker) => worker,
            None => self
                .worker
                .insert(SampleWorker::start(read_consistent_records)?),
        };
        worker.begin(now, REFRESH_DEADLINE)
    }

    /// Retires the pending query's authority without waiting for the OS.
    pub fn cancel_refresh(&mut self) {
        if let Some(worker) = &mut self.worker {
            worker.cancel();
        }
    }

    /// Publishes a completed refresh. The old revision is retired on failure.
    pub fn poll_refresh(&mut self, now: Instant) -> Poll<Result<(), InterfaceError>> {
        match self.worker.as_mut().map_or(
            Poll::Ready(Err(InterfaceError::RefreshNotStarted)),
            |worker| worker.poll(now),
        ) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => Poll::Ready(self.publish(result).map(|_| ())),
        }
    }

    fn publish(
        &mut self,
        refreshed: Result<NetworkSample, InterfaceError>,
    ) -> Result<&InterfaceSnapshot, InterfaceError> {
        let NetworkSample {
            records,
            startup_networks,
        } = match refreshed {
            Ok(sample) => sample,
            Err(error) => {
                self.inventory.retire();
                self.previous.clear();
                return Err(error);
            }
        };
        if self.previous != records {
            self.inventory.retire();
        }
        let interfaces = records
            .iter()
            .map(|record| record.interface.clone())
            .collect();
        self.previous = records;
        self.inventory.refresh(Ok(interfaces))?;
        self.inventory.update_startup_networks(startup_networks);
        self.inventory
            .current()
            .ok_or(InterfaceError::StaleInventory)
    }

    #[must_use]
    pub const fn inventory(&self) -> &InterfaceInventory {
        &self.inventory
    }
}

fn read_consistent_records() -> Result<NetworkSample, InterfaceError> {
    let reader = ProfileReader::new()?;
    let first = read_records(reader.read()?)?;
    let first_networks = continuity::sample(&first);
    let second = read_records(reader.read()?)?;
    if first != second {
        return Err(InterfaceError::ProviderChanged);
    }
    let mut startup_networks = continuity::sample(&second);
    startup_networks.retain(|id, network| first_networks.get(id) == Some(network));
    Ok(NetworkSample {
        records: second,
        startup_networks,
    })
}

struct MibTable<T>(NonNull<T>);

impl<T> MibTable<T> {
    fn receive(
        get: impl FnOnce(&mut *mut T) -> WIN32_ERROR,
    ) -> Result<Option<Self>, InterfaceError> {
        let mut pointer = ptr::null_mut();
        match get(&mut pointer) {
            NO_ERROR => NonNull::new(pointer)
                .map(|pointer| Some(Self(pointer)))
                .ok_or(InterfaceError::ProviderUnavailable),
            ERROR_NOT_FOUND => Ok(None),
            _ => Err(InterfaceError::ProviderUnavailable),
        }
    }
}

impl<T> Drop for MibTable<T> {
    fn drop(&mut self) {
        // SAFETY: receive takes ownership only of a successful, non-null IP
        // Helper allocation. This guard frees it once after all row borrows end.
        unsafe { FreeMibTable(self.0.as_ptr().cast()) };
    }
}

impl MibTable<MIB_IF_TABLE2> {
    fn interfaces(&self) -> Result<&[MIB_IF_ROW2], InterfaceError> {
        let table = self.0.as_ptr();
        // SAFETY: IP Helper owns the initialized header and trailing row
        // array. Use the generated layout's aligned offset and cap the count
        // before borrowing rows from this live allocation.
        unsafe {
            let count = (*table).NumEntries as usize;
            if count > MAX_INTERFACES {
                return Err(InterfaceError::InventoryLimit);
            }
            Ok(slice::from_raw_parts(
                (&raw const (*table).Table).cast(),
                count,
            ))
        }
    }
}

impl MibTable<MIB_UNICASTIPADDRESS_TABLE> {
    fn addresses(&self) -> Result<&[MIB_UNICASTIPADDRESS_ROW], InterfaceError> {
        let table = self.0.as_ptr();
        // SAFETY: IP Helper initializes this table including its trailing
        // rows. The returned slice borrows the owner and uses the SDK offset.
        unsafe {
            let count = (*table).NumEntries as usize;
            if count > MAX_NATIVE_ADDRESSES {
                return Err(InterfaceError::InventoryLimit);
            }
            Ok(slice::from_raw_parts(
                (&raw const (*table).Table).cast(),
                count,
            ))
        }
    }
}

fn read_records(mut profiles: Profiles) -> Result<Vec<WindowsRecord>, InterfaceError> {
    // SAFETY: Each call receives a valid writable out-pointer, initialized to
    // null. On success the returned allocation transfers to the MibTable.
    let interfaces =
        MibTable::receive(|out| unsafe { GetIfTable2Ex(MibIfTableNormalWithoutStatistics, out) })?
            .ok_or(InterfaceError::ProviderUnavailable)?;
    // SAFETY: The successful AF_UNSPEC table has initialized IPv4/IPv6 rows
    // owned by the guard. ERROR_NOT_FOUND permits an empty address inventory.
    let addresses = MibTable::receive(|out| unsafe { GetUnicastIpAddressTable(AF_UNSPEC, out) })?;
    let interfaces = interfaces.interfaces()?;
    let addresses = addresses
        .as_ref()
        .map_or(Ok([].as_slice()), MibTable::addresses)?;

    let mut rows = BTreeMap::new();
    let mut indexes = BTreeMap::new();
    for interface in interfaces {
        // SAFETY: Value is the full initialized backing integer of NET_LUID.
        let luid = unsafe { interface.InterfaceLuid.Value };
        if luid == 0
            || rows.insert(luid, (interface, Vec::new())).is_some()
            || indexes.insert(interface.InterfaceIndex, luid).is_some()
        {
            return Err(InterfaceError::ProviderChanged);
        }
    }
    for address in addresses {
        // SAFETY: Value reads the full initialized NET_LUID backing integer.
        let luid = unsafe { address.InterfaceLuid.Value };
        let (interface, assigned) = rows.get_mut(&luid).ok_or(InterfaceError::ProviderChanged)?;
        if interface.InterfaceIndex != address.InterfaceIndex {
            return Err(InterfaceError::ProviderChanged);
        }
        if assigned.len() == MAX_ADDRESSES_PER_INTERFACE {
            return Err(InterfaceError::InventoryLimit);
        }
        assigned.push(translate_address(address)?);
    }

    rows.into_iter()
        .map(|(luid, (row, addresses))| {
            let profiles = profiles
                .remove(&row.InterfaceGuid.to_u128())
                .unwrap_or_default();
            translate_interface(row, luid, addresses, profiles)
        })
        .collect()
}

fn translate_interface(
    row: &MIB_IF_ROW2,
    luid: u64,
    addresses: Vec<AssignedAddress>,
    profiles: AdapterProfiles,
) -> Result<WindowsRecord, InterfaceError> {
    let guid = row.InterfaceGuid.to_u128();
    if guid == 0 {
        return Err(InterfaceError::ProviderUnavailable);
    }
    let index = NonZeroU32::new(row.InterfaceIndex).ok_or(InterfaceError::ProviderUnavailable)?;
    let alias_end = row
        .Alias
        .iter()
        .position(|&unit| unit == 0)
        .ok_or(InterfaceError::InvalidName)?;
    let alias =
        String::from_utf16(&row.Alias[..alias_end]).map_err(|_| InterfaceError::InvalidName)?;
    let flags = row.InterfaceAndOperStatusFlags._bitfield;
    let physical = flags & HARDWARE_INTERFACE != 0 && flags & FILTER_INTERFACE == 0;
    let kind = match row.Type {
        IF_TYPE_SOFTWARE_LOOPBACK => InterfaceKind::Loopback,
        IF_TYPE_PPP | IF_TYPE_TUNNEL => InterfaceKind::Tunnel,
        IF_TYPE_IEEE80211 if physical => InterfaceKind::WiFi,
        IF_TYPE_ETHERNET_CSMACD if physical => InterfaceKind::Ethernet,
        IF_TYPE_IEEE80211 | IF_TYPE_ETHERNET_CSMACD | IF_TYPE_PROP_VIRTUAL => {
            InterfaceKind::Virtual
        }
        _ => InterfaceKind::Unknown,
    };
    let link = if row.OperStatus != IfOperStatusUp
        || row.AdminStatus != NET_IF_ADMIN_STATUS_UP
        || row.MediaConnectState != MediaConnectStateConnected
        || flags & LINK_UNAVAILABLE != 0
    {
        LinkState::Down
    } else if row.AccessType == NET_IF_ACCESS_POINT_TO_POINT {
        LinkState::PointToPoint
    } else {
        LinkState::Up
    };
    let id = ApprovedInterfaceId::new(format!("windows-adapter-{guid:032x}"))
        .map_err(|_| InterfaceError::ProviderUnavailable)?;
    let profile = profiles.classification();
    Ok(WindowsRecord {
        luid,
        network_guid: row.NetworkGuid.to_u128(),
        profiles,
        interface: Interface::new(
            id,
            &alias,
            IdentityPersistence::Stable,
            InterfaceIndexes {
                ipv4: Some(index),
                ipv6: Some(index),
            },
            InterfaceStatus {
                kind,
                link,
                profile,
            },
            addresses,
        )?,
    })
}

fn translate_address(row: &MIB_UNICASTIPADDRESS_ROW) -> Result<AssignedAddress, InterfaceError> {
    // SAFETY: SOCKADDR_INET stores the family at the shared offset. Read the
    // address member only for the matching, initialized IP Helper socket type.
    let (ip, needs_zone) = unsafe {
        match row.Address.si_family {
            AF_INET => (
                IpAddr::V4(Ipv4Addr::from(
                    row.Address.Ipv4.sin_addr.S_un.S_addr.to_ne_bytes(),
                )),
                false,
            ),
            AF_INET6 => (
                IpAddr::V6(Ipv6Addr::from(row.Address.Ipv6.sin6_addr.u.Byte)),
                row.Address.Ipv6.Anonymous.sin6_scope_id != 0,
            ),
            _ => return Err(InterfaceError::ProviderUnavailable),
        }
    };
    let state = if row.DadState != IpDadStatePreferred
        || row.SkipAsSource
        || needs_zone
        || row.ValidLifetime == 0
        || row.PreferredLifetime == 0
    {
        AddressState::Ineligible
    } else {
        match row.SuffixOrigin {
            IP_SUFFIX_RANDOM => AddressState::Temporary,
            IP_SUFFIX_DHCP | IP_SUFFIX_LINK_LAYER | IP_SUFFIX_MANUAL | IP_SUFFIX_WELL_KNOWN => {
                AddressState::Preferred
            }
            _ => AddressState::Unknown,
        }
    };
    AssignedAddress::new(ip, row.OnLinkPrefixLength, state)
}

#[cfg(test)]
mod tests;
