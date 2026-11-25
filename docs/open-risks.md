# Tether open risks and validation register

**Status:** Active  
**Last reviewed:** 2026-11-21
**Scope:** Remaining uncertainty after the accepted architecture decisions.

This register contains validation risks, not permission to weaken `SPEC.md`.
Closing a risk requires recorded evidence from the named gate. A failed gate
must narrow the support claim, disable the capability, or produce an ADR; it
must not be converted into an undocumented workaround.

The [standalone Windows test executable checkpoint](manual-tests/2026-11-21-windows-personal-test-build.md)
records protected startup and loopback pairing with mock input. Physical
LAN reachability, firewall behavior, native input release on lifecycle
events, installed recovery, and Windows discovery still require validation.
R-018 also tracks an intermittent unpublished-pairing rollback test result.
The checkpoint records the failure and the successful reruns.
The same checkpoint records a Windows TCP port exclusion which stopped the
initial test. The build now lets the owner select another port explicitly.

## Severity and state

- **Release blocker:** prevents the affected support claim or production release.
- **Capability blocker:** permits release only with the affected feature disabled
  and accurately reported.
- **Tracked:** requires evidence but does not independently block unaffected
  configurations.
- **Open:** evidence is not yet complete.
- **Closed:** evidence and resulting documentation are linked from this file.

## Risk register

ADR 0004 prioritizes Windows development artifacts. The Windows 11 x64 host
now links the desktop skeleton with MSVC, and the `SendInput` export resolves.
Guarded input, storage security, application validation and cross-SDK provenance
remain open under R-006/R-015/R-018/R-023. Linux work is deferred and R-019
remains open. See the
[Windows checkpoint](manual-tests/2026-11-19-windows-foundation.md).

| ID | Severity | State | Risk and impact | Required validation / exit criterion | Plan gate |
| --- | --- | --- | --- | --- | --- |
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
| R-011 | Tracked | Open | NW-001 selects configurable port 54321/TCP in the IANA Dynamic/Private range; actual collision, firewall, and package behavior still need validation. | Implement and test visible collision handling, explicitly approved/persisted alternatives, QR/mDNS updates, upgrade behavior, and narrowly scoped firewall rules. Registry review is dated 2026-10-25 and is not an exclusive registration or proof of local availability. | NW-003, DS-003, PK-001 |
| R-012 | Tracked | Open | Browser LAN access policy continues to evolve and may add prompts or restrictions despite same-origin delivery. | Under owner-approved ADR 0002, run BV-001 first on the actual physical iPhone/Safari before the personal-use network contract: direct navigation, observed permissions/denial where exposed, same-origin WebSocket with the active port, and reconnect. Record absent prompts and unavailable denial/reset tests rather than inventing them. Android Chrome LNA remains a separate expansion gate before Android claims. Retest actual supported versions after browser changes. | BV-001, NW-008, IT-005 |
| R-013 | Tracked | Open | `SIGKILL`, `TerminateProcess`, kernel failure, sudden power loss, or a failed injection facility can prevent release cleanup. | Fault tests cover every observable path, panic unwind, and startup neutralization. User docs state the uncatchable limit; no test or marketing language claims otherwise. | SS-004, IT-001, RL-001 |
| R-014 | Release blocker | Open | Pointer feel can vary with phone sampling, server coalescing, OS acceleration, display density, user settings, and mixed-monitor boundaries. | Measure ordered end-to-end behavior on representative phone/desktop pairs and each claimed OS. Establish per-platform tuning ranges without reordering clicks or relying on absolute screen coordinates. | IC-004, WIN/MAC/X11/WAY native gates, IT-003 |
| R-015 | Release blocker | Open | Actual hardware, signed identities, and clean OS installations may not be available for every intended target. | Create a test inventory before release work. A missing configuration is either acquired/tested or removed from the support claim; CI/cross-compilation never substitutes for evidence. | A-004, IT-006, RL-002 |
| R-016 | Release blocker | Open | Malformed/flooded clients can exhaust parser, connection, session, dispatcher, or OS input queues and could starve release work. | Fuzz/load tests demonstrate bounded memory, CPU, connections, messages, and queued bytes. Saturation clears discardable motion, executes priority release, closes the session, and leaves the mock/native backend neutral. | PR-004, NW-007, IT-002, IT-004 |
| R-017 | Tracked | Open | Browsers may purge persistent cookies or segregate storage for bookmarks/home-screen modes, causing apparent credential loss. | Test normal tabs, bookmarks, private modes as applicable, and supported home-screen conveniences. Loss returns to an understandable re-pair flow and never exposes the credential to JavaScript. | PA-001–PA-003, IT-005 |
| R-018 | Release blocker | Open | Windows settings, identity and paired devices share ADR 0005's DACL and handle checks. Forty-one unelevated Windows configuration tests passed, including paired-device restart and injected commit failures. Unsupported permissions, broad or inherited ACLs and unsafe entries fail closed. | Independently review the Windows boundary and qualify installed storage with a clean upgrade, actual interruption/recovery and production pairing/session integration. | ST-002/ST-003 Windows follow-up, PA-003, IT-004, PK-001 |

[Accepted ADR 0005](adr/0005-windows-private-storage.md) scopes the Win32 calls
and native checks. The [Windows storage checkpoint](manual-tests/2026-11-19-pairing-and-assets.md#windows-storage)
records the owner's approval, the initial sandbox failure and the passing test run.

R-005 now has [headless Edge evidence](manual-tests/2026-11-21-mobile-text-entry.md)
for trusted activation, confirmed composition, and single backspace delivery.
The physical software-keyboard and Compose and Send checks remain open.

## 2026-11-14 tooling security refresh

The [November 19 admission cursor](manual-tests/2026-11-19-session-admission.md)
adds stateful preparation/commit and queued held-state projection coverage, not
a single-owner runtime or native cleanup proof. R-016 stays open for actor,
queue/load and complete release integration. No cursor reset can resume or clear
a terminally faulted dispatcher. The owner's newly available Windows machine is
an opportunity for future evidence, not an executed validation or an R-018 waiver.

The November 16 [protocol fixtures](manual-tests/2026-11-16-protocol-fixtures.md)
and [session prerequisites](manual-tests/2026-11-16-session-prerequisites.md)
provide partial R-016 evidence: bounded wire validation, two-language synthetic
boundary/malformed fixtures and non-waiting receipt inspection. They do not close
R-016: the session actor, reserved delivery under full-system load and native
neutralization still need their SS/NW/IT gates. Pending/failed cleanup is never
treated as neutral, and a later successful release never clears terminal faults.
R-018/R-019/R-023 and every native/browser support boundary remain unchanged.

The [2026-11-16 Git checkpoint](manual-tests/2026-11-16-git-protocol-commit.md)
closes the PR-003 local-commit blocker through normal approved Git operations.
It does not close R-018/R-023 or native support gates. The isolated committed
dependency graph was checked again and still fails only the same three R-019
advisories; bans/licenses/sources pass. No ignore or Linux dependency removal
was introduced.

R-023 — **Windows cross-build acquisition blocker, open 2026-11-14:**
official VS 17 release and VS 18 stable channel metadata both advertise a
manifest payload digest/size that differs from the corresponding HTTPS body.
No SDK payload was used. xwin 0.10.0's source omits that manifest checksum check;
do not treat the tool's default omission as satisfying Tether's provenance gate.
The owner-approved native Windows MSVC setup now provides the linker and SDK
through Microsoft's installer and passes workspace linking. Cross-SDK
acquisition still requires matching authenticated Microsoft metadata and
artifacts. The official LLVM archive hash/attestation verifies; no
TLS/DNS/security checks were disabled.
Detailed expected/observed hashes are in the
[dated checkpoint](manual-tests/2026-11-14-protocol-tooling.md). R-018 is separate:
an executable built elsewhere still cannot enable Windows persistence until
its native security tests pass.

R-022 — **Development dependency verification blocker, closed 2026-11-14:**
the refreshed npm audit found moderate GHSA-g3hv-75wg-m249 in locked minimatch
10.2.6 through four lint/tooling parents. It is not a demonstrated Tether LAN
exploit. The owner approved four exact
parent-scoped 11.0.1 overrides with compatibility/source/transitive review;
installation, frozen lockfile, clean complete audit, five new compatibility
regressions and the full repository aggregate now pass. Direct pins and prior
overrides are unchanged. No ignore/severity waiver or global override was added.
See the
[proposal/evidence](reviews/2026-11-14-npm-tooling-findings.md). Historical clean
audits remain historical results, not a current clean claim.

## 2026-10-25 discovery candidate blocker

R-021 — **Discovery implementation/capability blocker, open 2026-10-25:**
mdns-sd 0.21.6's worker performs initial all-interface multicast setup before
client selection commands. The source-reviewed candidate is not adopted or
runtime-started. The owner accepted ADR 0003 on 2026-10-25; the reviewed Mac-native
ABI spike compiles and has inert tests. Separately approved en0/TCP 54322 native
registration, exact local scoped A/AAAA/SRV/TXT, browse add/removal and graceful
stop now pass; physical phone and packet containment have not run.
Validate per-interface containment, custom-host A/AAAA, multiple addresses in
one RRset, conflicts, withdrawal, daemon/permission failure and native lifecycle.
The Windows candidate projection rejects ambiguous or unsupported native
scopes and retains all existing IP fallback endpoints. No Windows
registration or LAN containment test has run.
[Discovery contract](components/discovery.md)
Synchronous system calls can stall despite readiness: DS-003 must isolate the
owner and bound external waits without unsafe concurrent deallocation.
D-03 is unchanged. DS-002 is in progress; DS-003 remains blocked. See the
[candidate review](reviews/2026-10-25-mdns-candidate-review.md),
[binding review](reviews/2026-10-25-bonjour-binding-review.md) and
[qualification procedure](manual-tests/macos-bonjour-qualification.md).

## 2026-10-23 macOS and advisory checkpoint

R-019 — **Verification/release blocker, open:** the 2026-10-23 pinned
cargo-deny refresh fails on Linux Wayland transitives `zvariant` 5.15.1,
`zbus_names` 4.3.5, and `zvariant_derive` 5.15.1. RustSec
[0370](https://rustsec.org/advisories/RUSTSEC-2026-0370.html),
[0371](https://rustsec.org/advisories/RUSTSEC-2026-0371.html), and
[0372](https://rustsec.org/advisories/RUSTSEC-2026-0372.html) were issued on
2026-10-17 after the previous successful run. They report unmaintained standalone
crates merged into zbus 6, not a demonstrated exploitable defect in Tether.
Review the ashpd/zbus migration and rerun compatibility, license, source, and
advisory checks before closing the gate. Do not remove the Linux crates, waive
advisories, or change reviewed pins silently to obtain green results. The new
Mac code adds no new registry package/version and does not use these transitives.

The 2026-11-14 cargo-deny 0.20.2 refresh still fails on the same three advisories
with no new advisory finding. Full-graph bans/licenses/sources pass after the
exact reviewed development-only foldhash 0.1.5 Zlib disposition. Additional
individual Windows x86-64 and Mac arm64 target checks pass; these are dependency
graph results on this Mac, not native execution or a substitute for the failed
Linux-inclusive gate. Linux migration remains deferred, not silently waived.

The 2026-10-23 macOS checkpoint implements actual posting code but deliberately
does not execute it. The non-prompting probe reports no Accessibility grant;
no prompt was requested. Non-injecting native event construction/inspection
passes with WindowServer access, while ordinary sandboxed construction fails.
R-007/R-014 remain open, including application behavior, actual notification
delivery, media, displays, and permission persistence. ADR 0001 permits unsigned
personal builds but does not turn those results into validated support.

### Frontend tooling and pinned runtime

R-020 — **Security/verification blocker, closed 2026-10-23:** the initial npm
refresh found seven development-tool advisories (two high, three moderate,
two low). Vite 8.1.0 was affected by GHSA-698v-4r3x-3vcv,
GHSA-r825-w8mm-5pxw, [GHSA-587h-7wmj-h5pm](https://github.com/advisories/GHSA-587h-7wmj-h5pm),
GHSA-9jrq-w75r-8gcw, and GHSA-rq7h-c2jc-7f22. markdown-it 14.3.2 was
affected by GHSA-vfgh-jcmw-g37q and
[GHSA-gfr2-rv44-r23c](https://github.com/advisories/GHSA-gfr2-rv44-r23c).
The owner authorized only Vite 8.1.6, markdown-it 14.3.3 within the existing
range, necessary transitives, and restoration of pinned Node 24.19.0. The
resolved graph changes only those two package versions; all other direct pins,
overrides, and package integrity hashes are unchanged. Frozen installation and
the full npm audit now pass with zero findings. Official Node's Darwin-arm64
archive matched its published SHA-256 before extraction; verification uses
that isolated runtime with pnpm 11.25.0, not the drifted bundled Node 24.22.1.
The final verification record is
[the scoped remediation report](reviews/2026-10-23-tooling-security-remediation.md).
No vulnerable dev server was exposed; the application must never ship Vite's
development server. The separate R-019 Rust advisory gate still fails.

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

## 2026-10-16 Wave 8 evidence update

The inert mapper milestone reduced implementation uncertainty without closing
native risks. Windows x86-64, X11 Linux x86-64, and Wayland Linux x86-64 binding
surfaces cross-compile with strict Clippy; macOS bindings compile natively and
the non-prompting Accessibility preflight executed. All four mapper suites pass
against inert sinks, including unsupported HID usages and cleanup/error paths.

R-006 remains open because no Windows API call or runtime environment was used.
R-007 remains open because no event posting, stable signing identity, permission
lifecycle, or application/display matrix was tested. R-008 remains open because
there was no X server or live XKB/XTEST observation. R-009 remains open because
there was no GNOME/KDE portal/EI session and `reis` is still a provisional
adapter. See `components/native-backend-spikes.md` and the dated milestone
record for the exact evidence classification.

The initial inventory was limited to an Apple Silicon Mac with 16 GiB RAM and
approximately 60 GiB free disk. The owner subsequently reported a Windows 11
AMD PC, providing a candidate native Windows/x86-64 test host. Its exact
architecture/build, tools, access, and resources remain unverified. No physical
Android phone is available. The owner subsequently confirmed an iPhone using
Safari and explicitly approved ADR 0002's iPhone-first BV-001 gate. On
2026-10-25 the owner reported iOS 27.0.1 Safari successfully loading the HTTP
page, completing same-origin JavaScript round trips, and stopping the count on
Disconnect, with no prompts observed. The owner then explicitly confirmed all
four reconnect/app-switch/Wi-Fi/server-stop checks passed. BV-001 is complete
for that personal configuration; R-012 remains open for broader untested
conditions, packaged identity, and expansion platforms. See
[the scoped physical evidence](manual-tests/2026-10-25-iphone-safari-lan.md).
`manual-tests/test-environment-setup.md` now prefers
Windows preparation and assessment of Linux VM capacity. The November 19
Windows checkpoint adds host inventory and inert input-mapper evidence.
R-006, R-008–R-010, R-012, R-015, and R-018 remain open;
neither cross-compilation nor ARM emulation/virtualization satisfies their
missing physical-device, target-architecture, permission, or native test gates.

## Architectural questions remaining

NW-002's Mac provider has now executed read-only on the available host; this
does not validate network-change handling, physical IPv6 connectivity or
Windows/Linux providers. OS service identities do not prove network continuity;
automatic restoration remains closed when context is unknown. Darwin's short
socket-creation/close-on-exec window must be addressed by the future child-spawn
layer's close-default/serialization policy. See the
[provider checkpoint](manual-tests/2026-10-25-interface-provider.md).

The [Windows profile and TCP checkpoint](manual-tests/2026-11-20-windows-network-profiles-and-tcp.md)
records native IP Helper and Network List Manager queries plus loopback
socket checks. Windows profile metadata invalidates approvals when a
change is observed. The Windows personal build reads the default IPv4
gateway and its link-layer address to corroborate the private profile for
automatic startup. Qualification across boots, Wi-Fi and multiple adapters
remains open. Physical DHCP, Windows firewall behavior and native IPv6 LAN
reachability also remain open.

The [listener supervisor checkpoint](manual-tests/2026-11-20-listener-supervisor.md)
adds loopback authority revocation and mock dispatcher release evidence.
The production session and input coordinator, timing under actual LAN
traffic, and same-port rebinding after active Windows connections still
need qualification.

The [HTTP and mobile-shell checkpoint](manual-tests/2026-11-20-http-and-mobile-shell.md)
adds raw HTTP framing tests and Windows loopback serving through the
supervisor. Published `.local` aliases and actual IPv6 LAN reachability
remain downstream. The appearance gutter is provisional pending iOS
and Android edge-gesture testing.

The [mobile control checkpoint](manual-tests/2026-11-20-mobile-controls.md)
adds simulated tap timing, drag/capture cancellation, bounded momentum, and
ordered Protobuf batch evidence in a browser mock. R-004 and R-014 still need
physical device and Windows input behavior. R-016 still needs production
WebSocket congestion and server-side neutralization evidence.

The [ownership and heartbeat checkpoint](manual-tests/2026-11-20-session-control.md)
adds mock neutralization after abort, timeout, transport retirement, ownership
transfer and queue saturation. R-016 still needs production transport load and
native release validation.

The [Windows pairing checkpoint](manual-tests/2026-11-21-windows-pairing.md)
adds HTTP-to-store credential publication, scoped revocation, browser cookie
receipt and inert Win32 record verification. R-006 still needs insertion in an
isolated interactive Windows session. R-018 still needs installed storage
recovery, and R-016 needs real WebSocket and native backend composition.

The [listener safety checkpoint](manual-tests/2026-11-21-listener-session-safety.md)
adds a mock supervisor/actor rebind with held input, a dropped cleanup ticket,
and an unconfirmed release deadline. R-016 retains its live transport and
native release conditions.

DS-002's subsequent source review now proves mdns-sd 0.21.6 cannot establish
the exact allowlist before its worker's initial multicast setup. It is not
adopted; no daemon ran, lockfile dependency was added or D-03 requirement waived.
Command/event channels are bounded, but cache/resource/lifecycle gates remain
unvalidated. The [candidate review](reviews/2026-10-25-mdns-candidate-review.md)
and accepted ADR 0003 record the owner-approved Mac-native alternative. The
binding review and inert/SDK checks are implemented; initial scoped Mac native
registration/query/cleanup passes are recorded in the
[Bonjour checkpoint](manual-tests/2026-10-25-bonjour-boundary.md). The rest of the
native matrix is untested and R-021 is not closed by those partial results.

NW-002's read-only Mac provider and NW-003's supervisor now pass their scoped
host/model/loopback checks. That does not validate physical interface changes,
IPv6 LAN connectivity, packaged permission/firewall behavior or other platforms.
Network-context stability remains unestablished; matching names/indexes/addresses
never justify auto-listen. The coordinator/session/HTTP integration is not
implemented. See the [listener evidence](manual-tests/2026-10-25-listener-supervisor.md).

Discovery's owner decision is resolved by accepted ADR 0003; remaining native
gates do not block unrelated ready tasks or change D-01–D-05. The following other
choices remain deferred to measured implementation gates:

1. native collision/firewall behavior for the selected configurable default port;
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

The Windows personal tray build still needs an actual sign-out/sign-in and
phone session across window close. The native tray and hidden-launch evidence
is in the [Windows test record](manual-tests/2026-11-21-windows-personal-test-build.md#windows-tray-and-startup-on-2026-11-25).
