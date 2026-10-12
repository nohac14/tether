# Dependency-ordered implementation plan

**Plan status:** Architecture accepted. A-004, F-001, F-003, F-006, PR-001,
IC-001, IC-002, IC-003, IC-004, ST-001, ST-002, DT-001, and F-004 are
complete. ST-003 and NB-001 are also complete.
F-002 is configured but its exact pinned npm graph is not installed or resolved
in `pnpm-lock.yaml`; separate dependency authorization and a successful install
are still required. Other independent foundation and component work may continue.
The platform-neutral input core is implemented through its dispatcher; native
backends, sessions, networking, and user interfaces are not yet implemented.
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
|---|---|---|---|
| A-001 | DONE | Owner | D-01–D-05 were explicitly approved on 2026-10-09 and recorded in `architecture-decisions.md`. |
| A-002 | DONE | A-001 | Reconciled both reviews into `SPEC.md`, including accepted/corrected/rejected findings, with rationale in `architecture-decisions.md`. Documentation-only validation passed. |
| A-003 | DONE | A-002 | Finalized the system/trust boundaries, threat and support policy, repository/documentation structure, test strategy, and risk gates in the authoritative and supporting documents. |
| A-004 | DONE | A-003 | Recorded the actually observed local toolchain/platform state and every required but unavailable signed identity, physical device, clean OS, browser, Linux desktop/portal, and network topology in `manual-tests/environment-inventory.md`. No unavailable environment is counted as validation or a support claim. |

## Wave 1 — Repository, toolchains, and quality gates

| ID | Status | Depends on | Deliverable and completion evidence |
|---|---|---|---|
| F-001 | DONE | A-003 | Created the pinned Cargo/pnpm workspaces, application/crate skeletons, editor and ignore policy, and both lockfiles. On 2026-10-09, dependency-free frozen pnpm install and both frontend skeleton builds passed with Node 24.19.0/pnpm 11.25.0; `cargo fmt --all --check`, strict workspace/all-target Clippy, workspace/all-target tests, and workspace/all-target builds passed with Rust/Cargo 1.99.0. |
| F-002 | BLOCKED | F-001; permission to download pinned npm development dependencies | Exact compatible versions and root commands are configured for strict TypeScript, ESLint, Prettier, Vitest, Markdown lint, and local-link checks; the link checker has unit tests. A prior `pnpm install` attempt was blocked, the Rust dependency authorization did not authorize npm downloads, and `pnpm list --depth 0` reports no installed root tools even though a `node_modules` directory exists. Completion requires a separately authorized successful install, an updated `pnpm-lock.yaml`, and every root F-002 command passing; do not claim the uninstalled checks. |
| F-003 | DONE | F-001 | Added the `cargo xtask` alias and commands for aggregate checks, guarded Buf generation, guarded fixture generation, packaging preflight, and explicit native/manual-test acknowledgement plus named-target guards. The aggregate excludes native injection. On 2026-10-09 strict Clippy passed, all 10 argument/guard/error tests passed, and help plus current package preflight executed successfully. Later protocol tasks must add their guarded Buf and fixture inputs. |
| F-004 | DONE | F-001 | Added fail-closed registry/Git/wildcard, license, advisory, duplicate, and lockfile policy in `deny.toml` and `docs/dependency-policy.md`; weekly Cargo/npm/GitHub Actions Dependabot configuration; a bounded redacting repository secret scanner with three unit tests; and a reviewed inventory of every resolved Rust dependency. Official crates.io sources and checksums were confirmed with locked Cargo metadata, TOML/YAML/JSON configuration parsing passed, the scanner inspected 119 text files with no finding, and local links in 48 Markdown files passed. On 2026-10-10 formatting, strict workspace/all-target Clippy, all 72 workspace tests, workspace/all-target builds, and no-dependency docs passed on macOS arm64. `cargo-deny` is not installed, so no network advisory lookup is claimed; F-005 must execute the committed policy before release. F-002's Vitest-based link-checker unit test remains blocked by its unresolved npm dependencies. |
| F-005 | BLOCKED | F-002, F-003 | Create CI matrices for native Rust/frontend builds, formatting/lint/unit tests, protocol generation, browser tests, and packaging smoke builds. Jobs must label actual vs manual coverage. CI must run pinned cargo-deny policy/advisory checks and a pinned redacting secret scanner over all reachable history appropriate to the checkout. F-003 is done, but F-002 remains incomplete because the pinned npm graph is not resolved in `pnpm-lock.yaml`; do not create or claim a partial CI gate. |
| F-006 | DONE | F-001 | Added the dev-only `tether-test-support` crate with a shared atomic monotonic fake clock, explicit-seed deterministic random stream, owner-only auto-cleaning temporary directories, bounded typed trace capture that cannot accept free-form text, and no-op OS lifecycle handles. On 2026-10-09 strict Clippy and all 7 unit tests passed; the crate has no production dependencies and must be used only as a dev-dependency. |

## Wave 1A — Early mobile-browser network validation

| ID | Status | Depends on | Deliverable and completion evidence |
|---|---|---|---|
| BV-001 | BLOCKED | A-004, F-001; current physical Android device | Build a test-only same-origin Axum HTTP/WebSocket probe using the intended server stack and a browser page that derives `ws:`/`wss:`, host, and active port from the page runtime origin. On a current physical Android phone with current Chrome, record the exact device/Android/Chrome/network versions and verify direct LAN navigation, any Local Network Access prompt and allow/deny behavior, same-origin `ws://` upgrade, active non-default port preservation, reconnect, and browser console/network errors. Keep the probe under `tests/e2e`; it must expose no input injection or credential path and must not ship. A simulator or desktop Chrome is not a substitute. Update R-012 and the browser support matrix from the observed result. |

## Wave 2 — Protocol contract

| ID | Status | Depends on | Deliverable and completion evidence |
|---|---|---|---|
| PR-001 | DONE | A-003, F-001 | Added the accepted `docs/components/protocol.md` contract for transport, envelopes, ordering, negotiation, capabilities, epoch/sequence/timing, JavaScript heartbeats, safe errors/close codes, canonical input identities, hard limits, and compatibility. Separate security and native-backend review passes are recorded in `protocol-reviews.md`; they approve schema work without claiming independent, executable, or hardware validation. Local-link validation passed. |
| PR-002 | BLOCKED | PR-001, F-003; Buf CLI 1.72.0 unavailable | Added `proto/buf.yaml` and the authoritative `tether.control.v1` schema with direction-specific envelopes, reserved ranges, ordered `oneof` operations, explicit non-zero enum semantics, separate physical keys/text/logical/media actions, negotiation, capabilities, control state, heartbeats, and safe errors. `protoc` 29.3 successfully compiled a descriptor on 2026-10-09. Completion still requires the pinned Buf format, `STANDARD` lint, and initial `FILE` breaking baseline; no Buf executable is installed. Before completion, resolve the audit-confirmed atomic-click capability mismatch by adding an explicit availability field safely or defining and testing zero maximum count as unavailable. |
| PR-003 | BLOCKED | PR-002 | Pin reproducible Rust/TypeScript generation and commit generated sources. `xtask check-generated` must regenerate to a temporary tree and produce no diff offline from checked-in tools/cache. |
| PR-004 | BLOCKED | PR-003, F-006 | Implement wire-to-domain validation: enum checks, numeric bounds, UTF-8/name/text sizes, capability checks, session/version checks, and safe error mapping. Fuzz/proptest malformed envelopes. |
| PR-005 | BLOCKED | PR-003, PR-004 | Create Rust↔TypeScript golden fixtures for every message family, unknown enum values, unknown fields, maximum sizes, and incompatible versions. Run in both toolchains in CI. |

## Wave 3 — Input core and fail-safe session model

| ID | Status | Depends on | Deliverable and completion evidence |
|---|---|---|---|
| IC-001 | DONE | A-002, PR-001 | Added `docs/components/input-backend.md` defining semantic fixed-point actions, USB HID keys, separate sensitive text/logical/media actions, current capability/status snapshots and permission transitions, closed safe error categories, serialized execution context, dispatcher/backend held-state mirrors, idempotent best-effort release ordering, platform obligations, and mock/fault-test requirements. |
| IC-002 | DONE | IC-001, F-006 | Added bounded fixed-point, button, USB HID key, sensitive text, logical/media, capability/status, safe error, and release domain types plus `InputBackend` and a payload-redacting `MockInputBackend`. The mock supports status changes, deterministic fake-time latency, injected execute/release outcomes, and a held-state mirror. On 2026-10-09 formatting, strict all-target Clippy, all 10 unit tests, and the all-target crate build passed. |
| IC-003 | DONE | IC-002 | Added the dedicated-thread `InputDispatcher` with a bounded logical-action queue, monotonic positions, safe adjacent motion/scroll coalescing and barriers, a separately reserved coalescing release mailbox, admission shutdown/cancellation on saturation and faults, press-ordered held-state ledger, explicit suspend/resume/fault modes, panic and poison handling, idempotent cleanup, and internally consistent neutrality checks. The Gemini IC-003 review findings were resolved with a persistent terminal-fault latch and rejection of atomic clicks on held buttons; regression tests cover the full fault/ordinary-release/resume sequence, repeated and racing releases, shutdown from fault, ledger immutability, valid clicks, and drag behavior. Enhanced the mock with release observability and deterministic release latency. On 2026-10-10 formatting, strict workspace/all-target Clippy, 50 workspace tests including 33 input-core tests, and workspace/all-target builds passed. No native input ran. |
| IC-004 | DONE | IC-002 | Implemented validated fixed-point pointer/scroll normalization with independent sensitivity/direction state, optional bounded linear pointer acceleration, clamping without delayed overload, per-axis fractional accumulation/reset, and integral native pointer/discrete-scroll translation hooks. Deterministic seeded property tests exercise 20,000 pointer pairs and 20,000 alternating-axis scroll values for direction, bounds, and axis isolation. On 2026-10-10 formatting, strict workspace/all-target Clippy, all 58 workspace tests (41 input-core), and workspace/all-target builds passed. No native input or physical-device validation ran. |
| SS-001 | BLOCKED | PR-004, IC-003, A-002 | Write the session-safety state model and exhaustive release-all trigger table. Define unauthenticated, standby, requesting, controlling, suspended, revoking, and closed transitions. |
| SS-002 | BLOCKED | SS-001 | Implement the single-owner session actor, per-session epoch/sequence checks, held key/button ledger, explicit atomic ownership transfer, standby clients, and capability gating. Model-test transition invariants. |
| SS-003 | BLOCKED | SS-002, F-006 | Implement Protobuf application heartbeat/RTT tracking driven by browser JavaScript, never transport Ping/Pong alone, with fake-clock tests for event-loop suspension, delayed messages, an open socket without app liveness, reconnect, timeout, and ownership races. Never restore held state after reconnect. |
| SS-004 | BLOCKED | SS-002, SS-003 | Build a fault-injection suite covering every observable loss path, queue overflow, actor cancellation, backend errors, duplicate releases, panic unwind, shutdown, and transfer. Assert the mock ends neutral every time. |

## Wave 4 — Persistence, pairing, and device management

| ID | Status | Depends on | Deliverable and completion evidence |
|---|---|---|---|
| ST-001 | DONE | A-002, F-006 | Added `docs/components/storage.md` with exact per-user roots, separated versioned file envelopes and limits, lifetime process locking, atomic same-directory write/sync/replace order, owner-only permission and path-safety policy, deterministic migrations, fail-closed corruption recovery, reset scopes, serialized revisions, and native verification requirements. |
| ST-002 | DONE | ST-001 | Implemented strict schema-v2 `settings.json` persistence and deterministic v1 migration; positive revisions and stale-write conflicts; bounded strict Serde JSON decoding; lifetime standard-library file locking; create-new temporary write/flush/sync/re-read/rename; Unix directory sync, `0700`/`0600` tightening, symlink/opened-file/hard-link defenses; exact abandoned-temp cleanup; corruption quarantine and confirmed recovery; redacted errors; and reload-required indeterminate commits. Fourteen tests cover restart, migration, malformed/oversize/unknown/duplicate/trailing input, future schemas, lock contention, interrupted files, every injected commit failure phase, Unix permissions/path attacks, and redaction. On 2026-10-10 formatting, strict workspace/all-target Clippy, all 72 workspace tests, workspace/all-target build, and no-dependency docs passed on macOS arm64. Non-Unix permission setup fails closed; R-018 blocks Windows persistence/support until explicit DACL/reparse implementation and native validation, and no Linux/Windows native validation is claimed. |
| ST-003 | DONE | ST-002 | Implemented independent 256-bit installation identity and 128-bit hostname seed generation through `getrandom` OS entropy; deterministic entropy is test-only. Added the distinct strict `tether.identity` schema-v1 envelope with a 16 KiB ceiling, stable 48-bit preferred `.local` suffix and bounded conflict names, atomic persistence/revisioned rotation, fail-closed corruption quarantine and restart behavior, explicit reset, paired-device invalidation gates, redacted diagnostics, and settings-equivalent Unix path/permission defenses. F-01 now uses descriptor-based permission tightening for validated open files and revalidates inode/path state; a path-swap regression proves the replacement pathname is not chmodded. Twenty-eight config tests pass, including injected entropy and every identity commit phase. On 2026-10-12 component formatting, strict all-target Clippy, tests, build, and no-dependency docs passed on macOS arm64. R-018 still blocks Windows storage; no Windows/Linux native validation is claimed. |
| PA-001 | BLOCKED | ST-002, PR-004, A-002 | Write pairing/device lifecycle contract: fragment QR, token entropy/expiry/single use, approval information, cookie scope/lifetime, hash/compare, origin alias behavior, revoke and revoke-all. |
| PA-002 | BLOCKED | PA-001, F-006 | Implement in-memory pending pairing tokens and constant-time credential verification against stored SHA-256 digests. Inject cryptographic RNG; test expiry, race/reuse, denial, caps, and restart behavior. |
| PA-003 | BLOCKED | PA-002, ST-002 | Implement paired-device persistence, rename, last-seen metadata without input content, immediate revoke/revoke-all notifications, and reset. Test revocation during active control releases state. |
| PA-004 | BLOCKED | PA-002, PA-003 | Implement desktop approval coordinator with bounded pending requests, timeout/deny, source-address display, untrusted client-name labeling, and duplicate request behavior. UI-independent integration tests pass. |

## Wave 5 — LAN network and discovery services

| ID | Status | Depends on | Deliverable and completion evidence |
|---|---|---|---|
| NW-001 | BLOCKED | A-002, ST-002, BV-001 | Write network-server contract with default-port selection policy, exact routes, selected-interface listeners, on-link peer admission, IPv4/IPv6 canonicalization, Host/Origin allowlists, quotas, deadlines, body/frame limits, headers, and cache behavior. Incorporate the recorded Android Chrome LNA and same-origin LAN WebSocket result without generalizing beyond the tested versions. |
| NW-002 | BLOCKED | NW-001 | Implement cross-platform interface inventory model with stable identifiers where available, type/flags/prefix/address data, conservative default exclusions, and user selection persistence. Test fixtures for Wi-Fi, Ethernet, VPN, VM, container, loopback, multiple prefixes, and IPv6. |
| NW-003 | BLOCKED | NW-001, NW-002 | Implement listener supervisor for one or more selected local addresses, port collision failure, peer-prefix admission, stop/rebind, and graceful drain. Network-namespace/socket integration tests cover unexpected peers and address loss. |
| NW-004 | BLOCKED | NW-003 | Implement embedded static-asset handler with hashed immutable assets, no-store entry/pairing responses, MIME correctness, SPA fallback rules, restrictive headers, and no directory/filesystem access. Snapshot header/cache tests pass. |
| NW-005 | BLOCKED | NW-003, PA-004 | Implement pairing request/approval completion routes, fragment-to-POST flow contract, cookie issuance/clearing, exact Host/Origin validation, no permissive CORS, request limits, and per-peer/global rate limiting. Attack-oriented HTTP tests pass. |
| NW-006 | BLOCKED | NW-003, PR-005, SS-003, PA-003 | Implement authenticated binary WebSocket upgrade with cookie verification, subprotocol/hello negotiation, version/capability errors, quotas, frame limits, and session registration. Test absent/forged Origin, bad Host, malformed protobuf, revoked credentials, unsupported versions, and excessive connections. |
| NW-007 | BLOCKED | NW-006, IC-003 | Implement bounded read/write tasks, client/server backpressure policy, motion/scroll coalescing boundaries, heartbeat integration, close reasons, and emergency safe suspension. Load tests prove bounded memory and neutral input after congestion. |
| DS-001 | BLOCKED | ST-003, NW-002 | Write discovery contract for host/service names, TXT keys, no-secret policy, interface scope, conflict events, TTL/goodbyes, IP fallback selection, and DHCP/address-change ordering. |
| DS-002 | BLOCKED | DS-001 | Validate the current mDNS dependency with selected-interface, A/AAAA, conflict probing/rename, re-register, goodbye, and permission-denied tests on all OS families. Replace it only through an ADR if a concrete gap appears. |
| DS-003 | BLOCKED | DS-002, NW-003 | Implement discovery service and address-change monitor. Coordinate release, listener rebind, re-advertisement, stale-session close, and updated QR/status atomically through `agent-core`. Simulate DHCP and interface flaps. |
| NW-008 | BLOCKED | NW-004, NW-005, NW-006, DS-003 | Run full LAN server integration with mock input: primary `.local`, IP fallback, origin-scoped re-pair, hostname conflict, restart, revoked device, network change, and no-network mode. |

## Wave 6 — Mobile web application

| ID | Status | Depends on | Deliverable and completion evidence |
|---|---|---|---|
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
|---|---|---|---|
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
|---|---|---|---|
| NB-001 | DONE | IC-001, PR-001 | Added one explicit version-one USB HID Keyboard/Keypad allowlist, exhaustive reviewed Windows/macOS/Linux logical-command chord plans with no fabricated Linux task-manager mapping, redacted chord diagnostics, shared action-capability preflight, and an opt-in inert backend conformance suite. The suite checks more than 400 canonical actions when capabilities are advertised, requires every HID usage to map to balanced down/up or an explicit unsupported result, covers all advertised buttons/logical/media commands and independent axes, rejects unavailable capabilities before backend execution, and verifies held-state plus repeated idempotent release. Forty-nine input-core tests pass; strict all-feature/all-target Clippy, build, and no-dependency docs passed on 2026-10-12 macOS arm64. No native mapper, native injection, or physical-device validation is claimed. |
| WIN-001 | READY | NB-001 | Build a Windows `SendInput` spike for relative motion with OS acceleration, all buttons, both wheels, scan/extended keys, Unicode surrogate pairs, media keys, UIPI failure, and neutralization. Document observed limits. |
| WIN-002 | BLOCKED | WIN-001, IC-004 | Implement production Windows backend on a serialized execution context with permission/status/capabilities and error mapping. Unit-test structure generation without injecting input. |
| WIN-003 | BLOCKED | WIN-002, DT-006 | Run guarded native tests on supported Windows 11 and serviced Windows 10 targets: layouts, Unicode/surrogate-pair apps, horizontal scroll, pointer-speed/acceleration settings, mixed-DPI multi-monitor boundaries, normal/elevated targets, session lock, firewall, installer and autostart. Record evidence. |
| MAC-001 | READY | NB-001 | Spike maintained Rust bindings vs isolated CoreGraphics FFI for relative motion, buttons, pixel/line scroll, keycodes, Unicode, media keys, event-post preflight/request, and release. Select by documented capability, not preference. |
| MAC-002 | BLOCKED | MAC-001, IC-004 | Implement production macOS backend, Accessibility preflight/onboarding hooks without synthetic canary input, Local Network metadata/diagnostics/retry behavior, stable bundle/build identity configuration, and sleep/session behavior. Unit-test event construction where possible. |
| MAC-003 | BLOCKED | MAC-002, DT-006 | Run stable-identity signed/notarized native tests on supported Apple Silicon and Intel configurations: Accessibility and Local Network grant/deny/revoke/upgrade persistence, Unicode target apps, Command shortcuts, scroll axes, pointer acceleration, mixed-density display boundaries, sleep/wake, menu-bar mode, retry guidance and autostart. |
| X11-001 | READY | NB-001, A-002 | Spike XTEST/XKB extension detection, relative motion, button 4–7 scroll conventions, layout-aware keycode mapping, modifier/group changes, text capability boundaries, and connection failure. Do not mutate the global keymap. |
| X11-002 | BLOCKED | X11-001, IC-004 | Implement production X11 backend with runtime capability reporting, fractional wheel accumulation, mappable-text preflight, and serialized X connection ownership. |
| X11-003 | BLOCKED | X11-002 | Run guarded native tests on named supported X11 distributions/desktops/layouts, including missing XTEST, horizontal scrolling apps, unmappable Unicode, logout/relogin, multiple displays, package install and autostart. |
| WAY-001 | READY | NB-001, A-002 | Time-box `ashpd` + `reis` compatibility spike on target GNOME and KDE: portal v2, grant/deny, restore-token rotation, ConnectToEIS, handshake, devices, frames, keymap/modifiers, pause/resume/destroy, scroll stop/cancel, optional text, and disconnect. |
| WAY-002 | BLOCKED | WAY-001 | Architecture checkpoint: accept `reis`, or obtain approval and ADR for isolated `libei` FFI. If neither meets requirements, document the compositor gap instead of bypassing Wayland security. |
| WAY-003 | BLOCKED | WAY-002, IC-004 | Implement production Wayland backend with dynamic portal/EI capabilities, correct frame timestamps/order, text chunking at UTF-8 boundaries, restore lifecycle, and release before stop-emulating. |
| WAY-004 | BLOCKED | WAY-003, DT-006 | Run guarded native tests on named GNOME/KDE versions for grant/revoke/restore, missing portal v2/EI, pointer/keys/both scroll axes/text, pause/resume, compositor restart, logout, packaging and autostart. Record each environment separately. |

## Wave 9 — Cross-component hardening and performance

| ID | Status | Depends on | Deliverable and completion evidence |
|---|---|---|---|
| IT-001 | BLOCKED | MW-012, DT-007, WIN-002, MAC-002, X11-002, WAY-003 | Run full mock-backed system tests from pairing QR request through control, standby transfer, revoke, reconnect, app stop, network change, and shutdown. Verify neutral state after every scenario. |
| IT-002 | BLOCKED | IT-001 | Add malformed/fuzz/load suites for HTTP, WebSocket, protobuf, authentication, session actor, gesture/text state machines, storage, and discovery events. Set regression corpus retention without sensitive payloads. |
| IT-003 | BLOCKED | IT-001 | Measure browser sampling, RTT, server queue latency, backend injection duration, CPU, memory, wakeups, and battery-relevant traffic on representative hardware. Tune bounded queues/coalescing/momentum; document before/after data. |
| IT-004 | BLOCKED | IT-002, IT-003 | Security review: cross-site WebSocket attempts, DNS rebinding assumptions, Host/Origin aliases, token/cookie leakage, rate-limit bypass, slowloris/oversize inputs, unsafe diagnostics, Tauri IPC permissions, dependency advisories, and local file permissions. Fix all release blockers. |
| IT-005 | BLOCKED | MW-012, NW-008 | Real mobile validation on current iPhone Safari, iPad Safari, Android Chrome phone and tablet: QR fragment flow, `.local`, IP fallback, host-scope re-pair, OS LAN prompt/denial, edge-navigation gutters on both sides, neutral corner, both drag methods/tap latency, synchronous software-keyboard focus, IME/dictation, `visualViewport`, native/video keep-awake behavior, pointer cancellation, rotation, safe areas, JavaScript suspension heartbeat expiry, reconnect, and bookmark/home-screen convenience. |
| IT-006 | BLOCKED | WIN-003, MAC-003, X11-003, WAY-004, IT-005 | Reconcile capability/support matrix with actual evidence. Any unsupported item is disabled and documented; no compilation-only support claim remains. |

## Wave 10 — Packaging, release, and final acceptance

| ID | Status | Depends on | Deliverable and completion evidence |
|---|---|---|---|
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
