# Dependency-ordered implementation plan

**Plan status:** Architecture accepted; personal-deployment scope follows ADR
0001 (2026-10-23), without waiving functional/input-security requirements.
Accepted ADR 0004 (2026-11-14) now prioritizes Windows 11 x86-64 delivery, then
macOS, and defers Linux work. An unvalidated Windows artifact is permitted,
not a completed/support-validated application. SDK/CRT license use and scoped
build-tool acquisition are owner-approved; R-018 native storage-security evidence
remains a separate gate. Continue
dependency-ready shared work; do not mark native tests passed or silently
enable Windows persistence to manufacture a runnable artifact.
A-004, F-001, F-003, F-006, PR-001,
IC-001, IC-002, IC-003, IC-004, ST-001, ST-002, DT-001, and F-004 are
complete. ST-003 and NB-001 are also complete. The Wave 8 inert translation
milestone is implemented and independently reviewed; MAC-001 is complete,
while Windows/X11/Wayland native-environment portions remain blocked and are not
support claims. Gemini accepted that milestone in its 2026-10-16 Wave 8 review.
F-002 is complete after the owner-approved ESLint pin correction, Markdown CLI
replacement, and three scoped parser security overrides. Its historical
2026-10-16 224-entry npm graph passed frozen installation, npm audit and the
aggregate repository check. F-005 is dependency-ready but not
implemented or remotely executed. The 2026-10-16 cargo-deny pass is historical:
the 2026-10-23 refresh fails on three new unmaintained Wayland D-Bus advisories
(R-019); no ignore or silent dependency migration is approved. MAC-002 is now
in progress: the live Quartz backend and inert safety tests are implemented,
but actual posting, remaining onboarding/lifecycle wiring, and complete app
integration are not validated.
R-020's scoped remediation now resolves Vite 8.1.6 and markdown-it 14.3.3 with
all other package versions preserved. Frozen installation and npm audit pass
on restored, checksum-verified Node 24.19.0/pnpm 11.25.0. See
`reviews/2026-10-23-tooling-security-remediation.md` for the final gate results;
the separate R-019 failure still prevents a clean whole-project advisory gate.
ADR 0002 now makes the owner's physical iPhone/Safari the first BV-001 gate.
Its isolated Axum probe is prepared with loopback and browser-state tests;
the owner reports iOS 27.0.1 Safari successfully loading it and completing
same-origin JavaScript round trips over Wi-Fi, with Disconnect stopping the
count and no prompts observed. The owner explicitly confirmed the four
reconnect/app-switch/Wi-Fi/server-stop follow-up checks all passed. BV-001 is
complete for that configuration under ADR 0002. NW-001's bounded network
contract and documentation checks are complete. NW-002's shared policy/settings
model and read-only Mac provider are complete, including actual local inventory
and 27 network tests. Unchanged polls preserve approval revisions; changes or
recovery after failure invalidate them. Windows/Linux adapters remain unimplemented. DS-001's
bounded discovery contract is complete. NW-003's serialized supervisor, Mac
socket adapter and safety interface now pass 49 network tests, including four
real loopback tests and actual held-state mock-dispatcher cleanup. NW-004 is
dependency-ready. DS-002 source review rejects mdns-sd 0.21.6 unchanged: its
initial multicast setup precedes interface restrictions. The owner accepted
ADR 0003 on 2026-10-25. The Mac-native Bonjour qualification boundary now
compiles and passes inert tests. The separately approved en0/TCP 54322 diagnostic
now passes initial Mac registration, scoped A/AAAA/SRV/TXT, browse add/removal and
graceful cleanup. DS-002 remains in progress pending containment, permission,
conflict/lifecycle and physical resolution evidence; DS-003 remains blocked.
No production LAN service or Tether multicast daemon has been started. Cross-cutting
network threat/privacy/resource contracts remain in force. Auto-listen
restoration remains closed when network
continuity is unknown; matching service IDs alone never establish continuity.
Android Chrome LNA remains an expansion gate. No production networking task is
advanced by an automated probe pass or incomplete physical evidence.
Production sessions, HTTP/WS route serving, pairing, authentication, live native
injection and user interfaces are not implemented. The application is not yet
end-to-end runnable.

The 2026-11-14 batch resolves F-007/R-022 through four approved minimatch
overrides with source/license review, compatibility tests and a clean npm audit.
PR-002's explicit atomic-click flag and frozen Buf FILE baseline pass their
regressions. PR-003 now implements pinned offline Rust/TypeScript generation,
redacted Rust diagnostics and generated-diff checks; the npm graph has 230
entries. Only the owner-approved expectation on the generated capability
message is allowed; strict workspace lint policy remains unchanged. Current
full-graph cargo-deny still fails solely on R-019; separate Windows/macOS target
checks pass and do not replace that failure. WB-001 qualifies cargo-xwin/LLVM
tools but is blocked on R-023 Microsoft manifest digest mismatches. No SDK,
NSIS installer or Windows executable is claimed. See the
[dated checkpoint](manual-tests/2026-11-14-protocol-tooling.md).

On 2026-11-16 normal Git approvals succeeded. PR-003 is now committed as
`a5a8da0`. PR-004 subsequently completed bounded wire/domain validation. The
[commit verification record](manual-tests/2026-11-16-git-protocol-commit.md)
distinguishes the isolated committed snapshot from older unstaged platform
work. No push or global dependency-security waiver occurred.
The [validation checkpoint](manual-tests/2026-11-16-protocol-validation.md)
records full-worktree and isolated-candidate evidence. SS-001 subsequently
completed the session state/release-trigger contract, including the distinct
admission projection required for queued down/up transitions. PR-005's local
corpus is committed, with remote CI acceptance pending. SS-002 now includes
non-waiting receipts and transactional connection admission; its actual actor,
queues and coordinator remain incomplete. PA-001 is dependency-ready. No actual
session service or credential service is implied.
**Delivery target:** The complete production application in `SPEC.md`; no task
authorizes a disposable transport, backend, or MVP substitute.

## Status rules

- `BLOCKED`: a dependency or owner decision is unresolved.
- `READY`: all dependencies and required component documents are accepted.
- `IN PROGRESS`: implementation and verification are active.
- `DONE`: code, tests, builds, documentation, and stated evidence all pass.
- A task cannot be marked done with a suppressed failure or an unrecorded manual
  gap.
- Real-device/native validation is recorded separately from automated success.

The tables are ordered by dependency. Tasks in the same wave may proceed in
parallel only after their shared interfaces are accepted and edits to schemas,
workspace manifests, and build configuration are coordinated.

## Wave 0 — Approval and executable architecture

| ID | Status | Depends on | Deliverable and completion evidence |
| --- | --- | --- | --- |
| A-001 | DONE | Owner | D-01–D-05 were explicitly approved on 2026-10-09 and recorded in `architecture-decisions.md`. |
| A-002 | DONE | A-001 | Reconciled both reviews into `SPEC.md`, including accepted/corrected/rejected findings, with rationale in `architecture-decisions.md`. Documentation-only validation passed. |
| A-003 | DONE | A-002 | Finalized the system/trust boundaries, threat and support policy, repository/documentation structure, test strategy, and risk gates in the authoritative and supporting documents. |
| A-004 | DONE | A-003 | Recorded the actually observed local toolchain/platform state and every required but unavailable signed identity, physical device, clean OS, browser, Linux desktop/portal, and network topology in `manual-tests/environment-inventory.md`. No unavailable environment is counted as validation or a support claim. |

## Wave 1 — Repository, toolchains, and quality gates

| ID | Status | Depends on | Deliverable and completion evidence |
| --- | --- | --- | --- |
| F-001 | DONE | A-003 | Created the pinned Cargo/pnpm workspaces, application/crate skeletons, editor and ignore policy, and both lockfiles. On 2026-10-09, dependency-free frozen pnpm install and both frontend skeleton builds passed with Node 24.19.0/pnpm 11.25.0; `cargo fmt --all --check`, strict workspace/all-target Clippy, workspace/all-target tests, and workspace/all-target builds passed with Rust/Cargo 1.99.0. |
| F-002 | DONE | F-001 | Installed the reviewed graph from official npm using Node 24.19.0/pnpm 11.25.0 with lifecycle scripts disabled; 224 entries with integrity hashes are resolved in `pnpm-lock.yaml`. Owner approved correcting `@eslint/js` to 10.0.1, replacing CLI2 with `markdownlint-cli` 0.49.1, and scoped overrides for smol-toml 1.9.0, KaTeX 0.18.2, and js-yaml 5.4.1. Every other direct pin is unchanged. Frozen install and audit pass (zero known advisories). All root types, ESLint, Prettier, Markdown, links, unit, and skeleton build commands pass via `cargo xtask check`: 118 Rust, 10 Vitest, and four Node tests; 60 Markdown files. Existing lint rules were preserved; baseline documentation received mechanical fixes only. Parser/renderer, lint coverage, cache exclusion, and scanner-deletion regressions pass. License/optional-native scope is recorded in the dependency inventory. No browser or production package validation is claimed. |
| F-003 | DONE | F-001 | Added the `cargo xtask` alias and commands for aggregate checks, guarded Buf generation, guarded fixture generation, packaging preflight, and explicit native/manual-test acknowledgement plus named-target guards. The aggregate excludes native injection. On 2026-10-09 strict Clippy passed, all 10 argument/guard/error tests passed, and help plus current package preflight executed successfully. Later protocol tasks must add their guarded Buf and fixture inputs. |
| F-004 | DONE | F-001 | Added fail-closed registry/Git/wildcard, license, advisory, duplicate, and lockfile policy in `deny.toml` and `docs/dependency-policy.md`; weekly Cargo/npm/GitHub Actions Dependabot configuration; a bounded redacting worktree secret scanner; and a reviewed dependency inventory. On 2026-10-16 cargo-deny 0.20.2 was built through official Cargo tooling and passed locked advisories/bans/licenses/sources after ten internal path dependencies gained explicit `=0.1.0` versions. The reviewed `syn` 2/3 duplicate warning remains visible; no policy weakening was added. Formatting, strict all-feature/all-target workspace Clippy, all 118 Rust tests, builds, and docs passed on macOS arm64. F-002 now runs the link-checker suite and four Node scanner tests, including tracked deletion handling with unexpected read failures still fatal. F-005 still owns CI and history-aware scanning. |
| F-005 | READY | F-002, F-003 | Create CI matrices for native Rust/frontend builds, formatting/lint/unit tests, protocol generation, browser tests, and packaging smoke builds. Jobs must label actual vs manual coverage. CI must run cargo-deny 0.20.2 policy/advisory checks and a pinned redacting secret scanner over all reachable history appropriate to the checkout. F-002's full local aggregate gate and clean npm audit now pass; CI is not implemented or remotely executed by the F-002 follow-up. Do not represent future generation/browser/package gates or unexecuted jobs as complete. |
| F-006 | DONE | F-001 | Added the dev-only `tether-test-support` crate with a shared atomic monotonic fake clock, explicit-seed deterministic random stream, owner-only auto-cleaning temporary directories, bounded typed trace capture that cannot accept free-form text, and no-op OS lifecycle handles. On 2026-10-09 strict Clippy and all 7 unit tests passed; the crate has no production dependencies and must be used only as a dev-dependency. |
| F-007 | DONE | F-002, F-004 | Resolved GHSA-g3hv-75wg-m249 through four owner-approved exact minimatch 11.0.1 parent overrides. Reviewed source/licenses/new matcher transitive; five compatibility regressions, frozen scripts-disabled installation, zero-finding full npm audit and repository aggregate pass on macOS arm64. Direct pins/prior overrides unchanged; no global override or ignored advisory. R-022 closed for the dated result; no native Windows/CI/Rust advisory refresh is inferred. |
| WB-001 | BLOCKED | F-001, F-004; owner SDK/tool consent | cargo-xwin 0.23.1 was built with locked official Cargo. LLVM 23.1.5's official archive hash/attestation and clang/lld/RC smoke commands pass on Mac arm64. R-023 blocks SDK acquisition: both official VS 17/18 channel payload hashes/sizes disagree with downloaded manifests; no checksum bypass or xwin manifest-check omission is accepted. Need matching authenticated vendor metadata or a separately available native Windows build environment. SDK/CRT pins, NSIS acquisition, cross-link/PE inspection and application packaging are not complete. Tools/caches stay outside Git; no Windows execution/support claim. |

## Wave 1A — Early mobile-browser network validation

ADR 0002 records owner approval to validate personal use on their physical
iPhone with Safari first. Android Chrome LNA remains an expansion gate, not a
prerequisite to this personal-use path and not an inferred pass.

| ID | Status | Depends on | Deliverable and completion evidence |
| --- | --- | --- | --- |
| BV-001 | DONE | A-004, F-001; owner's physical iPhone/Safari | Implemented isolated `tests/e2e/lan-probe` with nine passing Rust and six browser-state/URL tests; all prior workspace gates passed in the preparation record. Under ADR 0002 the owner reports physical iPhone Safari/iOS 27.0.1 HTTP load, same-origin WS PASS/increasing page-runtime round trips, Disconnect/reconnect, return/reconnect after app switch and Wi-Fi off/on, and failed Connect after Ctrl-C, all passing; no permission prompts observed. Accepted evidence and unavailable prompt-denial/build metadata are explicit in `manual-tests/2026-10-25-iphone-safari-lan.md`. This closes only the early personal-use gate, not full app, packaged permission, input, or Android support. |

## Wave 2 — Protocol contract

| ID | Status | Depends on | Deliverable and completion evidence |
| --- | --- | --- | --- |
| PR-001 | DONE | A-003, F-001 | Added the accepted `docs/components/protocol.md` contract for transport, envelopes, ordering, negotiation, capabilities, epoch/sequence/timing, JavaScript heartbeats, safe errors/close codes, canonical input identities, hard limits, and compatibility. Separate security and native-backend review passes are recorded in `protocol-reviews.md`; they approve schema work without claiming independent, executable, or hardware validation. Local-link validation passed. |
| PR-002 | DONE | PR-001, F-003 | Initial schema, direction-specific envelopes, reserved ranges, ordered actions, negotiation/capabilities and safe error shapes are complete. On 2026-11-14 resolved audit A-01 with explicit atomic-click availability at field 21, retaining reserved 14–20 and positive count semantics. Added the frozen initial JSON descriptor FILE baseline, official per-host Buf 1.72.0 checksums, bounded offline/local gate and five real regressions: deletion fails, addition passes, baseline unchanged, tool failures are fatal. Pinned Node/Buf aggregate passes with 214 Rust, 24 Vitest and four Node tests; strict all-feature Rust checks/build/docs pass. New unrelated npm advisory R-022 is recorded, not suppressed. No generated protocol/domain/runtime implementation is claimed. |
| PR-003 | DONE | PR-002 | Committed as a5a8da0 on 2026-11-16: exact prost family 0.14.6, syn 3.0.7, protobuf-es 2.17.0; generated Rust/TS; offline scratch diff, redacted diagnostics, bounded/symlink/stale/missing/extra-output and LF/EOF checks, Rust/TS round trips. Full worktree aggregate passes with 220 Rust/38 Vitest/four Node tests. An isolated staged snapshot independently passes 124 Rust/29 Vitest/four Node tests, strict all-feature Clippy/build and frozen install, excluding older unstaged Mac/network work. One owner-approved generated CapabilitySnapshot expectation only. No Git permission bypass or push; no PR-004/005 claim. Full-graph R-019 still fails and is not waived. See the 2026-11-16 commit checkpoint. |
| PR-004 | DONE | PR-003, F-006 | Added bounded pure client/server hello/runtime validators, descriptor-derived allocation guard, strict unions/scalars/enums, negotiation and lower budgets, safe errors, immutable whole-batch held projection, capability conversion, sequence/epoch/timestamp/heartbeat checks and bounded edit expansion. All 21 validation and two generator-policy regressions pass, including 20,000 deterministic fuzz inputs through four entry points. Full worktree: 243 Rust tests, strict all-feature/all-target Clippy/build, aggregate checks, docs and protocol Windows-target compilation pass. Isolated candidate: 147 Rust/29 Vitest/four Node tests and aggregate pass. No native input, authentication/session service or LAN admission implemented; R-019 is not waived. See dated checkpoint. |
| PR-005 | IN PROGRESS | PR-003, PR-004 | Implemented 76 shared synthetic goldens with independent Rust/TypeScript encode/decode and admission checks, every family/operation, unknown fields/enums, duplicate faults, optional presence, uint64 and exact size/text/count boundaries. Full local checks pass: 245 Rust/115 Vitest/four Node tests; isolated aggregate: 149 Rust/106 Vitest/four Node. Deliberate scratch corpus drift fails both suites, explicit locked/offline generation restores identical bytes. One explicitly run writer command is ignored during ordinary verification, not skipped safety coverage. CI execution remains an unmet F-005 acceptance gate; not DONE. See `manual-tests/2026-11-16-protocol-fixtures.md`. |

## Wave 3 — Input core and fail-safe session model

| ID | Status | Depends on | Deliverable and completion evidence |
| --- | --- | --- | --- |
| IC-001 | DONE | A-002, PR-001 | Added `docs/components/input-backend.md` defining semantic fixed-point actions, USB HID keys, separate sensitive text/logical/media actions, current capability/status snapshots and permission transitions, closed safe error categories, serialized execution context, dispatcher/backend held-state mirrors, idempotent best-effort release ordering, platform obligations, and mock/fault-test requirements. |
| IC-002 | DONE | IC-001, F-006 | Added bounded fixed-point, button, USB HID key, sensitive text, logical/media, capability/status, safe error, and release domain types plus `InputBackend` and a payload-redacting `MockInputBackend`. The mock supports status changes, deterministic fake-time latency, injected execute/release outcomes, and a held-state mirror. On 2026-10-09 formatting, strict all-target Clippy, all 10 unit tests, and the all-target crate build passed. |
| IC-003 | DONE | IC-002 | Added the dedicated-thread `InputDispatcher` with a bounded logical-action queue, monotonic positions, safe adjacent motion/scroll coalescing and barriers, a separately reserved coalescing release mailbox, admission shutdown/cancellation on saturation and faults, press-ordered held-state ledger, explicit suspend/resume/fault modes, panic and poison handling, idempotent cleanup, and internally consistent neutrality checks. The Gemini IC-003 review findings were resolved with a persistent terminal-fault latch and rejection of atomic clicks on held buttons; regression tests cover the full fault/ordinary-release/resume sequence, repeated and racing releases, shutdown from fault, ledger immutability, valid clicks, and drag behavior. Enhanced the mock with release observability and deterministic release latency. On 2026-10-10 formatting, strict workspace/all-target Clippy, 50 workspace tests including 33 input-core tests, and workspace/all-target builds passed. No native input ran. |
| IC-004 | DONE | IC-002 | Implemented validated fixed-point pointer/scroll normalization with independent sensitivity/direction state, optional bounded linear pointer acceleration, clamping without delayed overload, per-axis fractional accumulation/reset, and integral native pointer/discrete-scroll translation hooks. Deterministic seeded property tests exercise 20,000 pointer pairs and 20,000 alternating-axis scroll values for direction, bounds, and axis isolation. On 2026-10-10 formatting, strict workspace/all-target Clippy, all 58 workspace tests (41 input-core), and workspace/all-target builds passed. No native input or physical-device validation ran. |
| SS-001 | DONE | PR-004, IC-003, A-002 | Added `components/session-safety.md`: distinct actor/connection/wire/dispatcher states, total transition policy, atomic neutral epoch barriers, confirmed versus admission-projected held state, scoped/global exhaustive release-trigger mapping, terminal faults, non-cancelling safety priority, current resource/deadline budgets, reset coordination and explicit SS-002/003/004 model/fault-test gates. Documentation/format/link/privacy checks pass; no actor, authentication, network admission or native/device validation is claimed. |
| SS-002 | IN PROGRESS | SS-001 | Supporting components implemented: non-waiting dispatcher receipts and transactional admission cursor with immutable staging, context/cursor/revision checks, queued held-state projection, barriers and monotonic counters. Eleven cursor tests include a 10,000-step seeded model; all 52 input-core tests pass. The single-owner actor, bounded queue/accounting, reserved external safety delivery, native receipt/grant/transfer coordinator and exhaustive actor model/race tests remain incomplete; not DONE. See `components/session-admission.md` and the 2026-11-19 checkpoint. |
| SS-003 | BLOCKED | SS-002, F-006 | Implement Protobuf application heartbeat/RTT tracking driven by browser JavaScript, never transport Ping/Pong alone, with fake-clock tests for event-loop suspension, delayed messages, an open socket without app liveness, reconnect, timeout, and ownership races. Never restore held state after reconnect. |
| SS-004 | BLOCKED | SS-002, SS-003 | Build a fault-injection suite covering every observable loss path, queue overflow, actor cancellation, backend errors, duplicate releases, panic unwind, shutdown, and transfer. Assert the mock ends neutral every time. |

## Wave 4 — Persistence, pairing, and device management

| ID | Status | Depends on | Deliverable and completion evidence |
| --- | --- | --- | --- |
| ST-001 | DONE | A-002, F-006 | Added `docs/components/storage.md` with exact per-user roots, separated versioned file envelopes and limits, lifetime process locking, atomic same-directory write/sync/replace order, owner-only permission and path-safety policy, deterministic migrations, fail-closed corruption recovery, reset scopes, serialized revisions, and native verification requirements. |
| ST-002 | DONE | ST-001 | Implemented strict schema-v2 `settings.json` persistence and deterministic v1 migration; positive revisions and stale-write conflicts; bounded strict Serde JSON decoding; lifetime standard-library file locking; create-new temporary write/flush/sync/re-read/rename; Unix directory sync, `0700`/`0600` tightening, symlink/opened-file/hard-link defenses; exact abandoned-temp cleanup; corruption quarantine and confirmed recovery; redacted errors; and reload-required indeterminate commits. Fourteen tests cover restart, migration, malformed/oversize/unknown/duplicate/trailing input, future schemas, lock contention, interrupted files, every injected commit failure phase, Unix permissions/path attacks, and redaction. On 2026-10-10 formatting, strict workspace/all-target Clippy, all 72 workspace tests, workspace/all-target build, and no-dependency docs passed on macOS arm64. Non-Unix permission setup fails closed; R-018 blocks Windows persistence/support until explicit DACL/reparse implementation and native validation, and no Linux/Windows native validation is claimed. |
| ST-003 | DONE | ST-002 | Implemented independent 256-bit installation identity and 128-bit hostname seed generation through `getrandom` OS entropy; deterministic entropy is test-only. Added the distinct strict `tether.identity` schema-v1 envelope with a 16 KiB ceiling, stable 48-bit preferred `.local` suffix and bounded conflict names, atomic persistence/revisioned rotation, fail-closed corruption quarantine and restart behavior, explicit reset, paired-device invalidation gates, redacted diagnostics, and settings-equivalent Unix path/permission defenses. F-01 now uses descriptor-based permission tightening for validated open files and revalidates inode/path state; a path-swap regression proves the replacement pathname is not chmodded. Twenty-eight config tests pass, including injected entropy and every identity commit phase. On 2026-10-12 component formatting, strict all-target Clippy, tests, build, and no-dependency docs passed on macOS arm64. R-018 still blocks Windows storage; no Windows/Linux native validation is claimed. |
| PA-001 | READY | ST-002, PR-004, A-002 | Write pairing/device lifecycle contract: fragment QR, token entropy/expiry/single use, approval information, cookie scope/lifetime, hash/compare, origin alias behavior, revoke and revoke-all. |
| PA-002 | BLOCKED | PA-001, F-006 | Implement in-memory pending pairing tokens and constant-time credential verification against stored SHA-256 digests. Inject cryptographic RNG; test expiry, race/reuse, denial, caps, and restart behavior. |
| PA-003 | BLOCKED | PA-002, ST-002 | Implement paired-device persistence, rename, last-seen metadata without input content, immediate revoke/revoke-all notifications, and reset. Test revocation during active control releases state. |
| PA-004 | BLOCKED | PA-002, PA-003 | Implement desktop approval coordinator with bounded pending requests, timeout/deny, source-address display, untrusted client-name labeling, and duplicate request behavior. UI-independent integration tests pass. |

## Wave 5 — LAN network and discovery services

| ID | Status | Depends on | Deliverable and completion evidence |
| --- | --- | --- | --- |
| NW-001 | DONE | A-002, ST-002, BV-001 | Added `components/network-server.md` with current-IANA-reviewed proposed TCP port 54321, explicit collision/persistence policy, exact routes, interface-bound on-link admission, Host/Origin and cookie scope, bounded HTTP/WS budgets, JavaScript liveness, priority cleanup, security headers/cache/redaction, and implementation verification gates. Incorporated actual owner-reported Safari/iOS 27.0.1 physical results without broader claims. Markdown, links, formatting, secret and whitespace checks pass; no production server implemented or native firewall/package behavior claimed. |
| NW-002 | DONE | NW-001 | Cross-platform bounded/redacted inventory/selection, persistence, binding/admission model plus read-only Mac provider completed under ADR 0001. OS service IDs are persistent only when unambiguous; random session IDs retire on loss/failure. Actual masks/flags and successful IPv6-status queries govern eligibility; unknowns fail closed. 27 network tests pass, including unchanged-poll approval preservation and index/failure invalidation, along with strict workspace/cross-target Clippy, workspace tests/builds and docs. Read-only native diagnostic observed one proposed Wi-Fi interface and unchanged consecutive inventories; no packets/listener/input or native network mutation. Windows/Linux providers, continuity evidence, ongoing change coordination and physical IPv6 remain unvalidated/deferred, not support claims. See `manual-tests/2026-10-25-interface-provider.md`. |
| NW-003 | DONE | NW-001, NW-002 | Implemented serialized supervisor, independent non-cancelling safety tickets/two-second deadline, persistent fault latch, fresh pre/post-bind and idle/accept inventory validation, complete-set rollback, generation invalidation, shared 64-global/8-peer RAII transport quotas and forced teardown. Mac-native Tokio/socket2 adapter uses literal nonblocking/v6-only sockets without reuse-port; real tests verify collision, TIME_WAIT restart, native endpoints, peer rejection and rollback. 49 network and 196 workspace tests pass; strict workspace/cross-target Clippy, builds/docs pass. Four tests use actual loopback sockets; one native test checks pre-IO guards and remaining policy/lifecycle/input is inert/mock. No LAN/multicast/input posting, native Windows/Linux, namespace, packaged permission/firewall or physical DHCP tests ran; platform gaps remain under ADR 0001. Production core/session coordination and HTTP routing remain later tasks. See `manual-tests/2026-10-25-listener-supervisor.md`. |
| NW-004 | READY | NW-003 | Implement embedded static-asset handler with hashed immutable assets, no-store entry/pairing responses, MIME correctness, SPA fallback rules, restrictive headers, and no directory/filesystem access. Snapshot header/cache tests pass. No implementation yet; the new discovery decision does not relax any routing/authentication prerequisite. |
| NW-005 | BLOCKED | NW-003, PA-004 | Implement pairing request/approval completion routes, fragment-to-POST flow contract, cookie issuance/clearing, exact Host/Origin validation, no permissive CORS, request limits, and per-peer/global rate limiting. Attack-oriented HTTP tests pass. |
| NW-006 | BLOCKED | NW-003, PR-005, SS-003, PA-003 | Implement authenticated binary WebSocket upgrade with cookie verification, subprotocol/hello negotiation, version/capability errors, quotas, frame limits, and session registration. Test absent/forged Origin, bad Host, malformed protobuf, revoked credentials, unsupported versions, and excessive connections. |
| NW-007 | BLOCKED | NW-006, IC-003 | Implement bounded read/write tasks, client/server backpressure policy, motion/scroll coalescing boundaries, heartbeat integration, close reasons, and emergency safe suspension. Load tests prove bounded memory and neutral input after congestion. |
| DS-001 | DONE | ST-003, NW-002 | Added `components/discovery.md`: ST-003-derived names, bounded PTR/SRV/TXT/no-secret metadata, exact pre-worker interface scope, TTL/goodbyes, conflict/command/event/deadline bounds, generation-safe DHCP/rebind/cleanup ordering, host-scoped re-pair and current IP fallback. Checked current official RFC/API documentation; the mdns-sd pre-worker allowlist/internal-bounds question remains an explicit DS-002 validation gate, not a claimed pass. No daemon or multicast traffic implemented/run. Documentation checks pass. |
| DS-002 | IN PROGRESS | DS-001; accepted ADR 0003 | Rejected mdns-sd 0.21.6 unchanged with source evidence. Owner approved the Mac-native alternative and separately en0/TCP 54322 native testing. Reviewed six-function boundary, exact current scopes, custom A/AAAA, bounded pinned failure-latching slots, readiness quota/deadline and parent-only teardown have inert/SDK tests. Initial Mac registration, exact scoped A/AAAA/SRV/TXT queries, browse add/removal, repeated graceful stop and listener cleanup pass after fixing a regression-tested canonical callback type assumption. See `manual-tests/2026-10-25-bonjour-boundary.md`. Packet containment, multiple same-family RRsets, real conflicts/flaps, permission denial/recovery and physical phone resolution remain open; not DONE and no other-platform support inferred. |
| DS-003 | BLOCKED | DS-002, NW-003 | Implement discovery service and address-change monitor. Coordinate release, listener rebind, re-advertisement, stale-session close, and updated QR/status atomically through `agent-core`. Simulate DHCP and interface flaps. |
| NW-008 | BLOCKED | NW-004, NW-005, NW-006, DS-003 | Run full LAN server integration with mock input: primary `.local`, IP fallback, origin-scoped re-pair, hostname conflict, restart, revoked device, network change, and no-network mode. |

## Wave 6 — Mobile web application

| ID | Status | Depends on | Deliverable and completion evidence |
| --- | --- | --- | --- |
| MW-001 | BLOCKED | F-002, PR-003, NW-004 | Build the mobile UI shell, responsive region geometry, `visualViewport`/safe-area handling, portrait/landscape layouts, edge-navigation gutters for either vertical-strip side, preserved horizontal strip and neutral corner, status states, themes, semantic controls, and local preference schema/migrations. Unit and viewport geometry tests pass. |
| MW-002 | BLOCKED | MW-001, NW-006 | Implement the authenticated client connection state machine: origin-derived URL, hello/capabilities, standby/control request, bounded exponential backoff with jitter, auth-required flow, protocol mismatch, visibility/pagehide best effort, and no held-state replay. Fake-clock tests pass. |
| MW-003 | BLOCKED | MW-001, PR-004 | Write and implement the React-independent gesture state machine with pointer capture and per-ID tracking for tap, double tap, long-press drag, double-tap drag, two/three-finger taps, cancellation, capture loss, region boundaries, and orientation reset. Test both drag methods separately/together and measure tap latency with double-tap drag off/on against the documented interaction budget. |
| MW-004 | BLOCKED | MW-003, IC-004 | Implement the single ordered client dispatch pipeline: animation-frame motion/scroll accumulation, sensitivity/acceleration, flush-before-click/key/button/ownership barriers, ordered batches, queue thresholds, and anomalous-delta clamps. High-rate tests prove bounded sends and exact state-transition order. |
| MW-005 | BLOCKED | MW-001, IC-004 | Implement independent vertical/horizontal strip engines, neutral corner, side switching, natural/traditional direction, per-axis sensitivity, velocity sampling, Off/Low/Medium/High momentum, cancellation, and browser suspension reset. Fake-time physics tests pass. |
| MW-006 | BLOCKED | MW-003, SS-002 | Implement explicit left/right/middle and optional back/forward controls with down/up/atomic click semantics, hold indicators, pointer cancellation, and emergency release UI. Interaction tests end neutral. |
| MW-007 | BLOCKED | MW-001, PR-004, A-002 | Write the mobile text-entry contract and implement a persistent accessible proxy editable that is synchronously focused in the user-gesture stack, plus the local-buffer/live mode for `beforeinput`, `input`, `keydown`, composition, and `visualViewport` changes. Test async-focus rejection paths, keyboard open/close anchoring, autocorrect, dictation-style replacement, deletion, surrogate pairs, grapheme sequences, and no partial composition sends. |
| MW-008 | BLOCKED | MW-007 | Implement Compose and Send with explicit confirmation, message chunking, backend capability/error feedback, long-text bounds, and no clipboard API dependency. Tests cover multiline, accents, emoji, cancellation, and partial backend failure. |
| MW-009 | BLOCKED | MW-001, PR-004 | Implement special keys, momentary/locked modifiers, platform-logical shortcut deck, media capabilities, user-defined labels/key combinations, clear unsupported states, and release on session loss. Mapping and state tests pass. |
| MW-010 | BLOCKED | MW-002–MW-009 | Integrate the complete touch surface without high-frequency React re-renders. Run Chromium/WebKit Playwright suites over small/large phone/tablet portrait/landscape matrices, accessibility scans, reconnect, ordered-input, and protocol mocks. |
| MW-011 | BLOCKED | MW-001, MW-002 | Implement configurable Keep Screen Awake: use native Screen Wake Lock only when exposed in an eligible context, handle rejection/revocation/visibility, and provide only browser-gated, explicit-user-gesture, stoppable muted-video fallback. Tests prove cleanup and honest unsupported state; real behavior remains an IT-005 gate. |
| MW-012 | BLOCKED | MW-010, MW-011 | Run the integrated mobile acceptance suite for region geometry, edge gutters, neutral corner, both drag methods/tap latency, ordered dispatch, keyboard/viewport, heartbeat suspension, keep-awake state, reconnect, and capability reporting. |

## Wave 7 — Desktop shell and orchestration

| ID | Status | Depends on | Deliverable and completion evidence |
| --- | --- | --- | --- |
| AC-001 | BLOCKED | IC-003, SS-004, ST-002, PA-004, NW-008 | Implement `agent-core` ordered startup, service supervision, state snapshots, coordinated rebind, stop-control, and reverse-order shutdown. Fake services verify every partial-start and partial-stop failure path. |
| DT-001 | DONE | A-003, F-001 | Added `docs/components/desktop-shell.md` with the no-LAN administration boundary, bounded revisioned command/event DTOs, least-privilege Tauri capability/CSP rules, startup and single-instance ordering, tray states, close-vs-quit, explicit autostart consent, and fail-safe OS lifecycle mapping. Current official Tauri 2 capability, single-instance, close-event, and autostart behavior was checked on 2026-10-09; no native behavior is claimed. |
| DT-002 | BLOCKED | DT-001, AC-001 | Implement the thin Tauri composition root, least-privilege capabilities, single-instance handling, window close-to-tray, tray/menu state, and safe full quit. Tauri mock/runtime tests pass. |
| DT-003 | BLOCKED | DT-002, NW-002, DS-003 | Implement first-run network confirmation, status/QR/IP fallback, macOS Local Network guidance, server start/stop, interface selection, and auto-listen settings. Denied/no-interface/port-conflict states are actionable. |
| DT-004 | BLOCKED | DT-002, PA-004, SS-002 | Implement pairing approval UI, connected/standby/controller list, rename/revoke/disconnect/revoke-all, atomic ownership transfer, and prominent Stop Control. UI/IPC integration tests verify release. |
| DT-005 | BLOCKED | DT-002, IC-002, DS-003 | Implement permission/capability and safe diagnostics screens. Verify no typed text, keys, tokens, credentials, cookies, raw frames, or clipboard data appear at any log level or export. |
| DT-006 | BLOCKED | DT-002, AC-001 | Implement start-at-login opt-in and platform lifecycle adapters for suspend/resume, session lock/logout, graceful signals, and OS shutdown where observable. Fault tests verify pre-suspend/session release ordering. |
| DT-007 | BLOCKED | DT-003–DT-006 | Run Tauri desktop E2E suites on Windows, macOS, and Linux for onboarding, tray/background mode, stop, device management, settings persistence, permission errors, and clean quit. Record platform-specific automation gaps. |

## Wave 8 — Native input backends

These tasks can proceed in parallel after the `InputBackend` and session safety
contracts are stable. Each backend first proves feasibility without changing the
shared interface, then implements, then passes isolated native validation.

| ID | Status | Depends on | Deliverable and completion evidence |
| --- | --- | --- | --- |
| NB-001 | DONE | IC-001, PR-001 | Added one explicit version-one USB HID Keyboard/Keypad allowlist, exhaustive reviewed Windows/macOS/Linux logical-command chord plans with no fabricated Linux task-manager mapping, redacted chord diagnostics, shared action-capability preflight, and an opt-in inert backend conformance suite. The suite checks more than 400 canonical actions when capabilities are advertised, requires every HID usage to map to balanced down/up or an explicit unsupported result, covers all advertised buttons/logical/media commands and independent axes, rejects unavailable capabilities before backend execution, and verifies held-state plus repeated idempotent release. Forty-nine input-core tests pass; strict all-feature/all-target Clippy, build, and no-dependency docs passed on 2026-10-12 macOS arm64. No native mapper, native injection, or physical-device validation is claimed. |
| WIN-001 | BLOCKED | NB-001; actual Windows environment | Implemented a redacted pure `SendInput` event-plan mapper using `windows` 0.62.2 for relative motion, five buttons, both wheel axes, partial scan/extended-key mappings, media keys, and correct UTF-16 surrogate-unit pairs. General keyboard capability remains advertised while unmappable HID usages return `Unsupported`; inert conformance and partial-failure/release tests pass, and strict Clippy/check cross-compiles for Windows x86-64. No Windows host was available, so actual `SendInput` return behavior, UIPI, OS acceleration, layouts, wheel consumers, DPI/monitors, and neutralization are not validated. Keep blocked pending native spike evidence and independent review; do not treat cross-compilation as support. |
| WIN-002 | BLOCKED | WIN-001, IC-004 | Implement production Windows backend on a serialized execution context with permission/status/capabilities and error mapping. Unit-test structure generation without injecting input. |
| WIN-003 | BLOCKED | WIN-002, DT-006 | Run guarded native tests on supported Windows 11 and serviced Windows 10 targets: layouts, Unicode/surrogate-pair apps, horizontal scroll, pointer-speed/acceleration settings, mixed-DPI multi-monitor boundaries, normal/elevated targets, session lock, firewall, installer and autostart. Record evidence. |
| MAC-001 | DONE | NB-001 | Selected maintained generated `objc2-core-graphics` 0.3.2 bindings rather than local FFI. Added redacted inert plans for relative motion, five buttons/click state, continuous scroll, partial keycode maps, UTF-16 text, Command chords, and auxiliary media keys; honest unsupported HID behavior; release/error tests; and the shared conformance suite. On 2026-10-16 it compiled on macOS arm64, executed the non-prompting Accessibility preflight, and passed formatting, strict Clippy, tests, build, and docs. It did not create/post input, prompt for permission, validate target applications/media behavior, signing identity, Intel macOS, acceleration, or displays; those remain MAC-002/003 and R-007. |
| MAC-002 | IN PROGRESS | MAC-001, IC-004 | Implemented `MacOsBackend`/Quartz posting through `InputBackend`, driver-injected safety tests, full preflight/transition checks, fractional pointer and independent pixel-scroll conversion, drag/left-right modifier state, borrowed logical modifiers, bounded surrogate-safe Unicode, mirrored/in-flight cleanup, permanent fault retention, and session-active hooks. Added explicit user-gesture Accessibility request hook, non-posting native event/UTF-16 inspection tests, private bundle/privacy metadata, and a separately guarded motion-only diagnostic (not run live). Default tests never post input. The non-prompting probe reports `PromptRequired` both inside and outside the sandbox. Media remains unavailable, not falsely advertised. Task remains incomplete pending appropriate media handling, desktop onboarding/Local Network retry and OS notification wiring, and applicable validation; this is not a working phone-to-Mac application. Evidence is in `manual-tests/2026-10-23-macos-backend.md`. |
| MAC-003 | BLOCKED | MAC-002, DT-006 | Run stable-identity signed/notarized native tests on supported Apple Silicon and Intel configurations: Accessibility and Local Network grant/deny/revoke/upgrade persistence, Unicode target apps, Command shortcuts, scroll axes, pointer acceleration, mixed-density display boundaries, sleep/wake, menu-bar mode, retry guidance and autostart. |
| X11-001 | BLOCKED | NB-001, A-002; actual X11 environment | Implemented an `x11rb` 0.14.0 XKB/XTEST probe-snapshot model and redacted inert plans for bounded relative motion, buttons, partial discovered keycodes, layout-mappable text preflight, logical/media keys, and independent fractional Button 4–7 scrolling. Tests cover missing connection/XTEST, unsupported mappings, whole-text rejection, balanced scroll, residue reset, partial failures, held cleanup, and shared conformance; strict Clippy/check cross-compiles for Linux x86-64. No X server was available, so live extension queries, XKB/group changes, application scrolling, connection loss, and native events remain unvalidated. Keep blocked pending target evidence and independent review. |
| X11-002 | BLOCKED | X11-001, IC-004 | Implement production X11 backend with runtime capability reporting, fractional wheel accumulation, mappable-text preflight, and serialized X connection ownership. |
| X11-003 | BLOCKED | X11-002 | Run guarded native tests on named supported X11 distributions/desktops/layouts, including missing XTEST, horizontal scrolling apps, unmappable Unicode, logout/relogin, multiple displays, package install and autostart. |
| WAY-001 | BLOCKED | NB-001, A-002; target GNOME and KDE environments | Pinned and cross-compiled `ashpd` 0.13.13 plus `reis` 0.7.1 and implemented a portal/EI negotiation snapshot with fail-closed v2/session/grant/connection/device lifecycle, independent capabilities, partial evdev key mapping, framed pointer/button/key plans, independent fixed-point-to-v120 scroll translation/reset, explicit scroll stop/cancel and stop-emulating plans, and optional UTF-8 text split to at most 254 bytes at character boundaries with one request per frame. Inert conformance, missing capability, denial/disconnect/pause/removal, unsupported mapping, partial failure, and cleanup tests pass. No target GNOME/KDE portal, restore, handshake, device, compositor restart, or live event ran; R-009 remains open and adapter selection in WAY-002 is not approved. |
| WAY-002 | BLOCKED | WAY-001 | Architecture checkpoint: accept `reis`, or obtain approval and ADR for isolated `libei` FFI. If neither meets requirements, document the compositor gap instead of bypassing Wayland security. |
| WAY-003 | BLOCKED | WAY-002, IC-004 | Implement production Wayland backend with dynamic portal/EI capabilities, correct frame timestamps/order, text chunking at UTF-8 boundaries, restore lifecycle, and release before stop-emulating. |
| WAY-004 | BLOCKED | WAY-003, DT-006 | Run guarded native tests on named GNOME/KDE versions for grant/revoke/restore, missing portal v2/EI, pointer/keys/both scroll axes/text, pause/resume, compositor restart, logout, packaging and autostart. Record each environment separately. |

## Wave 9 — Cross-component hardening and performance

| ID | Status | Depends on | Deliverable and completion evidence |
| --- | --- | --- | --- |
| IT-001 | BLOCKED | MW-012, DT-007, WIN-002, MAC-002, X11-002, WAY-003 | Run full mock-backed system tests from pairing QR request through control, standby transfer, revoke, reconnect, app stop, network change, and shutdown. Verify neutral state after every scenario. |
| IT-002 | BLOCKED | IT-001 | Add malformed/fuzz/load suites for HTTP, WebSocket, protobuf, authentication, session actor, gesture/text state machines, storage, and discovery events. Set regression corpus retention without sensitive payloads. |
| IT-003 | BLOCKED | IT-001 | Measure browser sampling, RTT, server queue latency, backend injection duration, CPU, memory, wakeups, and battery-relevant traffic on representative hardware. Tune bounded queues/coalescing/momentum; document before/after data. |
| IT-004 | BLOCKED | IT-002, IT-003 | Security review: cross-site WebSocket attempts, DNS rebinding assumptions, Host/Origin aliases, token/cookie leakage, rate-limit bypass, slowloris/oversize inputs, unsafe diagnostics, Tauri IPC permissions, dependency advisories, and local file permissions. Fix all release blockers. |
| IT-005 | BLOCKED | MW-012, NW-008 | Real mobile validation on current iPhone Safari, iPad Safari, Android Chrome phone and tablet: QR fragment flow, `.local`, IP fallback, host-scope re-pair, OS LAN prompt/denial, edge-navigation gutters on both sides, neutral corner, both drag methods/tap latency, synchronous software-keyboard focus, IME/dictation, `visualViewport`, native/video keep-awake behavior, pointer cancellation, rotation, safe areas, JavaScript suspension heartbeat expiry, reconnect, and bookmark/home-screen convenience. |
| IT-006 | BLOCKED | WIN-003, MAC-003, X11-003, WAY-004, IT-005 | Reconcile capability/support matrix with actual evidence. Any unsupported item is disabled and documented; no compilation-only support claim remains. |

## Wave 10 — Packaging, release, and final acceptance

ADR 0001 defers commercial/public distribution, signing/notarization, and
expanded-platform claims for personal builds. These rows remain uncompleted
public-release gates, not blockers to all independent personal-use development.
Actual installed-app and physical-browser behavior still requires verification.

| ID | Status | Depends on | Deliverable and completion evidence |
| --- | --- | --- | --- |
| PK-001 | BLOCKED | DT-007, WIN-003, IT-004 | Produce signed Windows installer(s), private-network firewall rule behavior, install/upgrade/uninstall/start-menu/autostart tests, WebView2 strategy, data preservation/removal choices, and signature verification. |
| PK-002 | BLOCKED | DT-007, MAC-003, IT-004 | Produce universal or per-architecture signed/notarized `.app` and DMG per accepted support policy. Test fresh install, App Translocation/move to Applications, permission identity persistence, upgrade, uninstall guidance and launch-at-login. |
| PK-003 | BLOCKED | DT-007, X11-003, WAY-004, IT-004 | Produce `.deb` and `.rpm` with exact WebKitGTK/GTK/tray/runtime dependencies and desktop/autostart entries. Test clean VM install/upgrade/remove on every supported distro baseline. |
| PK-004 | BLOCKED | PK-003 | Produce AppImage where practical, document its compatibility/signature limits, and test it as a convenience artifact. It does not replace native package validation. |
| PK-005 | BLOCKED | PK-001–PK-004 | Generate checksums, SBOM, dependency/license report, build provenance, signed metadata, and reproducible-build notes. Validate artifacts on clean machines without developer tools or internet for normal operation. |
| RL-001 | BLOCKED | IT-006, PK-005 | Complete user README, install guides, privacy/security limitations, troubleshooting, supported environments, and release notes. Resolve the provisional product name/trademark before public branding. |
| RL-002 | BLOCKED | RL-001 | Execute the full `SPEC.md` acceptance checklist with links to CI artifacts and manual test records. Any missing hardware/platform evidence remains a blocker or is removed from the support claim with owner approval. |
| RL-003 | BLOCKED | RL-002 | Independent final architecture, security, native-backend, accessibility, performance, and documentation review. Fix findings and rerun affected gates. |
| RL-004 | BLOCKED | RL-003 | Produce release candidate, perform clean-room install/pair/control/stop/uninstall on each Tier 1 platform, verify offline operation and zero automatic telemetry, then approve production release. |

## Continuous verification command contract

Exact commands will be established in F-002/F-003, but the root workflow must
provide stable equivalents of:

- format check for Rust, TypeScript, styles, protobuf, and Markdown;
- Rust Clippy with warnings denied for workspace/target-appropriate crates;
- Rust unit/integration/doc tests;
- TypeScript type check, lint, unit tests, and production builds;
- Buf lint/breaking/generated-code checks and Rust↔TypeScript fixtures;
- browser E2E for Chromium and WebKit viewports;
- native compile/package matrix on each host OS;
- dependency advisory/license/source checks; and
- a single aggregate CI command that does not run destructive native input
  tests on a developer workstation.

Every task handoff reports the commands actually run, their results, and the
manual/platform work not run. Near-equivalent checks are not described as passed
when the specified command or environment was unavailable.
