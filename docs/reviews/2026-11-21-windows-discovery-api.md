# Windows discovery API review

This DS-002 review considers `DnsServiceRegister` as a Windows candidate
using Microsoft's published Win32 API contract. The Windows adapter remains
unselected and no DNS calls or multicast tests ran.

The registration request carries an explicit interface index and an mDNS
switch. Zero selects all interfaces. A candidate boundary needs to resolve
the current approved listener to a nonzero index and keep ownership tied
to that listener generation.
[Microsoft registration request](https://learn.microsoft.com/en-us/windows/win32/api/windns/ns-windns-dns_service_register_request)

The service instance accepts a custom hostname and exposes one IPv4 and one
IPv6 field. The published contract leaves open whether multiple registrations
can publish multiple addresses of the same family as one record set and
whether Windows preserves the requested name on collision. The experiment needs
peer queries on approved and unapproved interfaces, exact PTR/SRV/TXT and A/AAAA
answers, conflicts, and withdrawal after address changes. These results are
needed before selecting the adapter for Tether's listener policy.
[Microsoft service instance](https://learn.microsoft.com/en-us/windows/win32/api/windns/ns-windns-dns_service_instance)

A pending request can be cancelled and a registered instance can be removed
through asynchronous deregistration. The callback contract assigns ownership
of the returned instance to the application. The available documentation
leaves the cancellation and late-callback lifetime boundary open. A bounded
native driver needs source or isolated native evidence for that boundary
before freeing callback storage or starting a replacement generation.
[Microsoft registration](https://learn.microsoft.com/en-us/windows/win32/api/windns/nf-windns-dnsserviceregister)
[Microsoft cancellation](https://learn.microsoft.com/en-us/windows/win32/api/windns/nf-windns-dnsserviceregistercancel)
[Microsoft deregistration](https://learn.microsoft.com/en-us/windows/win32/api/windns/nf-windns-dnsservicederegister)
[Microsoft completion callback](https://learn.microsoft.com/en-us/windows/win32/api/windns/nc-windns-dns_service_register_complete)

The existing [network contract](../components/network-server.md) and
[ADR 0003](../adr/0003-macos-native-discovery.md) still govern approved-interface
exposure and withdrawal. The Windows discovery and LAN integration gates stay
open until a platform candidate satisfies them.

## Inert Windows verification

The Windows SDK constructor allocated the prepared IPv4-only and dual-stack
records. Tests checked the copied service and host names, bounded TXT fields,
addresses, port and zero initial interface index. The tests freed each record
through the SDK and invoked no registration, cancellation, or network query.
An approved interface index will need to be supplied separately when the
request is registered. The shared projection rejects unsupported scopes without
dropping any endpoint from the literal-IP fallback.
[Microsoft constructor](https://learn.microsoft.com/en-us/windows/win32/api/windns/nf-windns-dnsserviceconstructinstance)
[Microsoft destructor](https://learn.microsoft.com/en-us/windows/win32/api/windns/nf-windns-dnsservicefreeinstance)
