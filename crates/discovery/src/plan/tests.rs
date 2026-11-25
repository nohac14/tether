use super::*;
use tether_config::SettingsStore;
use tether_network::interfaces::{
    AddressState, IdentityPersistence, Interface, InterfaceIndexes, InterfaceKind, InterfaceStatus,
    LinkState, NetworkProfile,
};
use tether_test_support::PrivateTempDir;

#[cfg(windows)]
mod windows;

const PORT: NonZeroU16 = NonZeroU16::new(54321).unwrap();
const GENERATION: NonZeroU64 = NonZeroU64::new(7).unwrap();

fn id(name: &str) -> ApprovedInterfaceId {
    ApprovedInterfaceId::new(name.to_owned()).unwrap()
}

fn adapter(name: &str, addresses: &[&str], ipv4: u32, ipv6: u32) -> Interface {
    Interface::new(
        id(name),
        name,
        IdentityPersistence::Stable,
        InterfaceIndexes {
            ipv4: NonZeroU32::new(ipv4),
            ipv6: NonZeroU32::new(ipv6),
        },
        InterfaceStatus {
            kind: InterfaceKind::Ethernet,
            link: LinkState::Up,
            profile: NetworkProfile::Private,
        },
        addresses
            .iter()
            .map(|address| {
                let ip: IpAddr = address.parse().unwrap();
                AssignedAddress::new(
                    ip,
                    if ip.is_ipv4() { 24 } else { 64 },
                    AddressState::Preferred,
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn fixture(records: Vec<Interface>, selected: &[&str]) -> (InterfaceInventory, BindingPlan) {
    let mut inventory = InterfaceInventory::default();
    let snapshot = inventory.refresh(Ok(records)).unwrap();
    let ids: Vec<_> = selected.iter().map(|name| id(name)).collect();
    let approval = snapshot.approve(snapshot.revision(), &ids, PORT).unwrap();
    let binding = BindingPlan::new(snapshot, &approval, GENERATION).unwrap();
    (inventory, binding)
}

fn identity() -> InstallationIdentity {
    let parent = PrivateTempDir::new().unwrap();
    let (store, _) = SettingsStore::open(&parent.path().join("settings")).unwrap();
    store.identity_snapshot().unwrap().identity().clone()
}

#[test]
fn projects_only_bound_literals_and_ephemeral_scope_indexes() {
    let (inventory, binding) = fixture(
        vec![
            adapter("approved", &["192.0.2.18", "fd00::18"], 7, 7),
            adapter("unapproved", &["198.51.100.43"], 15, 15),
        ],
        &["approved"],
    );
    let identity = identity();
    let plan =
        Advertisement::for_binding(&binding, &inventory, GENERATION, &identity, None).unwrap();
    assert_eq!(plan.generation(), GENERATION);
    assert_eq!(plan.hostname(), identity.preferred_hostname().as_str());
    assert!(plan.instance_name().ends_with("._tether._tcp.local"));
    let endpoints: Vec<_> = plan.literal_endpoints().collect();
    assert_eq!(
        endpoints,
        [
            "192.0.2.18:54321".parse::<SocketAddr>().unwrap(),
            "[fd00::18]:54321".parse().unwrap()
        ]
    );
    let registrations = plan.windows_registrations().unwrap();
    assert_eq!(registrations.len(), 1);
    let scope = registrations.first().unwrap();
    assert_eq!(scope.index().get(), 7);
    assert_eq!(scope.port(), PORT);
    assert_eq!(scope.ipv4(), Some("192.0.2.18".parse().unwrap()));
    assert_eq!(scope.ipv6(), Some("fd00::18".parse().unwrap()));
    assert_eq!(format!("{plan:?}"), "Advertisement(<redacted>)");
}

#[test]
fn stale_inventory_or_listener_generation_cannot_prepare_discovery() {
    let (mut inventory, binding) = fixture(
        vec![adapter("approved", &["192.0.2.18"], 7, 7)],
        &["approved"],
    );
    let identity = identity();
    assert_eq!(
        Advertisement::for_binding(&binding, &inventory, NonZeroU64::MIN, &identity, None)
            .unwrap_err(),
        DiscoveryError::StaleBinding
    );
    inventory
        .refresh(Ok(vec![adapter("approved", &["192.0.2.20"], 7, 7)]))
        .unwrap();
    assert_eq!(
        Advertisement::for_binding(&binding, &inventory, GENERATION, &identity, None).unwrap_err(),
        DiscoveryError::StaleBinding
    );
}

#[test]
fn conflicts_change_the_dns_name_without_changing_literal_or_instance_identity() {
    let (inventory, binding) = fixture(
        vec![adapter("approved", &["192.0.2.18"], 7, 7)],
        &["approved"],
    );
    let identity = identity();
    let first =
        Advertisement::for_binding(&binding, &inventory, GENERATION, &identity, None).unwrap();
    let next = Advertisement::for_binding(
        &binding,
        &inventory,
        GENERATION,
        &identity,
        Some(NonZeroU16::MIN),
    )
    .unwrap();
    assert_ne!(next.hostname(), first.hostname());
    assert_ne!(next.instance_name(), first.instance_name());
    assert_eq!(next.txt_records(), first.txt_records());
    assert!(next.literal_endpoints().eq(first.literal_endpoints()));
    assert_eq!(
        Advertisement::for_binding(
            &binding,
            &inventory,
            GENERATION,
            &identity,
            Some(NonZeroU16::MAX)
        )
        .unwrap_err(),
        DiscoveryError::InvalidName
    );
}

#[test]
fn unrepresentable_windows_scopes_keep_the_complete_ip_fallback() {
    let identity = identity();
    for (records, selected, expected) in [
        (
            vec![adapter("a", &["192.0.2.18"], 0, 0)],
            vec!["a"],
            DiscoveryError::MissingScope,
        ),
        (
            vec![adapter("a", &["192.0.2.18", "192.0.2.19"], 7, 7)],
            vec!["a"],
            DiscoveryError::UnrepresentableScope,
        ),
        (
            vec![adapter("a", &["192.0.2.18", "fd00::18"], 7, 8)],
            vec!["a"],
            DiscoveryError::UnrepresentableScope,
        ),
        (
            vec![
                adapter("a", &["192.0.2.18"], 7, 7),
                adapter("b", &["198.51.100.43"], 7, 7),
            ],
            vec!["a", "b"],
            DiscoveryError::UnrepresentableScope,
        ),
    ] {
        let (inventory, binding) = fixture(records, &selected);
        let plan =
            Advertisement::for_binding(&binding, &inventory, GENERATION, &identity, None).unwrap();
        assert!(plan.literal_endpoints().eq(binding.bind_addresses()));
        assert_eq!(plan.windows_registrations().unwrap_err(), expected);
    }
}
