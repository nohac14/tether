use super::*;
use tether_config::LogLevel;

const PORT: NonZeroU16 = NonZeroU16::new(54321).unwrap();
const GENERATION: NonZeroU64 = NonZeroU64::new(7).unwrap();
const DEFAULT_STATUS: InterfaceStatus = InterfaceStatus {
    kind: InterfaceKind::WiFi,
    link: LinkState::Up,
    profile: NetworkProfile::Private,
};

fn id(name: &str) -> ApprovedInterfaceId {
    ApprovedInterfaceId::new(name.to_owned()).unwrap()
}

fn assigned(address: &str, prefix: u8) -> AssignedAddress {
    AssignedAddress::new(address.parse().unwrap(), prefix, AddressState::Preferred).unwrap()
}

fn fixture_interface(name: &str, addresses: Vec<AssignedAddress>) -> Interface {
    Interface::new(
        id(name),
        name,
        IdentityPersistence::Stable,
        InterfaceIndexes {
            ipv4: NonZeroU32::new(1),
            ipv6: NonZeroU32::new(2),
        },
        DEFAULT_STATUS,
        addresses,
    )
    .unwrap()
}

fn socket(address: &str) -> SocketAddr {
    SocketAddr::new(address.parse().unwrap(), PORT.get())
}

fn approve(snapshot: &InterfaceSnapshot, names: &[&str]) -> ApprovedSelection {
    let ids: Vec<_> = names.iter().map(|name| id(name)).collect();
    snapshot.approve(snapshot.revision(), &ids, PORT).unwrap()
}

#[test]
fn admission_uses_the_actual_local_socket_and_its_interface_prefix() {
    let wifi = fixture_interface(
        "wifi",
        vec![assigned("192.0.2.12", 28), assigned("fd00:1::12", 64)],
    );
    let ethernet = fixture_interface("ethernet", vec![assigned("198.51.100.65", 26)]);
    let mut inventory = InterfaceInventory::default();
    let snapshot = inventory.refresh(Ok(vec![wifi, ethernet])).unwrap();
    let approval = approve(snapshot, &["wifi", "ethernet"]);
    let plan = BindingPlan::new(snapshot, &approval, GENERATION).unwrap();
    assert_eq!(plan.bind_addresses().len(), 3);

    let local = socket("192.0.2.12");
    let admitted = plan.admit_peer(&inventory, GENERATION, local, socket("192.0.2.14"));
    assert_eq!(admitted, Some("192.0.2.14".parse().unwrap()));
    assert_eq!(
        plan.admit_peer(&inventory, GENERATION, local, socket("::ffff:192.0.2.14")),
        admitted
    );
    for address in [
        "192.0.2.0",
        "192.0.2.15",
        "192.0.2.16",
        "198.51.100.66",
        "0.0.0.0",
        "224.0.0.1",
        "255.255.255.255",
        "127.0.0.1",
        "fd00:1::99",
    ] {
        assert!(
            plan.admit_peer(&inventory, GENERATION, local, socket(address))
                .is_none()
        );
    }
    assert!(
        plan.admit_peer(
            &inventory,
            GENERATION,
            socket("198.51.100.65"),
            socket("198.51.100.66")
        )
        .is_some()
    );
    assert!(
        plan.admit_peer(
            &inventory,
            GENERATION,
            socket("fd00:1::12"),
            socket("fd00:1::99")
        )
        .is_some()
    );
    assert!(
        plan.admit_peer(
            &inventory,
            GENERATION,
            socket("fd00:1::12"),
            socket("fd00:2::99")
        )
        .is_none()
    );
    assert!(
        plan.admit_peer(&inventory, GENERATION, socket("::"), socket("fd00:1::99"))
            .is_none()
    );
    let wrong_port = SocketAddr::new(local.ip(), PORT.get() + 1);
    assert!(
        plan.admit_peer(&inventory, GENERATION, wrong_port, socket("192.0.2.14"))
            .is_none()
    );
    let scoped_peer = "[::ffff:192.0.2.14%1]:10000".parse().unwrap();
    assert!(
        plan.admit_peer(&inventory, GENERATION, local, scoped_peer)
            .is_none()
    );
    assert!(
        plan.admit_peer(&inventory, NonZeroU64::MIN, local, socket("192.0.2.14"))
            .is_none()
    );
}

#[test]
fn unknown_or_unsafe_addresses_never_bind() {
    let preferred = assigned("192.0.2.20", 24);
    let mut addresses = vec![preferred];
    for (ip, prefix) in [
        ("0.0.0.0", 8),
        ("127.0.0.2", 8),
        ("169.254.3.2", 16),
        ("192.0.2.0", 24),
        ("192.0.2.255", 24),
        ("::", 64),
        ("::1", 128),
        ("::ffff:192.0.2.5", 128),
        ("fe80::20", 64),
        ("ff02::1", 64),
    ] {
        let address = assigned(ip, prefix);
        assert!(!address.is_usable());
        addresses.push(address);
    }
    for (ip, state) in [
        ("fd00::1", AddressState::Temporary),
        ("fd00::2", AddressState::Unknown),
        ("fd00::3", AddressState::Ineligible),
    ] {
        addresses.push(AssignedAddress::new(ip.parse().unwrap(), 64, state).unwrap());
    }
    let mut inventory = InterfaceInventory::default();
    let snapshot = inventory
        .refresh(Ok(vec![fixture_interface("wifi", addresses)]))
        .unwrap();
    let plan = BindingPlan::new(snapshot, &approve(snapshot, &["wifi"]), GENERATION).unwrap();
    assert_eq!(
        plan.bind_addresses().collect::<Vec<_>>(),
        [socket("192.0.2.20")]
    );
    for (ip, prefix) in [("192.0.2.20", 0), ("192.0.2.20", 33), ("fd00::1", 129)] {
        assert_eq!(
            AssignedAddress::new(ip.parse().unwrap(), prefix, AddressState::Preferred),
            Err(InterfaceError::InvalidPrefix),
        );
    }
}

#[test]
fn unchanged_inventory_keeps_approval_but_index_change_and_failure_retire_it() {
    let a = fixture_interface("a", vec![assigned("192.0.2.20", 24)]);
    let mut b = fixture_interface("b", vec![assigned("fd00::17", 64)]);
    let mut inventory = InterfaceInventory::default();
    let first = inventory.refresh(Ok(vec![a.clone(), b.clone()])).unwrap();
    let revision = first.revision();
    let approval = approve(first, &["a"]);
    let plan = BindingPlan::new(first, &approval, GENERATION).unwrap();
    let reordered = inventory.refresh(Ok(vec![b.clone(), a.clone()])).unwrap();
    assert_eq!(reordered.revision(), revision);
    assert!(BindingPlan::new(reordered, &approval, GENERATION).is_ok());
    b.indexes.ipv6 = NonZeroU32::new(40);
    let changed = inventory.refresh(Ok(vec![b, a.clone()])).unwrap();
    assert!(changed.revision() > revision);
    assert_eq!(
        BindingPlan::new(changed, &approval, GENERATION).unwrap_err(),
        InterfaceError::StaleInventory
    );
    assert!(
        plan.admit_peer(
            &inventory,
            GENERATION,
            socket("192.0.2.20"),
            socket("192.0.2.2")
        )
        .is_none()
    );

    let before_error = inventory.refresh(Ok(vec![a.clone()])).unwrap();
    let before_error_revision = before_error.revision();
    let plan = BindingPlan::new(before_error, &approve(before_error, &["a"]), GENERATION).unwrap();
    assert_eq!(
        inventory
            .refresh(Err(InterfaceError::InventoryLimit))
            .unwrap_err(),
        InterfaceError::InventoryLimit
    );
    assert!(inventory.current().is_none());
    assert!(
        plan.admit_peer(
            &inventory,
            GENERATION,
            socket("192.0.2.20"),
            socket("192.0.2.2")
        )
        .is_none()
    );
    let recovered = inventory.refresh(Ok(vec![a])).unwrap();
    assert!(recovered.revision() > before_error_revision);
}

#[test]
fn defaults_exclude_public_virtual_point_to_point_and_unavailable_interfaces() {
    let base = fixture_interface("wifi", vec![assigned("192.0.2.12", 24)]);
    assert_eq!(base.default_exclusion(), None);
    for kind in [
        InterfaceKind::Tunnel,
        InterfaceKind::Virtual,
        InterfaceKind::Bridge,
        InterfaceKind::Loopback,
        InterfaceKind::Unknown,
    ] {
        let mut interface = base.clone();
        interface.status.kind = kind;
        assert_eq!(
            interface.default_exclusion(),
            Some(DefaultExclusion::NonDefaultKind)
        );
    }
    let mut public = base.clone();
    public.status.profile = NetworkProfile::Public;
    assert_eq!(
        public.default_exclusion(),
        Some(DefaultExclusion::PublicProfile)
    );

    let mut unavailable = base.clone();
    for link in [LinkState::Down, LinkState::Unknown] {
        unavailable.status.link = link;
        assert_eq!(
            unavailable.default_exclusion(),
            Some(DefaultExclusion::LinkUnavailable)
        );
        assert!(unavailable.usable_addresses().next().is_none());
    }
    unavailable.status.link = LinkState::PointToPoint;
    assert_eq!(
        unavailable.default_exclusion(),
        Some(DefaultExclusion::PointToPoint)
    );
    let no_addresses = fixture_interface("link-local", vec![assigned("fe80::1", 64)]);
    assert_eq!(
        no_addresses.default_exclusion(),
        Some(DefaultExclusion::NoUsableAddress)
    );
}

#[test]
fn saved_approvals_keep_preferences_and_need_independent_continuity() {
    let stable = fixture_interface("stable", vec![assigned("fd00::20", 64)]);
    let mut ephemeral = fixture_interface("ephemeral", vec![assigned("192.0.2.20", 24)]);
    ephemeral.persistence = IdentityPersistence::SessionOnly;
    let mut inventory = InterfaceInventory::default();
    let snapshot = inventory.refresh(Ok(vec![stable, ephemeral])).unwrap();
    let previous = DesktopSettings::new(None, vec![], false, true, LogLevel::Warn).unwrap();
    let mixed = approve(snapshot, &["stable", "ephemeral"]);
    assert_eq!(
        mixed.project_settings(&previous, true).unwrap_err(),
        InterfaceError::AutoListenUnavailable
    );
    let (manual, session_count) = mixed.project_settings(&previous, false).unwrap();
    assert_eq!(session_count, 1);
    assert_eq!(manual.approved_interfaces(), &[id("stable")]);
    assert_eq!(manual.server_port(), Some(PORT.get()));
    assert!(manual.start_at_login());
    assert_eq!(manual.log_level(), LogLevel::Warn);
    let (saved, _) = approve(snapshot, &["stable"])
        .project_settings(&previous, true)
        .unwrap();
    for context in [AutoListenContext::Unknown, AutoListenContext::Changed] {
        assert_eq!(
            snapshot.restore(&saved, context).unwrap_err(),
            InterfaceError::AutoListenUnavailable
        );
    }
    assert!(
        snapshot
            .restore(&saved, AutoListenContext::Confirmed)
            .is_ok()
    );
    let non_reusable = DesktopSettings::new(
        Some(PORT.get()),
        vec![id("ephemeral")],
        true,
        false,
        LogLevel::Info,
    )
    .unwrap();
    assert_eq!(
        snapshot
            .restore(&non_reusable, AutoListenContext::Confirmed)
            .unwrap_err(),
        InterfaceError::AutoListenUnavailable
    );
}

#[test]
fn startup_requires_matching_evidence_through_binding() {
    let stable = fixture_interface("stable", vec![assigned("192.0.2.20", 24)]);
    let mut inventory = InterfaceInventory::default();
    inventory.refresh(Ok(vec![stable])).unwrap();
    let network = NetworkFingerprint::new([1; 32]);
    let saved = DesktopSettings::new(
        Some(PORT.get()),
        vec![id("stable")],
        false,
        false,
        LogLevel::Info,
    )
    .unwrap()
    .with_startup_network(Some(network))
    .unwrap();
    let snapshot = inventory.current().unwrap();
    assert_eq!(
        snapshot.restore_startup(&saved).unwrap_err(),
        InterfaceError::AutoListenUnavailable
    );
    inventory.update_startup_networks(BTreeMap::from([(id("stable"), network)]));
    let snapshot = inventory.current().unwrap();
    let auto = snapshot.restore_startup(&saved).unwrap();
    let manual = approve(snapshot, &["stable"]);
    assert!(BindingPlan::new(snapshot, &auto, GENERATION).is_ok());
    inventory.update_startup_networks(BTreeMap::from([(
        id("stable"),
        NetworkFingerprint::new([2; 32]),
    )]));
    let snapshot = inventory.current().unwrap();
    assert_eq!(
        BindingPlan::new(snapshot, &auto, GENERATION).unwrap_err(),
        InterfaceError::StaleInventory
    );
    assert!(BindingPlan::new(snapshot, &manual, GENERATION).is_ok());
    assert_eq!(
        snapshot.restore_startup(&saved).unwrap_err(),
        InterfaceError::AutoListenUnavailable
    );
}

#[test]
fn an_ambiguous_literal_address_cannot_be_bound_even_if_only_one_interface_was_selected() {
    let first = fixture_interface("first", vec![assigned("192.0.2.12", 24)]);
    let second = fixture_interface("second", vec![assigned("192.0.2.12", 28)]);
    let mut inventory = InterfaceInventory::default();
    let snapshot = inventory.refresh(Ok(vec![first, second])).unwrap();
    for names in [&["first"][..], &["first", "second"][..]] {
        let approval = approve(snapshot, names);
        assert_eq!(
            BindingPlan::new(snapshot, &approval, GENERATION).unwrap_err(),
            InterfaceError::DuplicateAddress
        );
    }
}

#[test]
fn malformed_or_oversized_provider_data_fails_closed() {
    let a = fixture_interface("wifi", vec![assigned("192.0.2.12", 24)]);
    let mut inventory = InterfaceInventory::default();
    assert_eq!(
        inventory
            .refresh(Ok(vec![a.clone(); MAX_INTERFACES + 1]))
            .unwrap_err(),
        InterfaceError::InventoryLimit
    );
    assert_eq!(
        inventory
            .refresh(Ok(vec![a.clone(), a.clone()]))
            .unwrap_err(),
        InterfaceError::DuplicateInterface
    );
    assert!(inventory.current().is_none());
    let recreate = |name, addresses| {
        Interface::new(
            a.id.clone(),
            name,
            a.persistence,
            a.indexes,
            a.status,
            addresses,
        )
    };
    assert_eq!(
        recreate("misleading\nname", vec![]).unwrap_err(),
        InterfaceError::InvalidName
    );
    assert_eq!(
        recreate(
            "name",
            vec![a.addresses[0]; MAX_ADDRESSES_PER_INTERFACE + 1]
        )
        .unwrap_err(),
        InterfaceError::InventoryLimit
    );
    assert_eq!(
        recreate("name", vec![a.addresses[0], a.addresses[0]]).unwrap_err(),
        InterfaceError::DuplicateAddress
    );
    let mut over_capacity = Vec::new();
    for interface in 0..3 {
        let mut addresses = Vec::new();
        for host in 1..=MAX_ADDRESSES_PER_INTERFACE {
            addresses.push(assigned(&format!("10.0.{interface}.{host}"), 24));
        }
        over_capacity.push(fixture_interface(&format!("wifi-{interface}"), addresses));
    }
    let snapshot = inventory.refresh(Ok(over_capacity)).unwrap();
    let exactly_at_limit = approve(snapshot, &["wifi-0", "wifi-1"]);
    assert_eq!(
        BindingPlan::new(snapshot, &exactly_at_limit, GENERATION)
            .unwrap()
            .bind_addresses()
            .len(),
        MAX_LISTENERS
    );
    let too_many = approve(snapshot, &["wifi-0", "wifi-1", "wifi-2"]);
    assert_eq!(
        BindingPlan::new(snapshot, &too_many, GENERATION).unwrap_err(),
        InterfaceError::ListenerLimit
    );
    assert_eq!(
        snapshot
            .approve(snapshot.revision(), &[id("wifi-0"), id("wifi-0")], PORT)
            .unwrap_err(),
        InterfaceError::InvalidSelection
    );
    assert_eq!(
        snapshot
            .approve(snapshot.revision(), &[id("missing")], PORT)
            .unwrap_err(),
        InterfaceError::InvalidSelection
    );
}

#[test]
fn interface_and_plan_diagnostics_redact_labels_indexes_and_addresses() {
    let record = fixture_interface("test-only-device", vec![assigned("192.0.2.12", 24)]);
    assert_eq!(format!("{record:#?}"), "Interface(<redacted>)");
    assert_eq!(
        format!("{:?}", record.indexes()),
        "InterfaceIndexes(<redacted>)"
    );
    assert_eq!(
        format!("{:?}", record.addresses()),
        "[AssignedAddress(<redacted>)]"
    );
    let mut inventory = InterfaceInventory::default();
    let snapshot = inventory.refresh(Ok(vec![record])).unwrap();
    let approval = approve(snapshot, &["test-only-device"]);
    let plan = BindingPlan::new(snapshot, &approval, GENERATION).unwrap();
    assert_eq!(format!("{plan:#?}"), "BindingPlan(<redacted>)");
}
