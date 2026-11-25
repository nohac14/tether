# Windows interface inventory checkpoint

The Windows 11 Pro 25H2 x64 checkout contains the restored interface policy
and a read-only IP Helper provider. The earlier Mac supervisor and socket
adapter source is still absent from the tracked checkout.

The policy validates bounded interface records and exact, revisioned approval
for literal listener addresses. It rejects stale generations, duplicate
addresses even on unselected interfaces, IPv4 subnet boundaries, and peers
outside the accepted local socket's prefix. IPv4-mapped peers share the
canonical IPv4 identity. Saved IDs alone cannot restore automatic listening.

The provider uses the SDK's no-statistics interface table level and the
assigned unicast IPv4 and IPv6 tables. The scoped RAII wrappers release each
native table. Two successive samples must agree on the fields relevant to
binding. The native adapter keeps per-interface GUIDs, indexes and typed
eligibility in memory. It retires the previous revision on observed changes
or failure. [Interface table API](https://learn.microsoft.com/en-us/windows/win32/api/netioapi/nf-netioapi-getiftable2ex)
[Unicast address API](https://learn.microsoft.com/en-us/windows/win32/api/netioapi/nf-netioapi-getunicastipaddresstable)

## Native observation

```powershell
cargo run -p tether-network --example windows_interface_inventory --locked --offline
```

The diagnostic read 55 interfaces, two eligible address entries and six IPv6
entries. The repeat scan preserved the inventory revision. There were zero
default proposals because this provider reports Windows network profiles as
unknown. Foreground interface selection and network-profile classification
remain part of the desktop integration work. IPv6 entries counted in this run
include ineligible addresses. The diagnostic reported only totals and did no
socket binding, packets, network changes or input injection.

## Verification

- Fifteen network tests passed, including eight shared policy tests, two
  Windows record translation tests and five existing asset tests. The locked
  offline Windows workspace suite passed 202 tests as a standard user. The
  existing explicit protocol fixture generator remained opt-in.
- Rust formatting, strict all-target/all-feature Clippy, the workspace build,
  and warnings-denied network rustdoc passed using Rust 1.99.0 and MSVC.
- The locked, cached Windows dependency audit passed. The full configured
  graph still fails advisories on RUSTSEC-2026-0370, RUSTSEC-2026-0371
  and RUSTSEC-2026-0372 in the Wayland graph. Bans, licenses and sources pass.
- No profile API, live listener, firewall, DHCP change, physical IPv6
  reachability, native input, phone pairing or installer was tested.
