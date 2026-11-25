use super::*;
use crate::interfaces::{DefaultExclusion, NetworkProfile};
use windows::Win32::Networking::NetworkListManager::{
    NLM_NETWORK_CATEGORY, NLM_NETWORK_CATEGORY_DOMAIN_AUTHENTICATED, NLM_NETWORK_CATEGORY_PRIVATE,
    NLM_NETWORK_CATEGORY_PUBLIC,
};
use windows::Win32::Networking::WinSock::{
    IN_ADDR, IN_ADDR_0, IN6_ADDR, IN6_ADDR_0, IpDadStateTentative, SOCKADDR_IN, SOCKADDR_IN6,
    SOCKADDR_IN6_0, SOCKADDR_INET,
};
use windows::core::GUID;

#[test]
fn only_preferred_non_temporary_native_addresses_are_usable() {
    let mut row = MIB_UNICASTIPADDRESS_ROW {
        Address: SOCKADDR_INET {
            Ipv4: SOCKADDR_IN {
                sin_family: AF_INET,
                sin_addr: IN_ADDR {
                    S_un: IN_ADDR_0 {
                        S_addr: u32::from_ne_bytes([192, 0, 2, 42]),
                    },
                },
                ..Default::default()
            },
        },
        OnLinkPrefixLength: 25,
        DadState: IpDadStatePreferred,
        ValidLifetime: 10,
        PreferredLifetime: 5,
        SuffixOrigin: IP_SUFFIX_DHCP,
        ..Default::default()
    };
    let address = translate_address(&row).unwrap();
    assert_eq!(address.address(), Ipv4Addr::new(192, 0, 2, 42));
    assert_eq!(address.prefix_len(), 25);
    assert!(address.is_usable());
    row.SuffixOrigin = IP_SUFFIX_RANDOM;
    assert_eq!(
        translate_address(&row).unwrap().state(),
        AddressState::Temporary
    );
    row.SuffixOrigin = IP_SUFFIX_MANUAL;
    row.DadState = IpDadStateTentative;
    assert_eq!(
        translate_address(&row).unwrap().state(),
        AddressState::Ineligible
    );
    row.DadState = IpDadStatePreferred;
    row.SkipAsSource = true;
    assert!(!translate_address(&row).unwrap().is_usable());
    row.SkipAsSource = false;
    row.OnLinkPrefixLength = 64;
    let v6 = Ipv6Addr::new(0xfd00, 1, 0, 0, 0, 0, 0, 1);
    let mut v6_socket = SOCKADDR_IN6 {
        sin6_family: AF_INET6,
        sin6_addr: IN6_ADDR {
            u: IN6_ADDR_0 { Byte: v6.octets() },
        },
        ..Default::default()
    };
    row.Address = SOCKADDR_INET { Ipv6: v6_socket };
    assert!(translate_address(&row).unwrap().is_usable());
    v6_socket.Anonymous = SOCKADDR_IN6_0 { sin6_scope_id: 3 };
    row.Address = SOCKADDR_INET { Ipv6: v6_socket };
    assert!(!translate_address(&row).unwrap().is_usable());
    row.Address = SOCKADDR_INET {
        si_family: AF_UNSPEC,
    };
    assert_eq!(
        translate_address(&row),
        Err(InterfaceError::ProviderUnavailable)
    );
}

#[test]
fn stable_adapter_identity_does_not_grant_unknown_network_trust() {
    let mut row = MIB_IF_ROW2 {
        InterfaceGuid: GUID::from_u128(1),
        InterfaceIndex: 4,
        Type: IF_TYPE_ETHERNET_CSMACD,
        OperStatus: IfOperStatusUp,
        AdminStatus: NET_IF_ADMIN_STATUS_UP,
        MediaConnectState: MediaConnectStateConnected,
        ..Default::default()
    };
    row.Alias[0] = u16::from(b'e');
    row.InterfaceAndOperStatusFlags._bitfield = HARDWARE_INTERFACE;
    let addresses = vec![
        AssignedAddress::new(
            IpAddr::V4(Ipv4Addr::new(192, 0, 2, 42)),
            25,
            AddressState::Preferred,
        )
        .unwrap(),
    ];
    let record =
        translate_interface(&row, 1, addresses.clone(), AdapterProfiles::default()).unwrap();
    assert_eq!(record.interface.persistence(), IdentityPersistence::Stable);
    assert_eq!(record.interface.status().kind, InterfaceKind::Ethernet);
    assert_eq!(record.interface.status().profile, NetworkProfile::Unknown);
    assert_eq!(
        record.interface.default_exclusion(),
        Some(DefaultExclusion::UnknownProfile)
    );
    row.InterfaceAndOperStatusFlags._bitfield = HARDWARE_INTERFACE | FILTER_INTERFACE;
    let filtered =
        translate_interface(&row, 1, addresses.clone(), AdapterProfiles::default()).unwrap();
    assert_eq!(filtered.interface.status().kind, InterfaceKind::Virtual);
    row.InterfaceAndOperStatusFlags._bitfield = HARDWARE_INTERFACE | LINK_UNAVAILABLE;
    let stopped = translate_interface(&row, 1, addresses, AdapterProfiles::default()).unwrap();
    assert_eq!(stopped.interface.status().link, LinkState::Down);
}

#[test]
fn profile_change_or_timeout_retires_the_approval_revision() {
    let mut row = MIB_IF_ROW2 {
        InterfaceGuid: GUID::from_u128(1),
        InterfaceIndex: 4,
        Type: IF_TYPE_ETHERNET_CSMACD,
        OperStatus: IfOperStatusUp,
        AdminStatus: NET_IF_ADMIN_STATUS_UP,
        MediaConnectState: MediaConnectStateConnected,
        ..Default::default()
    };
    row.Alias[0] = u16::from(b'e');
    row.InterfaceAndOperStatusFlags._bitfield = HARDWARE_INTERFACE;
    let records = |network, profile| {
        let address = AssignedAddress::new(
            IpAddr::V4(Ipv4Addr::new(192, 0, 2, 42)),
            25,
            AddressState::Preferred,
        )
        .unwrap();
        let mut profiles = AdapterProfiles::default();
        profiles.observe(network, profile).unwrap();
        NetworkSample {
            records: vec![translate_interface(&row, 1, vec![address], profiles).unwrap()],
            ..Default::default()
        }
    };
    let mut provider = WindowsInterfaceProvider::default();
    let snapshot = provider
        .publish(Ok(records(1, NLM_NETWORK_CATEGORY_PRIVATE)))
        .unwrap();
    assert_eq!(snapshot.records()[0].default_exclusion(), None);
    let revision = snapshot.revision();
    let same = provider
        .publish(Ok(records(1, NLM_NETWORK_CATEGORY_PRIVATE)))
        .unwrap();
    assert_eq!(same.revision(), revision);
    let moved = provider
        .publish(Ok(records(2, NLM_NETWORK_CATEGORY_PRIVATE)))
        .unwrap();
    assert_ne!(moved.revision(), revision);
    let public = provider
        .publish(Ok(records(2, NLM_NETWORK_CATEGORY_PUBLIC)))
        .unwrap();
    assert_eq!(
        public.records()[0].default_exclusion(),
        Some(DefaultExclusion::PublicProfile)
    );
    let public_revision = public.revision();
    assert_eq!(
        provider
            .publish(Err(InterfaceError::ProviderTimedOut))
            .unwrap_err(),
        InterfaceError::ProviderTimedOut
    );
    assert!(provider.inventory().current().is_none());
    let recovered = provider
        .publish(Ok(records(2, NLM_NETWORK_CATEGORY_PUBLIC)))
        .unwrap();
    assert_ne!(recovered.revision(), public_revision);
    let domain = provider
        .publish(Ok(records(2, NLM_NETWORK_CATEGORY_DOMAIN_AUTHENTICATED)))
        .unwrap()
        .revision();
    let unknown = provider
        .publish(Ok(records(2, NLM_NETWORK_CATEGORY(10))))
        .unwrap();
    assert_eq!(
        unknown.records()[0].status().profile,
        NetworkProfile::Unknown
    );
    assert_ne!(unknown.revision(), domain);
}
