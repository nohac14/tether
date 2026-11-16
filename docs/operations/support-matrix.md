# Current implementation and validation matrix

**Date:** 2026-11-16

**Release claim:** None. The complete Tether application is not yet built or
validated end to end. Availability, compilation, inert tests, and a successful
test-only browser probe are separate evidence, not interchangeable support.

ADR 0004 now prioritizes the Windows 11 x86-64 personal artifact, then macOS,
with Linux work deferred. The owner accepts untested Windows development
artifacts and approved cross-build tool acquisition, not a Windows support claim
or R-018 waiver. No Windows executable has been produced by this priority
change; earlier runtime evidence in this matrix retains its recorded date.

The shared protocol/generation snapshot is now committed as `a5a8da0` and passes
independent staged-snapshot checks. The
[Git checkpoint](../manual-tests/2026-11-16-git-protocol-commit.md) records exact
scopes; no platform's native validation status changes from that result.

| Configuration | Implemented / compiled evidence | Physical/runtime evidence | Remaining boundary |
| --- | --- | --- | --- |
| Owner's Apple Silicon Mac, macOS 15.7.5 (24G624), arm64 | Shared foundations, partial live Quartz backend, interface policy/provider and listener supervisor; accepted ADR 0003 native Bonjour boundary with SDK/inert tests and a guarded diagnostic | Prior no-post/Unix/inventory/loopback checks plus separately approved en0/TCP 54322 registration, exact local A/AAAA/SRV/TXT, browse add/removal and repeated graceful listener cleanup | No production LAN host, native input application matrix, packet containment, real conflicts/flaps/permission denial, phone discovery, finished UI/media or complete app; DS-002 remains in progress and packaged/privacy/signing gates remain open |
| Owner's physical iPhone, Safari with reported iOS 27.0.1 | BV-001 test-only page uses actual host/active port and JavaScript messages | Owner reports HTTP/WS round trips, Disconnect/reconnect, return from app switch/Wi-Fi change, and failure after server stop all pass; no prompts observed | No separate Safari/build/model collected; no denial/reset, `.local`, pairing/cookies, real keyboard/gestures, or finished product tested |
| Windows 11 AMD PC, exact build/resources not observed | Windows inert mapper and Windows/x86-64 cross-target checks; pure interface/config code cross-compiles | No Windows native execution | R-018 persistence remains disabled; no UIPI/DPI/input, installer/firewall, native identity/provider, or end-to-end claim |
| Linux X11 x86-64 | Inert mapper plus cross-target compile checks; pure interface policy cross-compiles | No X server/runtime validation | Named distribution/desktop/input/provider/package matrix not validated |
| Linux GNOME/KDE Wayland x86-64 | Portal/EI inert spike and historical cross-target compile checks | No portal/compositor session or native input | R-009, R-019 and adapter acceptance remain open; no production Wayland support claim |
| Android Chrome, iPad Safari, Brave, other phone/browser/desktop architectures | No result inferred from the owner configuration | Not tested | Expansion gates remain unperformed |

The phone result is [owner-reported physical evidence](../manual-tests/2026-10-25-iphone-safari-lan.md).
Automated and manual provenance remains in dated checkpoint records; the
network contract and policy checkpoint do not supersede missing native tests.
Exact hardware model could not be read in the sandbox; no identifier, private
network address, phone serial, or secret is stored here.

ADR 0001 defers paid/public signing and distribution gates for personal builds,
not input safety, authentication, native behavior, or honest evidence. A future
IT-006/RL-002 reconciliation must replace this development matrix with the
actual configuration/package/browser claims accepted for that release.
