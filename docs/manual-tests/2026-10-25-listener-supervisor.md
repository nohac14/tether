# Listener supervisor and discovery review checkpoint — 2026-10-25

## Tasks and environment

NW-003 is complete for the shared model/Mac-first path under ADR 0001.
DS-002 is **BLOCKED**, not DONE: the reviewed mdns-sd 0.21.6 startup ordering
does not satisfy approved-interface exposure. Proposed ADR 0003 needs owner
approval before substituting a Mac-native discovery adapter. No existing
architecture decision or input/security requirement changed.

Host: macOS 15.7.5 (24G624), arm64 Apple Silicon; exact model unavailable,
owner-reported 16 GiB RAM. Rust/Cargo 1.99.0. Pinned frontend runtime remains
official Node 24.19.0/pnpm 11.25.0. Source is the uncommitted worktree based on
`94d8e49`, not a signed or end-to-end runnable application. Preserve the older
dirty Mac/probe/provider/tooling work. No LAN address or secret is in this record.

## Completed NW-003 acceptance criteria

- Serialized stop/start/accept ownership with complete-set bind rollback and
  checked generation/counter values. No bind before valid approved selection,
  required settings-commit integration and independent safety acknowledgement.
- Reserved nonblocking safety interface, owned non-cancelling cleanup ticket,
  immediate logical admission/generation invalidation and two-second cleanup
  deadline. Repeated/concurrent stops coalesce without extending it. A pending
  start can be cancelled; a terminal fault cannot become resumable after cleanup.
- Fresh inventory before/after binding, while idle and on acceptance. Removed,
  changed, stale or unavailable metadata stops admission and requests cleanup
  before HTTP allocation. No wildcard, automatic port fallback or partial-ready
  address set is published.
- Shared 64-global/eight-per-peer transport quotas, RAII lifetime and monotonic
  connection IDs across generations. Old leases cannot remove a new entry.
  Every accepted connection remains generation-checked, not authenticated.
- All retained peer transports close after neutral acknowledgement or force-close
  on fault/timeout; a failed/panicking close cannot starve other closures.
- Mac-native literal nonblocking Tokio/socket2 backend, v6-only IPv6 and owned
  shutdown handles. Actual tests cover port collision, restart with TIME_WAIT,
  rollback, native endpoint validation/rejection, EOF and idempotent shutdown.

The actual core/session coordinator, independent deadline driver and HTTP/WS
handlers are future integration tasks, not simulated production features.
Their contracts must retain these invariants. The dev-only adapter exercises
the real input dispatcher with a held mock button and proves neutral cleanup
after dropping the supervisor; no native input is posted.

## Actual verification

| Check | Actual result |
| --- | --- |
| Network all-target tests | 49 pass: 27 prior policy/provider tests plus 22 supervisor tests; no failures/ignored tests |
| Native socket execution | Four real loopback-only Mac tests: collision/accept/restart/shutdown, v6-only option, full-supervisor peer rejection and partial-bind rollback; one additional Mac guard test rejects targets/missing runtime before socket creation |
| Rust formatting | `cargo fmt --all --check` passes |
| Strict workspace/all-target/all-feature Clippy | Passes, locked offline |
| Workspace/all-target/all-feature tests | 196 pass, none failed/ignored, locked offline; existing Quartz inspection has WindowServer access but never posts events |
| Workspace/all-target/all-feature build | Passes, locked offline |
| Warnings-denied Rust docs | Pass, workspace/no-deps/all-features, locked offline |
| Windows/x86-64 and Linux/x86-64 network all-target strict Clippy | Pass; compile only, no native socket/provider/backend/runtime claim |
| Pinned cargo-deny bans/licenses/sources | Pass; reviewed duplicate syn/getrandom warnings remain visible |
| Refreshed cargo-deny advisories | **Fail**, unchanged RUSTSEC-2026-0370/0371/0372 in Wayland dependencies; no ignored finding |

The initial native restart assertion failed with Unix TCP TIME_WAIT. The code
was fixed to enable SO_REUSEADDR on the Mac backend while keeping SO_REUSEPORT
disabled. The original restart assertion and live-listener collision assertion
both pass; no failure was waived or converted to an unavailable/manual check.
Routine Clippy/test compilation issues were corrected without lint suppression.

The pinned-runtime aggregate `cargo xtask check` passed: 196 Rust, 19 Vitest
and four Node tests; types, ESLint, Prettier, Markdown, local links across 82
Markdown files and skeleton frontend builds. The local worktree secret scanner
passed across 183 text files with zero binary skips; it does not scan history.
Locked offline Cargo metadata and `git diff --check` passed.
The full command set does not start a non-loopback
listener or multicast daemon. No Accessibility prompt/input posting, packaged
Local Network permission, firewall, physical DHCP/interface/IPv6 LAN change,
network namespace, Windows/Linux runtime, CI, signing or phone product test ran.

## DS-002 source evidence and decision

The official Cargo archive was reviewed without adding it to the workspace or
constructing its daemon. Worker startup attempts multicast setup before queued
interface controls are processed, so a disable-immediately wrapper cannot
guarantee the approved subset. Channels are bounded (100 commands/1,000 events);
cache/lifecycle/resource claims were not invented. The exact version/hash and
source locations are in [the review](../reviews/2026-10-25-mdns-candidate-review.md).

Proposed [ADR 0003](../adr/0003-macos-native-discovery.md) recommends evaluating
the OS Bonjour interface-scoped registration APIs behind the same discovery
boundary. Official Apple API documentation and installed SDK confirm the
interface-index/custom-host-record distinctions, not runtime success. No new
native DNS-SD binding/FFI or alternative daemon is installed or implemented.

## Blockers and next dependency-ready work

DS-002/DS-003 require the new adapter decision and later opt-in native tests.
R-018 Windows storage remains fail-closed; R-019's three Wayland maintenance
advisories remain open. F-005 CI and PR-002 schema/baseline are unfinished.
NW-004 static asset delivery is dependency-ready but not implemented in this
batch. Authentication/pairing/session gates remain unchanged. No broad platform
support or complete Mac application claim follows from these checks.

Work pauses for the concrete new discovery decision, not for routine coding
approval. D-01–D-05 remain intact and the authoritative SPEC is unchanged.

## Git and changed paths

`.git` is read-only; no staging/commit bypass, history rewrite or push occurred.
The new implementation paths are `crates/network/src/supervisor.rs` and its
`supervisor/` native/test modules. Updated network manifest/lockfile edges reuse
existing dependency versions. Contracts, support/risk/plan/inventory documents,
this record, candidate review and proposed ADR document the behavior and limits.

Review the diff and use the explicit grouped checkpoint commands in
[the foundation record](2026-10-25-network-foundation.md), which include earlier
pending work and these additions. If the earlier groups have already been
committed manually, stage only this batch's paths and their shared lockfile:

```sh
git diff --check
git add -- Cargo.lock crates/network/Cargo.toml crates/network/README.md crates/network/src/lib.rs crates/network/src/interfaces.rs crates/network/src/supervisor.rs crates/network/src/supervisor docs/components/network-server.md docs/components/discovery.md docs/dependency-inventory.md docs/implementation-plan.md docs/open-risks.md docs/architecture-decisions.md docs/operations/support-matrix.md docs/manual-tests/2026-10-25-network-foundation.md docs/manual-tests/2026-10-25-listener-supervisor.md docs/reviews/2026-10-25-mdns-candidate-review.md docs/adr/0003-macos-native-discovery.md
git commit -m "feat: verify Mac listener supervision and record discovery blocker"
```

No credentials, local settings/identity, private network data, caches, downloads
or build artifacts belong in a commit. No push is authorized by these commands.
