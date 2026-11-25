# Current implementation and validation matrix

**Date:** 2026-11-21

**Release claim:** None. The complete Tether application is not yet built or
validated end to end. Availability, compilation, inert tests, and a successful
test-only browser probe are separate evidence, not interchangeable support.

ADR 0004 now prioritizes the Windows 11 x86-64 personal artifact, then macOS,
with Linux work deferred. The owner accepts untested Windows development
artifacts and approved cross-build tool acquisition. ADR 0006 also allows
the standalone personal Windows test build with a literal-IP QR code.

The shared protocol/generation snapshot is now committed as `a5a8da0` and passes
independent staged-snapshot checks. The
[Git checkpoint](../manual-tests/2026-11-16-git-protocol-commit.md) records exact
scopes; no platform's native validation status changes from that result.

PR-004 wire admission and SS-001's written session model are committed separately.
PR-005's two-language protocol fixtures passed locally and on hosted Windows,
Linux, and macOS runners. Runtime and browser evidence remains in the support
rows below.
[Protocol CI run](https://github.com/nohac14/tether/actions/runs/44968258011)
The shared actor now uses the transactional cursor and non-waiting receipts.
Mock tests exercise ownership, release, bounded admission and application
liveness. Physical PC input and phone pairing are still downstream.
The [Windows checkpoint](../manual-tests/2026-11-19-windows-foundation.md) records
toolchain setup, fractional pointer and wheel conversion, and direct observations
from the Windows candidate host.

The [text-entry checkpoint](../manual-tests/2026-11-21-mobile-text-entry.md)
records browser focus, composition, ordered backspace and ownership cleanup
with a mock controller. Physical phone and native input gates remain open.

The [authenticated control checkpoint](../manual-tests/2026-11-21-windows-control-session.md)
now covers the built browser, protected store, WebSocket and mock dispatcher
together over Windows loopback, including reload reconnect and neutralization.
The [personal test build checkpoint](../manual-tests/2026-11-21-windows-personal-test-build.md)
records startup, local IPC, native network inventory, HTTP serving over the
owner's approved Ethernet address, and normal exit of the standalone Windows
executable.

| Configuration | Implemented / compiled evidence | Physical/runtime evidence | Remaining boundary |
| --- | --- | --- | --- |
| Owner's Apple Silicon Mac, macOS 15.7.5 (24G624), arm64 | Shared foundations, partial live Quartz backend, interface policy/provider and listener supervisor; accepted ADR 0003 native Bonjour boundary with SDK/inert tests and a guarded diagnostic | Prior no-post/Unix/inventory/loopback checks plus separately approved en0/TCP 54322 registration, exact local A/AAAA/SRV/TXT, browse add/removal and repeated graceful listener cleanup | No production LAN host, native input application matrix, packet containment, real conflicts/flaps/permission denial, phone discovery, finished UI/media or complete app; DS-002 remains in progress and packaged/privacy/signing gates remain open |
| Owner's physical iPhone, Safari with reported iOS 27.0.1 | BV-001 test-only page uses actual host/active port and JavaScript messages | Owner reports HTTP/WS round trips, Disconnect/reconnect, return from app switch/Wi-Fi change, and failure after server stop all pass; no prompts observed | No separate Safari/build/model collected; no denial/reset, `.local`, pairing/cookies, real keyboard/gestures, or finished product tested |
| Owner's iPhone and Windows personal test build | LAN pairing and live keyboard with bounded repeated deletion | The owner reported successful control and live typing. Holding Backspace could pause live edits. The fix passed a headless browser fixture. | Physical held-Backspace retest, Home Screen credential retention, startup and discovery remain open. |
| Windows 11 Pro 25H2 x64, build 26200.9974 | Standalone personal test shell, private profile selection, literal-IP pairing QR, authenticated WebSocket, ordered Win32 input backend, and protected settings and paired devices. | Clean-folder and normal-profile launches, local IPC, repeated Ethernet start and stop with QR and HTTP from the same PC, and headless Edge loopback pairing and WebSocket control with mock input passed. | Physical phone reachability and pairing, firewall behavior across profiles, native input and release, UIPI/DPI, installed storage recovery, discovery, and signed packaging remain open. |
| Linux X11 x86-64 | Inert mapper plus cross-target compile checks; pure interface policy cross-compiles | No X server/runtime validation | Named distribution/desktop/input/provider/package matrix not validated |
| Linux GNOME/KDE Wayland x86-64 | Portal/EI inert spike and historical cross-target compile checks | No portal/compositor session or native input | R-009, R-019 and adapter acceptance remain open; no production Wayland support claim |
| Android Chrome, iPad Safari, Brave, other phone/browser/desktop architectures | No result inferred from the owner configuration | Not tested | Expansion gates remain unperformed |

The phone result is [owner-reported physical evidence](../manual-tests/2026-10-25-iphone-safari-lan.md).
Automated and manual provenance remains in dated checkpoint records; the
network contract and policy checkpoint do not supersede missing native tests.
Current Windows evidence is in the
[listener/session safety checkpoint](../manual-tests/2026-11-21-listener-session-safety.md),
[pairing and Win32 record checkpoint](../manual-tests/2026-11-21-windows-pairing.md),
[ownership and heartbeat checkpoint](../manual-tests/2026-11-20-session-control.md),
[mobile control checkpoint](../manual-tests/2026-11-20-mobile-controls.md),
[HTTP and mobile-shell checkpoint](../manual-tests/2026-11-20-http-and-mobile-shell.md),
[listener supervisor checkpoint](../manual-tests/2026-11-20-listener-supervisor.md),
[paired-device checkpoint](../manual-tests/2026-11-19-paired-device-store.md)
and [mobile bundle checkpoint](../manual-tests/2026-11-20-mobile-bundle.md).
Exact hardware model could not be read in the sandbox; no identifier, private
network address, phone serial, or secret is stored here.

ADR 0001 defers paid/public signing and distribution gates for personal builds,
not input safety, authentication, native behavior, or honest evidence. A future
IT-006/RL-002 reconciliation must replace this development matrix with the
actual configuration/package/browser claims accepted for that release.

The Windows personal executable now supports closing to the tray and opting
into hidden startup at sign-in. The packaged tray lifecycle was exercised on
Windows. See the [Windows test record](../manual-tests/2026-11-21-windows-personal-test-build.md#windows-tray-and-startup-on-2026-11-25).

The Windows test build also supports opting into automatic listening on a
verified private network with an existing pairing. Initial native coverage
uses a private Ethernet network on one PC.
