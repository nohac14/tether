use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::num::{NonZeroU16, NonZeroU32, NonZeroU64};
use tether_config::{ApprovedInterfaceId, InstallationIdentity, LocalHostname};
use tether_network::interfaces::{AssignedAddress, BindingPlan, InterfaceInventory};

pub const SERVICE_TYPE: &str = "_tether._tcp.local";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoveryError {
    StaleBinding,
    InvalidName,
    MissingScope,
    UnrepresentableScope,
}

struct Endpoint {
    interface: ApprovedInterfaceId,
    index: Option<NonZeroU32>,
    socket: SocketAddr,
}

/// Data for one listener generation. Native publication still has to confirm
/// request readiness before the host is eligible as an HTTP authority.
pub struct Advertisement {
    generation: NonZeroU64,
    hostname: LocalHostname,
    instance_name: String,
    stable_instance: String,
    endpoints: Vec<Endpoint>,
}

impl Advertisement {
    /// Projects the exact addresses in a bound, current plan. Call after
    /// successful binding. Native interface indexes are from this inventory
    /// generation and never serve as persisted device identity.
    ///
    /// # Errors
    /// Refuses stale plans, inconsistent inventory and invalid conflict attempts.
    pub fn for_binding(
        binding: &BindingPlan,
        inventory: &InterfaceInventory,
        generation: NonZeroU64,
        identity: &InstallationIdentity,
        conflict: Option<NonZeroU16>,
    ) -> Result<Self, DiscoveryError> {
        if !binding.is_current(inventory, generation) {
            return Err(DiscoveryError::StaleBinding);
        }
        let snapshot = inventory.current().ok_or(DiscoveryError::StaleBinding)?;
        let preferred = identity.preferred_hostname();
        let hostname = match conflict {
            Some(attempt) => identity
                .conflict_hostname(attempt.get())
                .map_err(|_| DiscoveryError::InvalidName)?,
            None => preferred.clone(),
        };
        let label = host_label(&hostname)?;
        let instance_name = format!("{label}.{SERVICE_TYPE}");
        let stable_instance = host_label(&preferred)?.to_owned();
        let endpoints = binding
            .bind_addresses()
            .map(|socket| {
                let interface = snapshot
                    .records()
                    .iter()
                    .find(|interface| {
                        interface
                            .addresses()
                            .binary_search_by_key(&socket.ip(), AssignedAddress::address)
                            .is_ok()
                    })
                    .ok_or(DiscoveryError::StaleBinding)?;
                let index = match socket {
                    SocketAddr::V4(_) => interface.indexes().ipv4,
                    SocketAddr::V6(_) => interface.indexes().ipv6,
                };
                Ok(Endpoint {
                    interface: interface.id().clone(),
                    index,
                    socket,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if endpoints.is_empty() {
            return Err(DiscoveryError::MissingScope);
        }
        Ok(Self {
            generation,
            hostname,
            instance_name,
            stable_instance,
            endpoints,
        })
    }

    #[must_use]
    pub const fn generation(&self) -> NonZeroU64 {
        self.generation
    }

    #[must_use]
    pub fn hostname(&self) -> &str {
        self.hostname.as_str()
    }

    #[must_use]
    pub fn instance_name(&self) -> &str {
        &self.instance_name
    }

    /// Only fixed, public DNS-SD metadata. No stored credentials, raw
    /// installation identifier, interface IDs or user-chosen labels.
    #[must_use]
    pub fn txt_records(&self) -> [(&str, &str); 3] {
        [
            ("protocol", "1.0"),
            ("app", env!("CARGO_PKG_VERSION")),
            ("instance", &self.stable_instance),
        ]
    }

    /// Literal QR authorities remain available if discovery cannot publish a
    /// scoped name. Each host still has its own pairing and cookie scope.
    pub fn literal_endpoints(&self) -> impl ExactSizeIterator<Item = SocketAddr> + '_ {
        self.endpoints.iter().map(|endpoint| endpoint.socket)
    }

    /// Maps only shapes expressible as one request per interface in the
    /// documented Windows DNS-SD API. A rejected shape can use literal addresses.
    ///
    /// # Errors
    /// Rejects absent or ambiguous indexes, distinct family indexes on the
    /// same record, and more than one address of a family on the same scope.
    pub fn windows_registrations(&self) -> Result<Vec<WindowsRegistration>, DiscoveryError> {
        let mut registrations = Vec::<WindowsRegistration>::new();
        for endpoint in &self.endpoints {
            let index = endpoint.index.ok_or(DiscoveryError::MissingScope)?;
            let port =
                NonZeroU16::new(endpoint.socket.port()).ok_or(DiscoveryError::MissingScope)?;
            let position = registrations
                .iter()
                .position(|registration| registration.interface == endpoint.interface);
            let registration = if let Some(position) = position {
                let existing = &mut registrations[position];
                if existing.index != index || existing.port != port {
                    return Err(DiscoveryError::UnrepresentableScope);
                }
                existing
            } else {
                if registrations
                    .iter()
                    .any(|registration| registration.index == index)
                {
                    return Err(DiscoveryError::UnrepresentableScope);
                }
                registrations.push_mut(WindowsRegistration {
                    interface: endpoint.interface.clone(),
                    index,
                    port,
                    ipv4: None,
                    ipv6: None,
                })
            };
            let previous = match endpoint.socket.ip() {
                IpAddr::V4(address) => registration.ipv4.replace(address).map(IpAddr::V4),
                IpAddr::V6(address) => registration.ipv6.replace(address).map(IpAddr::V6),
            };
            if previous.is_some() {
                return Err(DiscoveryError::UnrepresentableScope);
            }
        }
        Ok(registrations)
    }
}

/// Inert Windows DNS-SD request data. Binding to the OS is a separate boundary.
pub struct WindowsRegistration {
    interface: ApprovedInterfaceId,
    index: NonZeroU32,
    port: NonZeroU16,
    ipv4: Option<Ipv4Addr>,
    ipv6: Option<Ipv6Addr>,
}

impl WindowsRegistration {
    #[must_use]
    pub const fn index(&self) -> NonZeroU32 {
        self.index
    }

    #[must_use]
    pub const fn port(&self) -> NonZeroU16 {
        self.port
    }

    #[must_use]
    pub const fn ipv4(&self) -> Option<Ipv4Addr> {
        self.ipv4
    }

    #[must_use]
    pub const fn ipv6(&self) -> Option<Ipv6Addr> {
        self.ipv6
    }
}

fn host_label(hostname: &LocalHostname) -> Result<&str, DiscoveryError> {
    hostname
        .as_str()
        .strip_suffix(".local")
        .ok_or(DiscoveryError::InvalidName)
}

impl fmt::Debug for Advertisement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Advertisement(<redacted>)")
    }
}

impl fmt::Debug for WindowsRegistration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WindowsRegistration(<redacted>)")
    }
}

#[cfg(test)]
mod tests;
