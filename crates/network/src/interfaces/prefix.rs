use std::net::IpAddr;

pub(super) fn usable_unicast(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => {
            let first = ip.octets()[0];
            (1..240).contains(&first) && !ip.is_loopback() && !ip.is_link_local()
        }
        IpAddr::V6(ip) => {
            ip.to_ipv4_mapped().is_none()
                && !ip.is_unspecified()
                && !ip.is_loopback()
                && !ip.is_multicast()
                && !ip.is_unicast_link_local()
        }
    }
}

fn is_v4_subnet_boundary(bits: u32, prefix: u8) -> bool {
    if prefix >= 31 {
        return false;
    }
    let host_mask = u32::MAX >> prefix;
    let host = bits & host_mask;
    host == 0 || host == host_mask
}

pub(super) fn is_usable_host(address: IpAddr, prefix: u8) -> bool {
    usable_unicast(address)
        && match address {
            IpAddr::V4(ip) => !is_v4_subnet_boundary(u32::from(ip), prefix),
            IpAddr::V6(_) => true,
        }
}

pub(super) fn on_link(local: IpAddr, prefix: u8, peer: IpAddr) -> bool {
    if !is_usable_host(peer, prefix) {
        return false;
    }
    match (local, peer) {
        (IpAddr::V4(local), IpAddr::V4(peer)) => {
            u32::from(local) >> (u32::BITS - u32::from(prefix))
                == u32::from(peer) >> (u32::BITS - u32::from(prefix))
        }
        (IpAddr::V6(local), IpAddr::V6(peer)) => {
            u128::from(local) >> (u128::BITS - u32::from(prefix))
                == u128::from(peer) >> (u128::BITS - u32::from(prefix))
        }
        _ => false,
    }
}
