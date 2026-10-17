# Tether architecture decisions

**Status:** Accepted  
**Decision date:** 2026-10-09  
**Authority:** These decisions interpret and support `SPEC.md`. If this record
and `SPEC.md` differ, `SPEC.md` controls.

## Decision context

The original specification and two independent reviews were reconciled before
application implementation. The product architecture remains a desktop-hosted,
LAN-only React/TypeScript mobile web client communicating over same-origin HTTP
and binary WebSocket with a Rust/Tauri/Axum desktop agent and platform-native
input backends.

The review did not authorize a cloud service, native mobile application,
privileged desktop helper, different transport, or disposable MVP path.

## Accepted primary decisions

### D-01 — Pairing is scoped to the exact browser host

**Decision:** Retain a host-only, `HttpOnly`, `SameSite=Strict` persistent
credential cookie. Omit `Domain`, scope it to `/control`, and keep the credential
unavailable to JavaScript. Treat every hostname or IP address as a distinct
pairing scope. Prefer the stable `.local` hostname; changing to an IP address,
changing IPs, or an mDNS conflict rename may require re-pairing.

**Rationale:** A host-only cookie is returned only to the host that set it. A
credential issued by `tether-xxxx.local` cannot transparently authenticate
`192.168.x.x`, and exposing it to JavaScript would weaken the intended security
boundary. [RFC 6265](https://www.rfc-editor.org/rfc/rfc6265)

**Consequences:** The mobile UI explains host-scope changes. Browser credential
purging also recovers through pairing. The IP QR remains a usable fallback but
does not promise credential portability.

### D-02 — Support claims require maintained systems and real validation

**Decision:** Windows 11 x86-64, vendor-maintained macOS on Apple Silicon, and
named validated Linux x86-64 configurations are the initial primary targets.
Windows 10 is supported only on still-serviced LTSC, IoT, or ESU configurations;
unsupported editions are best-effort. Intel macOS and Linux arm64 become or
remain release targets only while maintained and validated on native hardware.
Mobile support claims name the actual Safari/iOS/iPadOS and Chrome/Android
versions tested.

**Rationale:** General Windows 10 support ended on 2025-10-14, and compilation
does not establish native input, permission, packaging, or browser behavior.
[Microsoft Windows 10 lifecycle](https://learn.microsoft.com/en-us/lifecycle/announcements/windows-10-end-of-support)

**Consequences:** Release documentation includes an evidence-backed support
matrix. Unavailable hardware or configurations are release gaps, not inferred
support.

### D-03 — LAN exposure requires prior interface approval

**Decision:** Before the first non-loopback listener starts, the desktop user
must approve the proposed LAN interfaces and trusted-private-LAN warning.
Automatic listening on later launches is separately opt-in. Bind only approved
interface addresses and, by default, admit only peers on-link under their
current prefixes. Routed-LAN exposure requires a future explicit design.

**Rationale:** Interface type and private-looking addresses are advisory, not a
complete boundary. VPN, tunnel, virtual, public-profile, and globally scoped
IPv6 interfaces can otherwise cause unintended exposure.

**Consequences:** Interface/address loss triggers input release, session close,
listener reconciliation, authority-list rebuilding, and mDNS re-advertisement.
Firewall rules remain defense in depth.

### D-04 — X11 text support is capability-limited

**Decision:** Guarantee X11 physical keys and text representable by the active
XKB layout. Arbitrary Unicode is capability-dependent. Do not silently mutate
the global keyboard map, `CLIPBOARD`, or `PRIMARY` selection. Unsupported text
produces an explicit capability/error result.

**Rationale:** XTEST injects keycodes and button/motion events, not arbitrary
UTF-8 strings. Global keymap or clipboard mutation would be disruptive and
privacy-sensitive. [XTEST specification](https://www.x.org/releases/current/doc/xextproto/xtest.pdf)

**Consequences:** Compose and Send remains valuable but does not promise every
character on X11. Current layout/state tracking is part of the backend contract.

### D-05 — Release guarantees cover paths the process can observe

**Decision:** Centralized, idempotent release is mandatory for every observable
connection-loss and graceful shutdown path. Add best-effort drop/panic cleanup
and startup neutralization. Explicitly exclude uncatchable process termination,
kernel/OS failure, sudden power loss, and a native backend unable to post release
events from an absolute guarantee.

**Rationale:** No in-process cleanup can execute after `SIGKILL`,
`TerminateProcess`, power loss, or equivalent machine failure. Adding a
privileged watchdog would not solve all such cases and would change the product
security architecture.

**Consequences:** Fault tests enumerate all observable paths and assert neutral
mock state. Documentation is honest about the residual OS limit.

## Accepted refinements from the independent review

| Finding | Accepted requirement | Specification location |
| --- | --- | --- |
| Screen awake | Configurable feature; native Screen Wake Lock only in an eligible secure context; any video fallback is explicit, battery-aware, browser-tested, and best-effort. | §§6.6, 18.6, 41–42 |
| Edge gestures | Inset the vertical scroll target from physical viewport edges using safe-area data and tested interior gutter placement. | §§6.2–6.3, 33–34 |
| Ordered input | All input uses one ordered client pipeline and serialized server dispatcher; flush pending motion/scroll before state changes. | §§11.2–11.3, 15, 21 |
| Mobile keyboard | Focus the proxy editable synchronously during the user's gesture; react to `visualViewport` changes and keep a local edit buffer. | §14 |
| Heartbeats | Use Protocol Buffers application heartbeats emitted by page JavaScript; transport activity is not page liveness. | §16.3 |
| Windows Unicode | Encode supplementary characters as correctly ordered UTF-16 surrogate code units for `KEYEVENTF_UNICODE`. | §§22, 26 |
| macOS permissions | Preflight Accessibility, handle Local Network privacy independently, and validate stable signed identity. | §23 |
| X11 scroll | Accumulate fractions per axis and emit balanced discrete XTEST wheel-button events. | §24 |
| Native behavior | Test OS acceleration, pointer settings, mixed DPI/density, multiple displays, and boundary crossing. | §§22–24, 33, 35, 41 |
| Resource abuse | Bound messages, bodies, connections, approvals, queues, and rates; malformed or flooded sessions fail closed through priority cleanup. | §§11, 15, 20–21 |
| Drag methods | Preserve long-press and double-tap drag, make behavior configurable, and measure tap latency. | §§7.2, 33.3 |

The dedicated vertical strip, dedicated bottom horizontal strip, and inactive
neutral corner remain product requirements.

## Corrected or rejected proposals

### C-01 — No ad hoc WebSocket payload encryption

The proposed ChaCha20-Poly1305 envelope was not adopted. Payload sealing does
not authenticate JavaScript delivered over HTTP; an active LAN attacker who
replaces the page can steal pairing data or plaintext before encryption. The
baseline retains the explicit trusted-private-LAN threat model. Any future
confidentiality design must authenticate both transport and web delivery and
requires a new architectural decision.

### C-02 — Wayland Unicode is capability-aware, not categorically absent

The claim that EI lacks Unicode text is false for current EI. The optional
`ei_text.utf8` interface supports UTF-8 where the compositor advertises the text
capability, subject to strict payload and frame-order limits. Tether uses it when
advertised and validated; otherwise it reports layout-mappable key support or an
unsupported result. [EI text interface](https://libinput.pages.freedesktop.org/libei/interfaces/ei_text/)

### C-03 — Android `.local` is preferred with an IP fallback

The claim that Android universally fails to resolve `.local` is rejected.
Android's resolver has supported `.local` mDNS since November 2021, but browser,
device, VPN, multicast, router, and isolation behavior still require real-device
testing and an IP fallback. [Android DNS resolver](https://source.android.com/docs/core/ota/modular-system/dns-resolver)

### C-04 — No elevation or `uiAccess`

Tether remains a standard unelevated application. It does not request
Administrator privileges, set `uiAccess`, or add a privileged helper. UIPI
limitations are documented and diagnosed as far as the API permits; Windows
does not reliably identify UIPI as the cause of a `SendInput` failure.
[Microsoft `SendInput`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput)

### C-05 — No automatic clipboard or keymap mutation

Clipboard replacement is not an automatic X11 or Wayland text fallback. It can
expose or destroy sensitive user state and cannot be made safe by an unnoticed
restore attempt. Global X11 keymap mutation is also rejected. Either mechanism
would require a separately approved, explicit user-facing privacy design.

### C-06 — Preserve dedicated strips and neutral corner

Directional axis locking may improve recognition but does not replace the
physical neutral corner or dedicated scrolling geometry. Generic directional
gestures are not substituted for the specified controls.

### C-07 — Preserve justified crate boundaries

No blanket five-crate consolidation was accepted. Network, discovery, pairing,
protocol, session/input core, and platform backends have distinct dependency,
test, build-target, and unsafe/FFI boundaries. Crates may be merged or split only
when a concrete dependency or build analysis demonstrates a benefit without
weakening platform isolation or testability.

### C-08 — Cookie authentication is not duplicated in JavaScript

The persistent credential is not copied into `ClientHello`; doing so would
require exposing an `HttpOnly` secret to JavaScript. Authentication occurs on
the WebSocket upgrade with the host-scoped cookie, followed by protocol/version
and capability negotiation. Strict Host and Origin allowlists, on-link peer
admission, and session authorization provide independent layers.

### C-09 — No guessed permanent port or universal video workaround

Example ports from the review were not adopted without service-assignment and
packaging review. The network contract selects a configurable product default,
handles collision visibly, and persists an approved fallback. Likewise, a
muted-video keep-awake technique is an optional compatibility fallback, not a
cross-browser guarantee.

### C-10 — No unvalidated Wayland bypass

wlroots-family support is not inferred from project family names, and
`/dev/uinput` or unstable compositor protocols are not universal fallbacks.
Support is claimed per validated compositor/portal/EI configuration. Unsupported
configurations receive accurate capability reporting.

## Binding architectural invariants

- The desktop computer hosts all mobile assets and control services locally.
- Normal operation has no cloud, account, relay, central server, telemetry, or
  mandatory internet dependency.
- Administrative mutations are local Tauri IPC only.
- One authenticated controller owns input at a time.
- Every native action passes through `InputBackend` and the ordered dispatcher.
- Emergency release cannot be starved by normal input traffic.
- Secrets and input contents never enter logs, diagnostics, or raw traces.
- Platform support is an evidence claim backed by real hardware and signed
  package testing.
- The final product includes the full mouse, scroll, keyboard, shortcut, media,
  pairing, device-management, lifecycle, and packaging scope; it is not an MVP.

## Remaining validation decisions

No unresolved architectural conflict blocks implementation. The exact supported
OS/browser matrix, product default port, Wayland Rust adapter, queue/rate
constants, keep-awake fallback matrix, and platform tuning values are bounded
implementation-validation outcomes. They are tracked in `docs/open-risks.md` and
must not be represented as established support before their gates pass.
