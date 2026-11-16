# Read-only Mac interface provider checkpoint — 2026-10-25

## Scope and environment

NW-002's cross-platform policy model is complete, with a native read-only macOS
provider for the ADR 0001 personal-use path. Windows/Linux provider adapters
remain unimplemented, not validated or silently substituted with shell output.
No listener, advertisement, authentication, pairing, session or native input
posting was added or started. The application is not yet end-to-end runnable.

Observed host: macOS 15.7.5 (24G624), Darwin arm64, Apple Silicon. Exact hardware
model was not available from the sandbox; owner reports 16 GiB RAM. Source is
the uncommitted worktree based on `94d8e49`. Rust/Cargo 1.99.0, Node 24.19.0 and
pnpm 11.25.0 were used. No signed package or packaged privacy flow was tested.

## Actual host observation

Executed the following read-only command, without elevated OS access:

```sh
cargo run -p tether-network --example interface_inventory --locked --offline -- --summary
```

It reported 22 interfaces, four persistent service associations, 18 session-only
identities, one proposed candidate, one Wi-Fi classification, four Ethernet
classifications, two usable IPv4 addresses and two usable IPv6 addresses.
The first implementation returned identical contents with increasing revisions.
Pre-bind integration review identified that this would spuriously invalidate
unchanged foreground approval. A regression failed before correction (revision
1 versus 2), then passed after making revisions stable for unchanged inventories.
The final native run reported identical consecutive contents with preserved
revision. Actual record/index change or recovery after failure still advances it.
The final diagnostic also reported 15 IPv6 addresses with successfully queried
preferred state, zero unknown/ineligible-state results; temporary/address/interface
exclusions still apply independently. This is not an IPv6 reachability test.
Address counts are individual eligibility counts, not a claim that every
interface qualifies or that IPv6 connectivity was tested. Routine output
contains no interface labels, addresses, MACs, service IDs, SSIDs or secrets.

The provider uses an unbound socket only for a local read-only ioctl; it never
binds, sends or listens. No permission prompt was observed. That observation
does not establish packaged macOS Local Network permission or Accessibility.
No interface was disconnected/renamed, OS service edited, trust continuity
inferred, autolistening enabled or address change induced on the real host.

## Automated verification

| Check | Actual result |
| --- | --- |
| Network tests | 27 pass: 13 prior policy/storage tests and 14 new provider translation/ABI/eligibility tests; no native enumeration or injection in unit tests |
| Formatting | `cargo fmt --all --check` passes |
| Strict workspace/all-target/all-feature Clippy | Passes, locked offline |
| Workspace/all-target/all-feature tests | 174 pass, none failed/ignored; existing Quartz tests construct/inspect unposted events with WindowServer access |
| Workspace/all-target/all-feature builds | Pass, locked offline |
| Warnings-denied Rust documentation | Pass, all workspace/no-deps/all-features, locked offline |
| Windows/x86-64 and Linux/x86-64 network all-target strict Clippy | Pass; compile-only, not native provider/runtime validation |
| cargo-deny 0.20.2 bans/licenses/sources | Pass; existing syn/getrandom duplicate warnings remain visible |
| Refreshed cargo-deny advisories | **Fail** on unchanged RUSTSEC-2026-0370/0371/0372 in the Wayland graph; no ignored finding |
| Locked offline Cargo metadata | Pass; no dependency source/checksum bypass |
| Pinned-runtime aggregate `cargo xtask check` | Pass: 174 Rust, 19 Vitest and four Node tests, types, ESLint, Prettier, Markdown, local links and skeleton frontend builds |
| Markdown/local links | Pass across 79 Markdown files, including DS-001 and network pre-code contracts |
| Local secret scanner | Pass: 175 text files, zero binary skips; not history scanning |
| Whitespace | `git diff --check` passes |

The initial advisory run could not acquire its read-only cache lock. A narrowly
approved ordinary-cache run refreshed the database and produced the reported
three failures. No TLS/DNS/source policy or advisory rule was bypassed.

The regression suite specifically distinguishes failed IPv6 queries from valid
zero flags, excludes temporary/duplicate/optimistic/unknown states, verifies
the Darwin buffer/request ABI, keeps ambiguous services session-only, retires
fallback IDs after disappearance/failure, rejects duplicate/malformed/oversized
metadata, tests counter exhaustion and validates contiguous actual netmasks.
Seeded fallback entropy is test-only; production uses getrandom's OS backend.

## Limits and next gates

Stable service association is not network-trust evidence. Automatic restoration
remains closed with unknown continuity until the core establishes it
independently; this provider does not collect SSIDs or manufacture that proof.
Native APIs do not offer a single inventory transaction. Configuration/index
rechecks detect observable changes, but NW-003 must refresh before binding and
DS-003 must coordinate later changes, releases and stale-session closure.

Darwin's close-on-exec descriptor flag is applied immediately but not atomically
with socket creation. The future app spawn layer must use close-default spawn
semantics or equivalent serialization; no child is spawned by this provider.
Physical unplug/DHCP/VPN/service-change and live IPv6 tests remain unperformed.
R-018 Windows storage and R-019 Wayland dependency maintenance remain open.
Paid/public signing gates remain deferred, not passed. No approved architecture
decision was changed.

DS-001's discovery contract is now complete, including metadata, interface scope,
conflict/resource bounds, DHCP generation transitions, TTL/withdrawal and IP
fallback. Official RFC and current mdns-sd documentation were checked without
installing a discovery dependency or starting a daemon. The cross-cutting
security-threat, privacy and resource contracts required before network code
are now present and verified. DS-002 must still validate pre-worker scoping and
internal bounds; no assumed constructor/interface ordering is accepted.

NW-003 listener supervision and DS-002 candidate validation are dependency-ready
on the Mac-first path after this checkpoint. Production auth/session routes
remain behind their original prerequisites. Only loopback/mock automation may
run without renewed explicit LAN/native-test opt-in.

## Git checkpoint

`.git` remains read-only; no permission bypass, staging, commit or push was
attempted. Preserve all older dirty files. The existing checkpoint's manual
commands are in [the foundation record](2026-10-25-network-foundation.md).
Include this provider record plus the following new Rust paths in that review:
`crates/network/src/provider.rs`, `crates/network/src/provider/`, and
`crates/network/examples/interface_inventory.rs`. Cargo.lock and the network
manifest belong in the same dependency/code checkpoint. No binaries, caches,
private metadata or local settings belong in the commit.
