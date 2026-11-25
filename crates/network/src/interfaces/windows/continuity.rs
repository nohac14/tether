use super::super::prefix;
use super::{BTreeMap, Interface, MibTable, WindowsRecord};
use sha2::{Digest, Sha256};
use std::net::{IpAddr, Ipv4Addr};
use std::slice;
use tether_config::{ApprovedInterfaceId, NetworkFingerprint};
use windows::Win32::Foundation::NO_ERROR;
use windows::Win32::NetworkManagement::IpHelper::{
    GetIpForwardTable2, GetIpNetEntry2, MIB_IPFORWARD_ROW2, MIB_IPNET_ROW2,
};
use windows::Win32::Networking::WinSock::{
    AF_INET, NlnsDelay, NlnsProbe, NlnsReachable, NlnsStale, SOCKADDR_INET,
};

const MAX_ROUTES: usize = 4096;

pub(super) fn sample(
    records: &[WindowsRecord],
) -> BTreeMap<ApprovedInterfaceId, NetworkFingerprint> {
    // SAFETY: on success the typed out-pointer transfers a table to the guard.
    let Ok(Some(routes)) = MibTable::receive(|out| unsafe { GetIpForwardTable2(AF_INET, out) })
    else {
        return BTreeMap::new();
    };
    let table = routes.0.as_ptr();
    // SAFETY: the guard owns the table and its entries until this borrow ends.
    let entries: &[MIB_IPFORWARD_ROW2] = unsafe {
        let count = (*table).NumEntries as usize;
        if count > MAX_ROUTES {
            return BTreeMap::new();
        }
        slice::from_raw_parts((&raw const (*table).Table).cast(), count)
    };
    records
        .iter()
        .filter_map(|record| {
            let interface = &record.interface;
            let profile = record.profiles.private_network()?;
            if interface.default_exclusion().is_some() {
                return None;
            }
            let local = interface
                .addresses()
                .iter()
                .find(|address| address.address().is_ipv4() && address.is_usable())?;
            let index = interface.indexes().ipv4?.get();
            let mut defaults = entries.iter().filter(|route| {
                route.InterfaceIndex == index
                    && route.DestinationPrefix.PrefixLength == 0
                    && ipv4(route.DestinationPrefix.Prefix) == Some(Ipv4Addr::UNSPECIFIED)
            });
            let route = defaults.next()?;
            // SAFETY: Value is the initialized backing integer of the route LUID.
            if defaults.next().is_some() || unsafe { route.InterfaceLuid.Value } != record.luid {
                return None;
            }
            let gateway = ipv4(route.NextHop)?;
            if local.address() == IpAddr::V4(gateway)
                || !prefix::on_link(local.address(), local.prefix_len(), IpAddr::V4(gateway))
            {
                return None;
            }
            let mac = router_address(route.NextHop, index)?;
            let network = fingerprint(profile, interface, gateway, mac);
            Some((interface.id().clone(), network))
        })
        .collect()
}

fn ipv4(address: SOCKADDR_INET) -> Option<Ipv4Addr> {
    // SAFETY: read the initialized family and only the matching union member.
    unsafe {
        (address.si_family == AF_INET)
            .then(|| Ipv4Addr::from(address.Ipv4.sin_addr.S_un.S_addr.to_ne_bytes()))
    }
}

fn router_address(gateway: SOCKADDR_INET, index: u32) -> Option<[u8; 6]> {
    let mut row = MIB_IPNET_ROW2 {
        Address: gateway,
        InterfaceIndex: index,
        ..Default::default()
    };
    // SAFETY: IP Helper receives one writable row with family, address and index.
    if unsafe { GetIpNetEntry2(&raw mut row) } != NO_ERROR
        || row.PhysicalAddressLength != 6
        || ![NlnsReachable, NlnsStale, NlnsDelay, NlnsProbe].contains(&row.State)
    {
        return None;
    }
    let mac: [u8; 6] = row.PhysicalAddress[..6].try_into().ok()?;
    (mac != [0; 6] && mac[0] & 1 == 0).then_some(mac)
}

fn fingerprint(
    profile: u128,
    interface: &Interface,
    gateway: Ipv4Addr,
    mac: [u8; 6],
) -> NetworkFingerprint {
    let mut hash = Sha256::new();
    hash.update(b"tether-windows-startup-v1");
    hash.update(profile.to_be_bytes());
    hash.update(gateway.octets());
    hash.update(mac);
    for address in interface.usable_addresses() {
        match address.address() {
            IpAddr::V4(ip) => {
                hash.update([4]);
                hash.update(ip.octets());
            }
            IpAddr::V6(ip) => {
                hash.update([6]);
                hash.update(ip.octets());
            }
        }
        hash.update([address.prefix_len()]);
    }
    NetworkFingerprint::new(hash.finalize().into())
}
