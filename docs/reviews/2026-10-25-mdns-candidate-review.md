# DS-002 discovery candidate review — 2026-10-25

**Disposition:** NO-GO for adopting mdns-sd 0.21.6 unchanged. DS-002 remains
BLOCKED; no dependency, daemon, advertisement or native validation is added.
This is a source review, not the independent Gemini review or a runtime audit.

## Provenance and scope

Standard `cargo info mdns-sd@0.21.6 --registry crates-io` fetched the official
registry package into Cargo's ordinary cache. Archive SHA-256:
`8562654710f72a9cbce4b0d5b7c366a2f94330ba1ce3740f15e932ea2d37b161`.
No checksum was hand-edited. Neither Cargo.lock nor the discovery manifest
contains this candidate. Its declared MSRV is 1.71 and license is MIT OR
Apache-2.0. Those facts do not establish a compatible or secure runtime graph;
the candidate/transitives were not compiled or advisory-validated for adoption.

Reviewed the versioned package's `service_daemon.rs` constructor, worker setup,
interface selection/command processing and channels, `dns_cache.rs` cache
structure/insertion/expiry, and manifest features. This is not a claim to have
audited every parser/unsafe/platform path. Initial documentation URL fetches
failed; subsequent official Apple search results verified scoped registration
and custom-host record requirements. The exact Cargo-distributed source and
installed Apple SDK header supplied version-specific worker/ABI evidence.

## Confirmed startup ordering conflict

The versioned source establishes this order:

1. `ServiceDaemon::new` delegates to `new_with_port` (lines 326–327).
2. `new_with_port` creates a loopback signal socket and bounded command channel,
   then immediately spawns `daemon_thread` (lines 361–396). There is no initial
   selected-interface policy argument or paused builder.
3. `daemon_thread` calls `Zeroconf::new` before `run` consumes commands
   (lines 850–862).
4. `Zeroconf::new` enumerates interfaces, attempts wildcard IPv4/IPv6 multicast
   sockets and joins the initial interfaces, with an initially empty selection
   filter meaning all interfaces (lines 1359–1421).
5. `enable_interface` and `disable_interface` only enqueue SetOption commands
   (lines 736–754), handled later by the already-started worker.

Even if disabling all interfaces is the client's next statement or was queued
before the worker was scheduled, the worker performs its initial setup first.
This is not fixed by calling the methods quickly, choosing explicit advertised
addresses, disabling automatic A/AAAA discovery, or delaying service registration.
No Tether record need be advertised for the initial all-interface socket/group
activity to violate the selected-interface startup contract.

This is a concrete incompatibility with D-03 and DS-001's pre-worker scoping,
not a stylistic objection to the library. A custom UDP port is not a production
solution: normal `.local` clients use standard mDNS, and the same ordering
still exists. Wildcard UDP sockets can legitimately be necessary for multicast;
their existence is not itself the TCP no-wildcard defect. The specific defect
is missing pre-start interface/membership/admission restriction. Do not change
D-03 or conflate OS-wide existing Bonjour activity with new Tether activity.

## Resource findings and limits of this review

The command channel is bounded to 100; event channels use capacity 1,000
(`EVENT_CHANNEL_CAPACITY`, line 89), with bounded status/unregister/shutdown
responses as well. It would be inaccurate to label all its channels unbounded.
Tether's adapter mailbox remains capped at 64; any internal buffers also need
accounting and overflow/lifecycle tests before approval.

The cache uses maps/vectors with TTL eviction and rejects new unsolicited
entries when not queried by default. No configurable total cache/retained-byte
ceiling was identified in the reviewed API/insertion paths. That is a remaining
validation gap, not proof that a publishing-only application is exploitable or
that every received record is cached. The startup ordering failure alone is
sufficient to reject this version unchanged; no invented cache exploit or
blanket failure claim is needed.

No physical multicast capture, Bonjour coexistence, name conflict, permission
denial, native A/AAAA resolution or goodbye test ran. We deliberately did not
construct the daemon because its startup scope is the issue under review.

## Proposed resolution requiring owner approval

Do not fork, patch, replace, add a Git dependency or relax exposure requirements
silently. [Proposed ADR 0003](../adr/0003-macos-native-discovery.md) recommends
evaluating a Mac-native Bonjour adapter behind the same discovery contract,
with source/binding review and opt-in native verification still required.
An upstream mdns-sd release with a reviewed pre-start allowlist and acceptable
resource behavior remains a legitimate alternative; no claim is made that such
an API exists in 0.21.6.

The installed official macOS SDK's `dns_sd.h` documents nonzero interface-index
registration, explicit SRV target/record registration and no-auto-rename flags.
It also warns that custom host targets do not automatically create A/AAAA
records, event processing can block if called without readiness, and callback
processing/deallocation must be serialized. Those are spike requirements, not
an assumption that replacing the crate will trivially pass all gates.

## Primary source locations

- [Versioned daemon source](https://docs.rs/crate/mdns-sd/0.21.6/source/src/service_daemon.rs).
- [Versioned cache source](https://docs.rs/crate/mdns-sd/0.21.6/source/src/dns_cache.rs).
- [Official package metadata](https://crates.io/crates/mdns-sd/0.21.6).
- [Apple scoped registration](https://developer.apple.com/documentation/dnssd/1804733-dnsserviceregister?preferredLanguage=occ): interface selection and custom-host records, corroborated by the installed official SDK header; no API was live-called.
