# Network foundation checkpoint — 2026-10-25

This is the earlier model checkpoint. The later
[Mac provider checkpoint](2026-10-25-interface-provider.md) supersedes its
NW-002 status and native-enumeration coverage, without changing these historical
test counts. DS-001's subsequent discovery contract and pre-code security,
privacy and resource contracts do not start a listener or discovery daemon.
The later [listener checkpoint](2026-10-25-listener-supervisor.md) completes
NW-003's scoped model/Mac-loopback verification and records the new DS-002
candidate blocker/proposed ADR 0003. No non-loopback service or discovery daemon
has started.

## Completed gates and implementation scope

- **BV-001 DONE (personal configuration):** the owner reports physical iPhone
  Safari on iOS 27.0.1 passing HTTP load, same-origin active-port WS/page-runtime
  round trips, Disconnect/reconnect, return from app switch/Wi-Fi change, and
  failure after Mac Ctrl-C server stop. No prompts were observed. The
  [evidence record](2026-10-25-iphone-safari-lan.md) retains missing build/model
  metadata and unexercised denial/reset cases instead of inventing validation.
- **NW-001 DONE:** [the network contract](../components/network-server.md)
  defines exact routes, interface/peer/authority boundaries, cookie delivery,
  approved port/collision behavior, bounds/deadlines/rates, redaction, emergency
  cleanup and the later implementation/test gates. The proposed configurable
  default is 54321/TCP, checked against IANA's Dynamic/Private range; neither
  that registry review nor the probe proves native firewall/package behavior.
- **NW-002 IN PROGRESS:** `tether-network` now implements the pure bounded,
  redacted inventory, identity-quality, flag/address-state, selection,
  persistence projection, literal binding-plan, and peer-admission policy.
  It opens no socket and performs no OS enumeration. Thirteen tests pass.
  Native adapters, actual OS-backed identity/context provenance, and live host
  provider verification remain unfinished; NW-003 is still blocked.

No authentication, pairing, production session/network service, native input
posting, or new listener was implemented or started in this checkpoint. The
owner's explicit physical test is separate from agent-run automation. Existing
Mac/NPM/probe worktree changes and approved architecture were preserved.

## Model safety and prerequisites

Records bound interfaces, IDs/names, addresses and choices; distinguish unknown
flags, temporary/tentative/deprecated addresses, stable/session-only identities,
and closed exclusions. Default proposals do not approve an interface. Manual
selection requires a local trusted-LAN consent value and current snapshot.
Binding plans also reject changed/reused records, duplicate addresses, overflow
and stale generations. Peer matching is tied to the accepted local address's
prefix, never the union of unrelated selected-interface subnets.

Settings projection preserves log/autostart preferences, reports session-only
choices explicitly, and cannot persist them as stable IDs or enable their
auto-listen. Automatic restoration also requires confirmed unchanged context
from the core/provider; unknown/changed context fails closed. These are pure
checks, not a claim that OS context detection already exists. Tests exercise
IPv4/IPv6 prefix extremes, public/global/ULA distinctions, false-safe defaults,
missing IDs, stale/reused snapshots, collection and binding saturation,
diagnostic redaction, and actual Unix atomic settings restart.

Only existing internal config/test-support dependencies were added. Cargo
regenerated their lockfile edges; no registry dependency, version, checksum,
policy exception, or native enumeration library changed. Public candidate APIs
were inspected, but no provider was selected: a library name/index is not
proof of persistent identity, and missing type/flag/lifetime metadata must not
be guessed. OS adapter source/feature/license review belongs to the next NW-002
step. No location/SSID access, packet probing, MAC-based identity, shell network
mutation, or unreviewed FFI was introduced.

Cross-target Windows Clippy initially failed on two pre-existing config helpers
that returned only Ok on unsupported platforms. Instead of suppressing the lint,
the non-Unix link-count and directory-sync helpers now fail, and same-file
comparison no longer returns true without proof. The existing early
PlatformPermissionEnforcementUnavailable boundary remains intact. The added
non-Unix helper regression compiles for Windows but did not execute natively.
R-018 is not resolved or worked around; no Windows data path is enabled.

## Actual verification

Host re-observed as macOS 15.7.5 (24G624), arm64. Hardware-model lookup was denied
by the sandbox; no broad access was requested for that optional metadata.
Rust/Cargo 1.99.0; frontend verification uses isolated official Node 24.19.0 and
pnpm 11.25.0, not the drifted bundled runtime. The source remains an uncommitted
worktree based on `94d8e49`; no signed app/package was tested.

| Check | Actual result |
| --- | --- |
| Network model tests | Passed: 13 including Unix settings restart; no sockets or input |
| Strict workspace/all-target/all-feature locked offline Clippy | Passed |
| Workspace/all-target/all-feature locked offline tests | Passed: 160, none failed or ignored |
| Workspace/all-target/all-feature build and warnings-denied Rust docs | Passed, locked offline; final aggregate build also passed |
| Windows/x86-64 network and config all-target strict Clippy | Passed after fail-closed stub correction; compile only, not Windows execution |
| Linux/x86-64 network all-target strict Clippy | Passed; compile only, not Linux execution |
| Pinned cargo-deny 0.20.2 bans/licenses/sources | Passed; reviewed duplicate warnings visible |
| Pinned cargo-deny advisories | **Failed**, unchanged RUSTSEC-2026-0370/0371/0372 in the Wayland graph; no ignore |
| Final aggregate `cargo xtask check` | Passed on pinned Node/pnpm; includes 160 Rust, 19 Vitest and four Node tests plus types/ESLint/Prettier/Markdown/links and skeleton frontend builds |
| Markdown/local links | Passed across 74 Markdown files |
| Local secret scanner | Passed: 165 text files, zero binary files skipped; not a Git-history scan |
| `git diff --check` | Passed |
| Actual OS enumeration, native input, packaged permissions, other platform/browser runtime, CI | Not run |

The full Rust suite includes only existing unposted Quartz inspection tests
with WindowServer access and the probe's loopback socket tests. No Accessibility
prompt or native event posting ran. The physical iPhone result does not prove
production protobuf, authentication/cookies, UI gestures/text, `.local`, or input
cleanup. The [development matrix](../operations/support-matrix.md) separates
these evidence categories.

## Remaining work and next dependencies

Continue NW-002 with reviewed OS providers and observed Mac inventory, keeping
unknown/stability/context handling conservative. Then NW-003 can implement the
listener supervisor. PR-002's schema baseline/atomic-click contract, R-019's
separate Wayland migration review, and F-005 CI remain unfinished. MAC-002 still
needs native media/onboarding/lifecycle integration and appropriate opt-in
validation. No user permission is required merely to continue the pure/model
or read-only provider preparation; no live input is implicitly authorized.

## Git boundary and current manual checkpoint commands

`.git` is read-only. No staging/commit permission bypass or push was attempted.
Review the diff before running these explicit groups. They include the older
pending personal-deployment/Mac/probe/npm work as well as this batch. The Rust
checkpoint deliberately keeps its shared manifest/lockfile graph together;
finer commit separation requires careful interactive staging, not a rewritten
lockfile or lost work. A checkpoint does not mark unfinished tasks DONE.

```sh
git diff --check
git add -- SPEC.md AGENTS.md docs/architecture-decisions.md docs/adr/0001-personal-use-quality-and-validation.md docs/adr/0002-iphone-safari-first-validation.md docs/components/README.md docs/components/macos-backend.md docs/components/browser-lan-probe.md docs/components/network-server.md docs/components/discovery.md docs/components/security-threat-model.md docs/components/privacy-and-logging.md docs/components/performance-budget.md docs/components/storage.md docs/dependency-inventory.md docs/dependency-policy.md docs/implementation-plan.md docs/open-risks.md docs/operations/support-matrix.md docs/manual-tests/environment-inventory.md docs/manual-tests/test-environment-setup.md docs/manual-tests/iphone-safari-lan-probe.md docs/manual-tests/2026-10-23-macos-backend.md docs/manual-tests/2026-10-25-iphone-probe-preparation.md docs/manual-tests/2026-10-25-iphone-safari-lan.md docs/manual-tests/2026-10-25-network-foundation.md docs/manual-tests/2026-10-25-interface-provider.md docs/reviews/2026-10-23-tooling-security-remediation.md
git add -- docs/adr/0003-macos-native-discovery.md docs/reviews/2026-10-25-mdns-candidate-review.md docs/manual-tests/2026-10-25-listener-supervisor.md
git commit -m "docs: record Mac network milestones and discovery decision"
git add -- package.json pnpm-lock.yaml eslint.config.mjs tooling/markdown-toolchain.test.mjs tooling/vite-toolchain.test.mjs
git commit -m "build: patch and verify frontend tooling security fixes"
git add -- Cargo.toml Cargo.lock deny.toml crates/config/src/store.rs crates/input-macos/Cargo.toml crates/input-macos/src/lib.rs crates/input-macos/src/backend.rs crates/input-macos/src/backend/tests.rs crates/input-macos/src/quartz.rs crates/input-macos/examples/macos_check.rs apps/desktop/src-tauri/Info.plist crates/network/Cargo.toml crates/network/README.md crates/network/src/lib.rs crates/network/src/interfaces.rs crates/network/src/interfaces/tests.rs crates/network/src/provider.rs crates/network/src/provider crates/network/examples/interface_inventory.rs tests/e2e/README.md tests/e2e/lan-probe tooling/lan-probe.test.mjs
git add -- crates/network/src/supervisor.rs crates/network/src/supervisor
git commit -m "feat: checkpoint Mac backend and guarded LAN interface foundation"
```

No credentials, private network addresses, local settings/identity, binaries,
caches, downloaded tools, or build artifacts belong in these commits. Do not
push without separate approval.
