# Component documentation plan

**Status:** Accepted. Documents marked “pre-code gate” must exist and be accepted
before implementation of the corresponding component.

## Documentation principles

- `SPEC.md` states product requirements and remains authoritative.
- ADRs record accepted decisions and their consequences; they do not silently
  rewrite the specification.
- Component documents define implementable contracts, invariants, failure modes,
  limits, and verification consistent with the spec and accepted ADRs.
- Code-level API documentation explains how an implementation satisfies its
  component contract. It should link to the contract rather than restating it.
- Manual test records say exactly what hardware, OS/browser version, package,
  steps, and result were observed. “Supported” claims must point to evidence.
- Documentation changes are part of the same task as the behavior they describe.

## Required architecture and decision documents

| Document | Purpose | Gate / synchronization trigger |
|---|---|---|
| `docs/architecture-review.md` | Historical feasibility findings and original approval points | Complete; not a second specification |
| `docs/architecture-decisions.md` | Accepted D-01–D-05, reconciled independent-review findings, rejected alternatives, rationale | Complete; binding interpretation of `SPEC.md` |
| `docs/open-risks.md` | Platform uncertainties, evidence requirements, and release/capability gates | Updated whenever validation changes |
| `docs/adr/NNNN-*.md` | Any later material architecture change | Accepted before merge of the changing code |

Every ADR records status, context, decision, alternatives rejected for concrete
reasons, security/compatibility consequences, and superseding ADR if any.

## Component contracts

### System and cross-cutting

| Proposed document | Required contents | Pre-code gate |
|---|---|---|
| `docs/components/system-overview.md` | Process model, dependency direction, startup/shutdown order, trust boundaries, shared terminology | `agent-core` |
| `docs/components/security-threat-model.md` | Assets, trusted-LAN assumption, attacker classes, HTTP residual risk, origin/host checks, DoS limits, admin boundary, security test cases | pairing/network |
| `docs/components/privacy-and-logging.md` | Prohibited data, redacted types, allowed aggregate metrics, log retention/rotation, diagnostics export | logging/diagnostics |
| `docs/components/configuration-storage.md` | Paths, file permissions, schema versions, atomic commit, locking, corruption handling, reset and migration semantics | `config` |
| `docs/components/error-model.md` | Stable error categories, user-safe messages, sensitive cause redaction, backend capability errors | shared public APIs |
| `docs/components/performance-budget.md` | Queue and connection budgets, input-rate assumptions, memory/CPU targets, measurement method, regression thresholds | network/input tuning |

### Protocol, authentication, and networking

| Proposed document | Required contents | Pre-code gate |
|---|---|---|
| `docs/components/protocol.md` | Envelope and ordered action batches, version/subprotocol negotiation, JavaScript heartbeats, message semantics, HID IDs, enums, limits, sequencing, timestamps, compatibility and field reservation rules | `.proto` schema |
| `docs/components/pairing.md` | QR fragment format, token state machine, approval/denial/expiry, credential hashing/cookie attributes, origin scoping, rename/revoke flows | `pairing` |
| `docs/components/session-safety.md` | Auth states, standby/controller ownership, held-state ledger, heartbeat, transfer, reconnect, release-all truth table and fault injection | `session` |
| `docs/components/network-server.md` | Routes, methods, listener model, Host/Origin canonicalization, peer admission, headers, timeouts, quotas, rate limits, cache policy, WebSocket backpressure | `network` |
| `docs/components/discovery.md` | Installation ID/hostname, probing/conflicts, service/TXT schema, selected interfaces, address changes, goodbyes, fallback URLs | `discovery` |

### Input and mobile web

| Proposed document | Required contents | Pre-code gate |
|---|---|---|
| `docs/components/input-backend.md` | `InputBackend` contract, semantic actions, capability/status model, dispatcher ordering, fractional accumulation, release/error guarantees | `input-core` |
| `docs/components/mobile-gesture-engine.md` | Pointer identity model, regions, both drag methods/tap latency, tap/hold/drag/multitouch state chart, cancellation and orientation behavior, thresholds | gesture implementation |
| `docs/components/scroll-engine.md` | Units, independent axes, sensitivity/direction, velocity estimator, anomaly rejection, momentum lifecycle, backend translation | scroll implementation |
| `docs/components/pointer-processing.md` | CSS-pixel deltas, coalescing boundaries, sensitivity/acceleration, OS acceleration interaction, congestion behavior | pointer implementation |
| `docs/components/mobile-text-entry.md` | Hidden edit-buffer model, beforeinput/input/composition handling, live vs compose/send, grapheme/code-point behavior, unmappable operations | keyboard implementation |
| `docs/components/mobile-ui.md` | Geometry, edge-navigation gutters, dedicated strips/neutral corner, visual viewport, safe areas, keep-awake capability/fallback, orientation, accessibility, theme, touch targets, status/offline states, local preferences | UI shell |
| `docs/components/shortcuts.md` | Logical commands, per-OS mappings, unsupported secure actions, user-defined key combinations, no shell execution | shortcut deck |

### Desktop and native platforms

| Proposed document | Required contents | Pre-code gate |
|---|---|---|
| `docs/components/desktop-shell.md` | Tauri IPC allowlist, tray/window/single-instance behavior, onboarding, stop/quit, autostart consent, lifecycle event delivery | Tauri shell |
| `docs/components/windows-backend.md` | HID-to-scan-code mapping, extended keys, wheel units, Unicode packets, UIPI, permission/status and native tests | Windows backend |
| `docs/components/macos-backend.md` | CoreGraphics mapping, Accessibility and Local Network permissions, Unicode/scroll behavior, sleep/session hooks, signing implications | macOS backend |
| `docs/components/x11-backend.md` | XTEST/XKB availability, layout mapping, wheel buttons, Unicode capability, connection loss, X server test matrix | X11 backend |
| `docs/components/wayland-backend.md` | Portal version flow, grant/restore lifecycle, EI handshake/devices/frames, pause/resume, text chunking, GNOME/KDE matrix | Wayland backend |
| `docs/components/diagnostics.md` | Safe fields, capability explanations, troubleshooting mapping, aggregate metrics, export redaction | diagnostics UI |

## Test and operations documents

| Proposed document | Purpose | Update trigger |
|---|---|---|
| `docs/operations/support-matrix.md` | Exact release claims: OS editions/versions, CPU, distro, compositor, browser, package | Every release/toolchain change |
| `docs/operations/testing-strategy.md` | Unit/contract/integration/E2E/native/manual layers, fixtures, environment guards | Test architecture change |
| `docs/operations/ci.md` | Jobs, platform coverage, required checks, generated-code and supply-chain gates | Workflow change |
| `docs/operations/packaging.md` | Install/uninstall, data paths, firewall, autostart, signing/notarization, Linux dependencies | Packaging change |
| `docs/operations/release-process.md` | Versioning, changelog, signing custody, SBOM/provenance, artifact validation, rollback | Release process change |
| `docs/operations/troubleshooting.md` | User-facing network, permission, mDNS, firewall, pairing and backend remediation | New error/capability |
| `docs/operations/security-response.md` | Private reporting, dependency advisory triage, credential-impact assessment, release response | Before public release |
| `docs/manual-tests/template.md` | Required fields and pass/fail/evidence format | Before first manual run |
| `docs/manual-tests/mobile-*.md` | Real iPhone/iPad/Android behavior | Each supported browser release |
| `docs/manual-tests/{windows,macos,x11,wayland}-*.md` | Native input/permission/package evidence | Each supported OS/backend release |

## Documentation ownership by task

The implementation task that creates or changes a component owns its contract
document. Review cannot close the task until:

1. public behavior, invariants, limits, and known gaps match the code;
2. automated test names or suites are linked from the verification section;
3. manual-only claims are listed without being marked passed prematurely;
4. capability and error strings have user-facing troubleshooting entries; and
5. a protocol, security, support, or dependency-direction change has an ADR.

## Required template for component documents

Each component document uses this outline unless a section is genuinely not
applicable:

1. Scope and non-goals.
2. Dependencies and consumers.
3. Public types/interfaces.
4. State model and invariants.
5. Data and control flows.
6. Limits, validation, and resource budgets.
7. Failure, cancellation, cleanup, and recovery.
8. Security and privacy considerations.
9. Platform/browser variations and capability reporting.
10. Automated verification.
11. Manual verification.
12. Known limitations and future-compatible extension points.

## Documentation verification

CI should eventually enforce:

- Markdown formatting and internal-link checking;
- no broken links from crate/package READMEs to component contracts;
- generated protocol documentation is current;
- every crate and application package links to its owning component document;
- the support matrix contains no release claim without a current manual or CI
  evidence entry; and
- prohibited sample data such as real credentials, pairing URLs, typed text, or
  clipboard contents never appears in fixtures or docs.
