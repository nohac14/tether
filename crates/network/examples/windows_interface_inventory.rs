//! Prints aggregate counts from a read-only Windows interface refresh.

#[cfg(windows)]
fn main() -> Result<(), tether_network::interfaces::InterfaceError> {
    use tether_network::interfaces::{
        AssignedAddress, Interface, NetworkProfile, WindowsInterfaceProvider,
    };

    let mut provider = WindowsInterfaceProvider::default();
    let first = provider.refresh()?;
    let revision = first.revision();
    let interface_count = first.records().len();
    let preferred_addresses = first
        .records()
        .iter()
        .flat_map(Interface::addresses)
        .filter(|address| address.is_usable())
        .count();
    let proposed = first
        .records()
        .iter()
        .filter(|record| record.default_exclusion().is_none())
        .count();
    let ipv6 = first
        .records()
        .iter()
        .flat_map(Interface::addresses)
        .map(AssignedAddress::address)
        .filter(std::net::IpAddr::is_ipv6)
        .count();
    let private = first
        .records()
        .iter()
        .filter(|record| record.status().profile == NetworkProfile::Private)
        .count();
    let public = first
        .records()
        .iter()
        .filter(|record| record.status().profile == NetworkProfile::Public)
        .count();
    let second = provider.refresh()?;
    println!("Interfaces: {interface_count}");
    println!("Eligible address rows: {preferred_addresses}");
    println!("IPv6 address rows: {ipv6}");
    println!("Private profile adapters: {private}");
    println!("Public profile adapters: {public}");
    println!("Default proposals: {proposed}");
    println!(
        "Unchanged revision on repeat: {}",
        second.revision() == revision
    );
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("This read-only inventory diagnostic requires Windows.");
}
