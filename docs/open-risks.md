# Tether open risks and validation register

**Status:** Active  
**Last reviewed:** 2026-10-12
**Scope:** Remaining uncertainty after the accepted architecture decisions.

This register contains validation risks, not permission to weaken `SPEC.md`.
Closing a risk requires recorded evidence from the named gate. A failed gate
must narrow the support claim, disable the capability, or produce an ADR; it
must not be converted into an undocumented workaround.

## Severity and state

- **Release blocker:** prevents the affected support claim or production release.
- **Capability blocker:** permits release only with the affected feature disabled
  and accurately reported.
- **Tracked:** requires evidence but does not independently block unaffected
  configurations.
- **Open:** evidence is not yet complete.
- **Closed:** evidence and resulting documentation are linked from this file.

## Risk register

| ID | Severity | State | Risk and impact | Required validation / exit criterion | Plan gate |
|---|---|---|---|---|---|
| R-001 | Release blocker | Open | Baseline HTTP/`ws://` provides no confidentiality or transport authentication against an active or passive LAN attacker. A stolen cookie permits input impersonation until revoked. | Threat-model review confirms all warnings and mitigations; packet-level tests demonstrate no false encryption claim; public docs clearly require a trusted private LAN. A future encrypted design requires its own ADR and authenticated web-delivery solution. | IT-004, RL-001 |
| R-002 | Release blocker | Open | `.local` resolution and DNS-SD behavior vary with Android/iOS versions, routers, multicast filtering, VPNs, client isolation, hostname conflicts, and macOS Local Network permission. | Physical-device matrix verifies preferred `.local`, conflict rename, DHCP change, and IP-fragment fallback. Every failed `.local` case has an actionable fallback and separate-scope re-pair flow. | DS-002, NW-008, IT-005 |
| R-003 | Tracked | Open | Screen Wake Lock is normally unavailable over baseline HTTP; video-based workarounds may fail, stop, consume power, or change across browsers. | Test native API eligibility and revocation plus explicitly activated video fallback on every claimed mobile browser. Document per-browser result and battery behavior; unsupported cases remain normal screen-timeout behavior. | MW-011, IT-005 |
| R-004 | Release blocker | Open | iOS browser history gestures and Android system back gestures may capture touches near either physical edge, reducing vertical-strip usability. | Real-device left/right-strip tests determine responsive gutter/safe-area rules in portrait/landscape without shrinking below the touch-target requirement; verify neutral corner and horizontal strip remain distinct. | MW-001, MW-005, IT-005 |
| R-005 | Release blocker | Open | Mobile keyboard activation, `beforeinput`, IME, autocorrect, dictation, and `visualViewport` behavior differ across Safari/Chrome and OS versions. | Physical keyboard/IME matrix verifies synchronous focus, viewport anchoring, local-buffer reconciliation, Compose and Send, cancellation, and no duplicate/partial text. Unsupported live edits transition honestly to Compose and Send. | MW-007, MW-008, IT-005 |
| R-006 | Release blocker | Open | `SendInput` behavior varies across applications, keyboard layouts, user pointer acceleration, mixed-DPI monitors, horizontal-wheel consumers, and UIPI boundaries. | Guarded native tests on every claimed Windows edition validate event structures, surrogate pairs, release, acceleration settings, display crossings, normal/elevated targets, installer, firewall, and lifecycle. | WIN-001–WIN-003, PK-001 |
| R-007 | Release blocker | Open | macOS Accessibility and Local Network privacy depend on lifecycle and stable signed identity; CoreGraphics Unicode and relative motion behavior vary by target application and display setup. | Signed/notarized builds with stable bundle ID/build UUID pass fresh grant, deny, revoke, upgrade, sleep/wake, multi-display, acceleration, Unicode-app, and local-network retry tests on each claimed macOS/architecture. | MAC-001–MAC-003, PK-002 |
| R-008 | Capability blocker | Open | XTEST exposes keycodes and discrete wheel buttons, not general Unicode or high-resolution scroll. Layout/group changes and horizontal-scroll application behavior vary. | Named X11 configurations validate XTEST detection, current XKB mapping updates, layout-mappable preflight, unmappable errors, independent fractional accumulators, balanced Buttons 4–7, release, logout, and package dependencies. | X11-001–X11-003 |
| R-009 | Capability blocker | Open | Wayland support depends on portal v2, compositor EIS behavior, permission grants, restore-token rotation, EI capabilities, and the maturity of the Rust EI adapter. Optional EI text may be absent or differ by version. | Time-boxed `ashpd`/`reis` spike and guarded GNOME/KDE tests cover handshake, denial, restore, pointer, both scroll axes, keys, optional UTF-8 text limits/order, pause/resume/removal, compositor restart, and release. Select `reis`, approve isolated libei FFI by ADR, or narrow support. | WAY-001–WAY-004 |
| R-010 | Release blocker | Open | Linux package/runtime compatibility depends on distribution WebKitGTK/GTK, tray integration, portal packages, XTEST/XKB/libei availability, architecture, and display server. | Define exact distro/desktop versions from native clean-system tests; verify `.deb`/`.rpm`, optional AppImage, offline launch, autostart, permissions, and both supported input paths. | X11-003, WAY-004, PK-003–PK-004 |
| R-011 | Tracked | Open | The exact default server port and collision/firewall policy are not yet validated; an unstable fallback breaks bookmarks and firewall expectations. | Check current service assignments; select a documented configurable default; test visible collision handling, persisted fallback, QR/mDNS updates, upgrade behavior, and narrowly scoped firewall rules. | NW-001, NW-003, PK-001 |
| R-012 | Tracked | Open | Browser Local Network Access policy continues to evolve and may add prompts or restrictions despite same-origin LAN delivery. | Run the early BV-001 gate on a current physical Android/Chrome device before the network contract, then retest claimed Chrome/Android and Safari/iOS releases against direct navigation, same-origin WebSocket upgrade with the active port, permission denial, private/public address transitions, and browser updates. | BV-001, NW-008, IT-005 |
| R-013 | Tracked | Open | `SIGKILL`, `TerminateProcess`, kernel failure, sudden power loss, or a failed injection facility can prevent release cleanup. | Fault tests cover every observable path, panic unwind, and startup neutralization. User docs state the uncatchable limit; no test or marketing language claims otherwise. | SS-004, IT-001, RL-001 |
| R-014 | Release blocker | Open | Pointer feel can vary with phone sampling, server coalescing, OS acceleration, display density, user settings, and mixed-monitor boundaries. | Measure ordered end-to-end behavior on representative phone/desktop pairs and each claimed OS. Establish per-platform tuning ranges without reordering clicks or relying on absolute screen coordinates. | IC-004, WIN/MAC/X11/WAY native gates, IT-003 |
| R-015 | Release blocker | Open | Actual hardware, signed identities, and clean OS installations may not be available for every intended target. | Create a test inventory before release work. A missing configuration is either acquired/tested or removed from the support claim; CI/cross-compilation never substitutes for evidence. | A-004, IT-006, RL-002 |
| R-016 | Release blocker | Open | Malformed/flooded clients can exhaust parser, connection, session, dispatcher, or OS input queues and could starve release work. | Fuzz/load tests demonstrate bounded memory, CPU, connections, messages, and queued bytes. Saturation clears discardable motion, executes priority release, closes the session, and leaves the mock/native backend neutral. | PR-004, NW-007, IT-002, IT-004 |
| R-017 | Tracked | Open | Browsers may purge persistent cookies or segregate storage for bookmarks/home-screen modes, causing apparent credential loss. | Test normal tabs, bookmarks, private modes as applicable, and supported home-screen conveniences. Loss returns to an understandable re-pair flow and never exposes the credential to JavaScript. | PA-001–PA-003, IT-005 |
| R-018 | Release blocker | Open | Rust standard filesystem APIs provide the required lifetime locking and replacement primitives on current Windows, but do not expose explicit owner DACL construction or complete reparse/hard-link inspection. ST-002 and ST-003 therefore fail closed on Windows rather than accepting inherited ACLs as compliant. | Select and security-review a maintained safe Windows ACL/reparse implementation or approve a narrowly isolated audited Windows FFI boundary; cross-compile it and validate owner/System/Administrators access, broad-inheritance removal, reparse/hard-link attacks, lock contention, settings/identity replacement, power-loss recovery, reset, and upgrade behavior on actual Windows 11 before enabling persistent state. | ST-002/ST-003 Windows follow-up, IT-004, PK-001 |

## Required evidence format

Each manual platform record must include:

- application version/commit and signed package identity;
- hardware model and CPU architecture;
- OS, browser, desktop/compositor, portal, and relevant dependency versions;
- interface/network topology without secrets;
- exact test cases and observed capability results;
- logs or screenshots only after sensitive-data review;
- pass, fail, or not-run status; and
- the support-matrix/documentation changes resulting from the run.

Never place pairing tokens, credentials, typed text, key streams, clipboard
contents, cookie headers, raw control frames, or private user data in evidence.

## Architectural questions remaining

There is no unresolved architectural question that prevents beginning the
dependency-ordered implementation plan. The following choices are deliberately
deferred to measured implementation gates:

1. the exact product default port and persisted collision fallback;
2. the exact first release OS/browser/distribution version matrix;
3. `reis` versus an ADR-approved isolated `libei` FFI adapter;
4. per-browser availability of the optional video keep-awake fallback;
5. final queue, message, text-chunk, heartbeat, rate, and timing constants within
   the hard safety ceilings in `SPEC.md`; and
6. per-platform pointer, scroll, momentum, and drag-recognition tuning values.

These choices may narrow support or tune implementation parameters. They may
not introduce cloud services, privilege escalation, hidden clipboard/keymap
mutation, generic scroll gestures in place of the dedicated regions, or weaker
input cleanup.
