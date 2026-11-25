//! The worker reads Network List Manager metadata on its own COM apartment.

use super::{InterfaceError, MAX_INTERFACES};
use crate::interfaces::NetworkProfile;
use std::collections::BTreeMap;
use windows::Win32::Foundation::{VARIANT_BOOL, VARIANT_FALSE, VARIANT_TRUE};
use windows::Win32::Networking::NetworkListManager::{
    INetworkListManager, NLM_NETWORK_CATEGORY, NLM_NETWORK_CATEGORY_PRIVATE,
    NLM_NETWORK_CATEGORY_PUBLIC, NetworkListManager,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};

const MAX_CONNECTIONS: usize = 2 * MAX_INTERFACES;
pub(super) type Profiles = BTreeMap<u128, AdapterProfiles>;

#[derive(Default, Eq, PartialEq)]
pub(super) struct AdapterProfiles(BTreeMap<u128, NLM_NETWORK_CATEGORY>);

impl AdapterProfiles {
    pub(super) fn observe(
        &mut self,
        network: u128,
        category: NLM_NETWORK_CATEGORY,
    ) -> Result<(), InterfaceError> {
        if network == 0 {
            return Err(InterfaceError::ProviderUnavailable);
        }
        if let Some(previous) = self.0.insert(network, category)
            && previous != category
        {
            return Err(InterfaceError::ProviderChanged);
        }
        Ok(())
    }

    pub(super) fn classification(&self) -> NetworkProfile {
        if self
            .0
            .values()
            .any(|category| *category == NLM_NETWORK_CATEGORY_PUBLIC)
        {
            NetworkProfile::Public
        } else if self.0.len() == 1 {
            self.0
                .values()
                .next()
                .copied()
                .map_or(NetworkProfile::Unknown, classify)
        } else {
            NetworkProfile::Unknown
        }
    }

    pub(super) fn private_network(&self) -> Option<u128> {
        let (&id, &category) = self.0.first_key_value()?;
        (self.0.len() == 1 && category == NLM_NETWORK_CATEGORY_PRIVATE).then_some(id)
    }
}

struct Apartment;

impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: this guard is created after a successful CoInitializeEx on
        // the inventory worker and dropped there after all COM interfaces.
        unsafe { CoUninitialize() };
    }
}

pub(super) struct ProfileReader {
    manager: INetworkListManager,
    _apartment: Apartment,
}

impl ProfileReader {
    pub(super) fn new() -> Result<Self, InterfaceError> {
        // SAFETY: called only on the dedicated native worker. The reserved
        // pointer is null and a guard balances both S_OK and S_FALSE.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(|_| InterfaceError::ProviderUnavailable)?;
        let apartment = Apartment;
        // SAFETY: the current thread has an initialized COM apartment. Local
        // activation requests the typed interface without aggregation.
        let manager = unsafe { CoCreateInstance(&NetworkListManager, None, CLSCTX_INPROC_SERVER) }
            .map_err(|_| InterfaceError::ProviderUnavailable)?;
        Ok(Self {
            manager,
            _apartment: apartment,
        })
    }

    pub(super) fn read(&self) -> Result<Profiles, InterfaceError> {
        // SAFETY: all COM calls use the generated bindings on the apartment
        // that owns the live manager. Next receives one initialized output
        // slot and a writable fetched counter. All returned interfaces drop
        // on this worker before its apartment is uninitialized.
        unsafe {
            let connections = self
                .manager
                .GetNetworkConnections()
                .map_err(|_| InterfaceError::ProviderUnavailable)?;
            let mut profiles = Profiles::new();
            for index in 0..=MAX_CONNECTIONS {
                let mut fetched = 0;
                let mut slot = [None];
                connections
                    .Next(&mut slot, Some(&raw mut fetched))
                    .map_err(|_| InterfaceError::ProviderUnavailable)?;
                let [item] = slot;
                let connection = match (fetched, item) {
                    (0, None) => return Ok(profiles),
                    (1, Some(connection)) => connection,
                    _ => return Err(InterfaceError::ProviderUnavailable),
                };
                if index == MAX_CONNECTIONS {
                    return Err(InterfaceError::InventoryLimit);
                }
                if !is_connected(
                    connection
                        .IsConnected()
                        .map_err(|_| InterfaceError::ProviderUnavailable)?,
                )? {
                    continue;
                }
                let adapter = connection
                    .GetAdapterId()
                    .map_err(|_| InterfaceError::ProviderUnavailable)?
                    .to_u128();
                if adapter == 0 {
                    return Err(InterfaceError::ProviderUnavailable);
                }
                let network = connection
                    .GetNetwork()
                    .map_err(|_| InterfaceError::ProviderUnavailable)?;
                if !is_connected(
                    network
                        .IsConnected()
                        .map_err(|_| InterfaceError::ProviderUnavailable)?,
                )? {
                    return Err(InterfaceError::ProviderChanged);
                }
                let id = network
                    .GetNetworkId()
                    .map_err(|_| InterfaceError::ProviderUnavailable)?
                    .to_u128();
                let category = network
                    .GetCategory()
                    .map_err(|_| InterfaceError::ProviderUnavailable)?;
                profiles.entry(adapter).or_default().observe(id, category)?;
            }
            Err(InterfaceError::InventoryLimit)
        }
    }
}

fn is_connected(value: VARIANT_BOOL) -> Result<bool, InterfaceError> {
    match value {
        VARIANT_TRUE => Ok(true),
        VARIANT_FALSE => Ok(false),
        _ => Err(InterfaceError::ProviderUnavailable),
    }
}

fn classify(category: NLM_NETWORK_CATEGORY) -> NetworkProfile {
    match category {
        NLM_NETWORK_CATEGORY_PRIVATE => NetworkProfile::Private,
        NLM_NETWORK_CATEGORY_PUBLIC => NetworkProfile::Public,
        _ => NetworkProfile::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Networking::NetworkListManager::NLM_NETWORK_CATEGORY_DOMAIN_AUTHENTICATED;

    #[test]
    fn only_the_explicit_private_category_authorizes_default_proposal() {
        assert_eq!(
            classify(NLM_NETWORK_CATEGORY_PRIVATE),
            NetworkProfile::Private
        );
        assert_eq!(
            classify(NLM_NETWORK_CATEGORY_PUBLIC),
            NetworkProfile::Public
        );
        assert_eq!(
            classify(NLM_NETWORK_CATEGORY_DOMAIN_AUTHENTICATED),
            NetworkProfile::Unknown
        );
        assert_eq!(classify(NLM_NETWORK_CATEGORY(10)), NetworkProfile::Unknown);
        assert_eq!(
            is_connected(VARIANT_BOOL(1)),
            Err(InterfaceError::ProviderUnavailable)
        );
    }

    #[test]
    fn multiple_networks_for_an_adapter_cannot_establish_private_classification() {
        let mut profiles = AdapterProfiles::default();
        assert_eq!(profiles.classification(), NetworkProfile::Unknown);
        profiles.observe(1, NLM_NETWORK_CATEGORY_PRIVATE).unwrap();
        profiles.observe(1, NLM_NETWORK_CATEGORY_PRIVATE).unwrap();
        assert_eq!(profiles.classification(), NetworkProfile::Private);
        profiles.observe(2, NLM_NETWORK_CATEGORY_PRIVATE).unwrap();
        assert_eq!(profiles.classification(), NetworkProfile::Unknown);
        profiles.observe(3, NLM_NETWORK_CATEGORY_PUBLIC).unwrap();
        assert_eq!(profiles.classification(), NetworkProfile::Public);
        assert_eq!(
            profiles.observe(3, NLM_NETWORK_CATEGORY_PRIVATE),
            Err(InterfaceError::ProviderChanged)
        );
        assert_eq!(
            profiles.observe(0, NLM_NETWORK_CATEGORY_PRIVATE),
            Err(InterfaceError::ProviderUnavailable)
        );
    }
}
