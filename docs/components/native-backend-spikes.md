# Native input backend spike contract and evidence

**Tasks:** WIN-001, MAC-001, X11-001, WAY-001
**Status:** Inert/cross-compilation milestone implemented; native environment gates remain
**Authority:** `SPEC.md` sections 14, 15, and 22–26; `input-backend.md`; `backend-conformance.md`

The Wave 8 spike code translates platform-neutral actions into redacted native
event plans, derives conservative capability snapshots from explicit probe
state, and runs those paths through the shared held-state/release conformance
harness with an inert sink. None of these crates posts an event in default
tests. Passing an inert test or a cross-compile is not a platform support claim.

## Shared safety boundary

- General keyboard availability is independent from individual HID mappings.
  A partially mapped backend advertises physical-key capability and returns the
  closed `Unsupported` backend error for a missing usage before sink execution.
- Event diagnostics redact input identities and text. Sink failures report only
  the number of leading native-plan records that may have been accepted.
- A partial batch marks native state uncertain. `release_all` remains bounded,
  attempts held keys in reverse order with modifiers last, and cannot report a
  confirmed neutral state while uncertainty or cleanup failures remain.
- Capability absence fails before backend execution through
  `execute_checked`. Dynamic native loss must be reflected by future live
  backends and routed to the dispatcher emergency-release path.
- Live workstation injection is forbidden in ordinary tests. It requires the
  later platform-specific opt-in procedure, warning, isolation, and safety
  controls.

## Windows `SendInput` spike

`tether-input-windows` uses `windows` 0.62.2 bindings and builds plans for
relative motion, five buttons, both wheel axes, scan/extended keys, media
virtual keys, and UTF-16 Unicode records. Supplementary Unicode scalars are
tested as ordered surrogate-unit down/up pairs. The binding surface
cross-compiles for `x86_64-pc-windows-msvc`.

Relative movement uses the shared fixed-point translator at one normalized
unit per native pixel before Windows applies user pointer settings. Each wheel
axis independently maps one normalized unit to 120 Win32 wheel units. Positive
wheel deltas go up vertically and right horizontally. The mapper preserves
signed fractional remainders, produces no record for zero native movement, and
clears all remainders through the inert release hook. These scales are the
initial conversion and still require application and hardware tuning.

The mapper is intentionally partial: reviewed HID usages map, while other
allowlisted usages return `Unsupported` without concealing keyboard capability.
No call to `SendInput` occurs. Consequently this spike does not validate UIPI,
OS pointer acceleration, partial native return counts, focused application
behavior, elevation boundaries, DPI, multiple monitors, or Windows release
support. Microsoft documents both the serial insertion semantics and UIPI
restriction of `SendInput` in the
[official API reference](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-sendinput).
The native host inventory and current linked-test scope are recorded in the
[Windows checkpoint](../manual-tests/2026-11-19-windows-foundation.md).

The Windows mapper now lowers plans to the generated Win32 `INPUT` structs.
Windows tests inspect relative coordinates, signed wheel data, the extra-button
codes, scan flags and UTF-16 events without submitting them to the OS.
`guarded_motion` is a separate opt-in diagnostic for an isolated Windows
session. Its native path submits one relative pointer movement and verifies the
insertion count. Only the refusal path ran on this workstation. [INPUT](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-input)
[KEYBDINPUT](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-keybdinput)
[MOUSEINPUT](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-mouseinput)

## macOS Core Graphics spike

`tether-input-macos` selects maintained generated
`objc2-core-graphics` 0.3.2 bindings instead of adding local FFI. It builds
plans for motion, buttons/click state, continuous scroll, virtual keycodes,
UTF-16 text, conventional Command chords, and auxiliary media keys. On the
available Apple Silicon macOS host, the crate compiles and executes Apple's
non-prompting `CGPreflightPostEventAccess` check; a false result remains a
prompt/denial-required state and exposes no capabilities. Apple documents the
preflight in the
[Core Graphics API](https://developer.apple.com/documentation/coregraphics/cgpreflightposteventaccess%28%29).

No event is created or posted. Accessibility prompt persistence, stable signed
identity, Unicode behavior across target applications, auxiliary media events,
relative movement/acceleration, mixed-density monitors, Intel macOS, sleep,
and signed/notarized packaging remain unvalidated under R-007 and MAC-002/003.

## X11 XTEST/XKB spike

`tether-input-x11` uses pure-Rust `x11rb` 0.14.0 with only `xkb` and `xtest`
features. Its probe snapshot distinguishes connection, XTEST, XKB, discovered
HID-to-keycode mappings, current-layout text mappings, and media keycodes. Text
is preflighted as a whole and never uses the clipboard or changes the global
keymap. Independent fixed-point residues produce balanced discrete Button 4/5
and 6/7 pairs and are cleared during release-all. The binding surface
cross-compiles for `x86_64-unknown-linux-gnu`.

The X Consortium's [XTEST specification](https://www.x.org/releases/current/doc/xextproto/xtest.pdf)
defines synthetic input requests but not arbitrary Unicode insertion. No X
server was available here, so extension versions, live keymap/group refresh,
connection loss, desktop packaging, horizontal-scroll consumers, and actual
native injection remain unvalidated under R-008 and X11-002/003.

## Wayland portal/EI spike

`tether-input-wayland` pins `ashpd` 0.13.13 with `remote_desktop`,
`screencast`, and `tokio`, plus `reis` 0.7.1 with `tokio`. The `screencast`
feature is required by this `ashpd` release's compiled RemoteDesktop stream
surface even when Tether does not request screen capture. The Linux binding
surface cross-compiles for `x86_64-unknown-linux-gnu`.

The probe model covers portal v2, grant state, restore-token presence, EIS
connection, active/paused/removed devices, and independently advertised
pointer/button/scroll/keyboard/text interfaces. Paused, removed, denied,
pre-v2, and disconnected states fail closed. Optional EI text is exposed only
when advertised; UTF-8 is split at character boundaries to at most 254 bytes,
with exactly one text request per frame. Normalized fixed-point scroll is
translated with independent residue into EI v120 units and reset on release.
Explicit scroll stop/cancel and stop-emulating plans are represented. The
portal's version-2 ConnectToEIS
requirement is documented by the
[XDG RemoteDesktop portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html).

No GNOME/KDE portal session, permission dialog, restore-token rotation,
handshake, device enumeration, keymap/modifier update, compositor restart, or
input injection ran. `reis` also describes itself as incomplete and subject to
change. R-009 therefore remains open, WAY-001 is not a successful target-
environment validation, and WAY-002 must not select the production adapter
until independent review and named GNOME/KDE evidence are available.

## Evidence classification

| Backend | Mapper/inert tests | Binding compile | Native probe | Live injection | Physical target validation |
| --- | --- | --- | --- | --- | --- |
| Windows | Eight tests passed on Windows 11 x64 with Rust MSVC and GNU | Passed natively with MSVC and GNU | `SendInput` export resolved | Not run | Not run |
| macOS | Passed on macOS arm64 | Passed natively | Accessibility preflight only | Not run | Not run |
| X11 | Passed on macOS host | Passed, Linux x86_64 cross-target | Not run | Not run | Not run |
| Wayland | Passed on macOS host | Passed, Linux x86_64 cross-target | Simulated negotiation only | Not run | Not run |

Exact commands and dated results belong in the milestone evidence record. This
document records boundaries, not a claim that later native acceptance gates
have passed.
