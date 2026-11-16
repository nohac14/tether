# Mac Bonjour binding boundary review — 2026-10-25

**Task:** DS-002, accepted ADR 0003. This review precedes the local FFI code.
**Scope:** registration only; no browse, resolver, packet parser or mDNS daemon.
**Native status:** owner-approved en0/TCP 54322 registration, local A/AAAA and
service queries, browse add/removal and graceful cleanup now pass. Packet
containment, physical-phone resolution and packaged permissions remain untested.

## Binding choice and alternatives

Reviewed the official crates.io `async-dnssd` 0.5.1 source/manifest. It exposes
explicit-interface registration and custom-host record APIs, so it would be
incorrect to reject it as missing those operations. However, `ServiceStream`
uses an unbounded callback channel and Unix `ReadProcessor` drains while
readable without a work quota. Its shared connection starts a background Tokio
task. These do not establish Tether's bounded queue/work and single-owner
lifecycle properties. No unsoundness or exploitable vulnerability is claimed;
the version is not added, patched or adopted unchanged.

Use a minimal Mac-only DNS-SD ABI boundary linked to macOS `libSystem`, which
exports the DNS-SD API through its system umbrella. The six functions are CreateConnection,
RegisterRecord, Register, RefSockFD, ProcessResult and RefDeallocate. Avoid
additional API surface and external callback/runtime implementations. Existing
locked nix/libc bindings supply a zero-timeout readiness poll, not a new DNS
parser. There is no external installation script or unreviewed Git dependency.

## ABI and ownership requirements

Checked current official Apple documentation and the installed official SDK's
`dns_sd.h` for flags, integer widths, callbacks, shared references, threading,
records and readiness/deallocation. Native refs are opaque pointers,
flags/indexes/TTL u32, errors i32, and ports/lengths/types/classes u16.
Service ports use network byte order. No structure layout is guessed.

One parent connection owns all records and shared service operations. Always
set the scoped nonzero index, local domain, explicit stable host/instance and
NoAutoRename/ShareConnection service flags; record flags use Unique and TTL 120.
Validate every bounded string/address/scope before the first API call. A custom
SRV host requires explicit address records, not just service registration.

The driver is single-owner and not Send/Sync. Callback contexts are heap-stable
boxes with interior-mutable fixed result slots, outliving all native refs even
during rollback. Stop invalidates readiness, deallocates the parent once (which
invalidates every subordinate ref), then frees contexts. Never separately free
invalidated children, close the borrowed FD or deallocate during callbacks.
Callbacks never call DNS-SD, allocate, block, log or unwind.

Error callbacks read only error/context: other parameters are undefined.
Service success checks Add and exact bounded name/type/domain; reject implicit
rename/removal. Record flags are reserved, not service Add flags. Each slot
permanently latches its first failure; later success cannot erase it. Success
coalesces without an unbounded event queue, while failure cannot be discarded.
The core receives bounded summary state, not raw callback strings or secrets.

## Readiness, limits and failure

Native construction is inert. No create/register call occurs until a validated
plan and current scope are supplied. Process only after zero-timeout descriptor
readiness, at most eight processing calls per drive. Pending registration expires
after 30 seconds. No unbounded drain/retry, background callback task, registration
cache or unchecked counter is introduced.

The FD is borrowed and no persistent external reactor watcher is kept, avoiding
stale FD registration at deallocation. These are synchronous OS APIs: polling
readiness does not justify claiming immunity to an unresponsive daemon/OS. The
future DS-003 coordinator must isolate this owner from input safety and bound
waits externally, never concurrently freeing a blocked native call. No forced
cancellation or unsafe Send shortcut is permitted.

Test fake ABI calls/callbacks for exact flags/arguments/byte order, ownership,
delayed/repeated success, error-before-invalid-parameters, name mismatch,
removal, partial failure, quota/deadline exhaustion and stop/drop. Native symbol
linkage is separate from registration/permission, packet containment, Bonjour
coexistence and physical-phone hostname tests. DS-002 stays incomplete until
required scoped native evidence exists.

## Implementation evidence and corrections

The local boundary and fake ABI sink now implement the review's ownership and
bounds. Contexts use `Pin<Box<_>>`; a compile-fail doctest rejects Send. The SDK
compile-only fixture checks all six signatures, integer/flag/error values.
Native readiness tests use only a local Unix socket pair, not Bonjour.

The first link attempt incorrectly requested `-ldns_sd`, which is not a separate
SDK library on this Mac. It failed visibly with `library 'dns_sd' not found`.
Inspection of the official SDK's `libSystem.tbd` confirmed the exports; the
boundary now links `System` and the symbol-only constructor test passes.
This is a corrected linkage assumption, not a suppressed test or native
registration result. Strict test-harness lints were fixed without weakening
the workspace policy.

Final commands and results are in the [checkpoint](../manual-tests/2026-10-25-bonjour-boundary.md).
The [native procedure](../manual-tests/macos-bonjour-qualification.md) has an
initial scoped Mac pass, not a completed matrix. No wrapper package or new
registry package/version was added.

The first live callback validation failed closed: it assumed the reply service
type omitted the trailing DNS dot. Apple's official
[GenerateNTDResponse implementation](https://github.com/apple-oss-distributions/mDNSResponder/blob/main/mDNSShared/uds_daemon.c)
and [DNS name conversion](https://github.com/apple-oss-distributions/mDNSResponder/blob/main/mDNSCore/DNSCommon.c)
produce a fully dotted type. Updating the inert callback fixture to that form
made the original exact-scope test fail with InvalidCallback before correction.
The boundary now compares the exact canonical `_tether._tcp.`; the regression
and wrong-type/domain/extra-label rejection tests pass. No permissive wildcard
or implicit-rename acceptance was introduced. The native retry reaches Ready.

The initial auxiliary address-info counter missed the CLI's IPv6 display suffix;
[Apple's CLI formatter](https://github.com/apple-oss-distributions/mDNSResponder/blob/main/Clients/dns-sd.c)
adds a percent/interface presentation suffix. Direct A/AAAA queries already
matched both current addresses. A repeat address-info query with that diagnostic
format normalized reports both families, followed by graceful stop/no listener.
This is diagnostic-output handling only; production zone-ID URL rejection stays.

## Primary references

- [Apple service registration](https://developer.apple.com/documentation/dnssd/1804733-dnsserviceregister?preferredLanguage=occ).
- [Service callbacks](https://developer.apple.com/documentation/dnssd/dnsserviceregisterreply).
- [Record callbacks](https://developer.apple.com/la/documentation/dnssd/dnsserviceregisterrecordreply).
- [Processing](https://developer.apple.com/documentation/dnssd/dnsserviceprocessresult(_:)).
- [Deallocation](https://developer.apple.com/documentation/dnssd/dnsservicerefdeallocate(_:)).
- [Wrapper source](https://docs.rs/crate/async-dnssd/0.5.1/source/src/): boundedness rejection comes from the Cargo-distributed stream/Unix processor, not an inferred missing feature.
