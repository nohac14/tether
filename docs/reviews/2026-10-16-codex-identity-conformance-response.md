# Codex response to the ST-003/NB-001 identity-conformance audit

**Source review:** `2026-10-12-gemini-identity-conformance-audit.md` read in full
**Disposition date:** 2026-10-16
**Milestone:** F-07/F-08 plus Wave 8 inert native-backend spikes

## Finding disposition

### F-07 — accepted

`docs/components/pairing-lifecycle.md` now makes the future coordinator order
enforceable: close new control admission; terminate active/standby sessions and
complete bounded release; invalidate and remove persisted pairing credentials;
confirm `paired-devices.json` is absent; only then reset/rotate identity; and
recreate an empty device store/reopen services only after successful identity
commit and explicit recovery. Every mandatory failure leaves admission closed.

The general storage text was corrected to match the implemented identity API's
file-absence precondition. This is contract work only. Pairing, authentication,
session management, and reset coordination were not implemented prematurely.

### F-08 — accepted

Each Wave 8 mapper separates interface-level keyboard availability from a
single unmappable HID usage. The general capability remains available for a
partial table; a missing usage returns `BackendErrorCategory::Unsupported`
before the inert/native sink and creates no held-state entry. All four platform
test suites prove this behavior, while the shared conformance suite walks the
entire allowlist and accepts only balanced execution or explicit unsupported.

No Gemini recommendation was rejected in this audit response. The conditional
GO was treated as permission for bounded spike implementation, not native
validation or approval to start later security-sensitive components.

## Implementation summary

- Added the reusable feature-gated `InertConformanceMapper`, event sink, and
  `InputBackend` wrapper with a backend-side held ledger, reverse release,
  redacted partial-batch error handling, and an uncertain-native-state latch.
- WIN-001: Windows event-plan translation, UTF-16 surrogate pairs, partial scan
  mappings, media/wheels/buttons, cross-target binding check, conformance and
  failure/release tests.
- MAC-001: selected `objc2-core-graphics`, added translation plans, compiled the
  event API surface, ran the non-prompting Accessibility preflight, and passed
  conformance/failure/release tests on macOS arm64.
- X11-001: XTEST/XKB snapshot model, partial active-layout mappings, whole-text
  preflight, independent fractional discrete scroll, cross-target binding
  check, and conformance/failure/release tests.
- WAY-001: portal/EI negotiation and lifecycle model, independent capabilities,
  framed event plans, optional UTF-8 boundary-safe text, cleanup plans,
  cross-target `ashpd`/`reis` compatibility check, and conformance/failure tests.

The exact evidence is in
`docs/manual-tests/2026-10-16-wave8-spike.md`. MAC-001 satisfies its spike gate.
WIN-001, X11-001, and WAY-001 retain blocked status because their unavailable
native environments prevent the remaining observed-runtime criteria; this is
not papered over by cross-compilation.

## Dependency review

Direct versions are `windows` 0.62.2, `objc2-core-graphics` 0.3.2, `x11rb`
0.14.0, `ashpd` 0.13.13, and `reis` 0.7.1 from crates.io. Features are narrowed
to the platform APIs used. Linux cross-compilation found that `ashpd`'s
RemoteDesktop surface requires its `screencast` feature; that concrete compile
failure was fixed rather than suppressed. `Cargo.lock` and the complete
platform graph inventory were updated. No Git dependency, arbitrary installer,
TLS/checksum bypass, or live input library shortcut was introduced.

`cargo-deny` remains unavailable, so advisory/license/source policy execution
is a recorded gap, not a pass. Locked metadata, source provenance, licenses,
duplicate versions, and local unsafe blocks were reviewed manually. R-018 and
BV-001 are unchanged.

## Independent-review questions

1. Confirm the inert harness's partial-batch uncertainty and cleanup semantics
   remain compatible with the terminal dispatcher fault/release invariants.
2. Confirm the partial HID capability model satisfies F-08 without overclaiming
   support for a specific key.
3. Review the Windows scan/extended map, macOS key/media event plans, X11
   keycode/text assumptions, and Wayland evdev/EI frame plans before production
   backends use them.
4. Decide only after GNOME/KDE evidence whether `reis` is acceptable for
   WAY-002; its own documentation still calls the library incomplete.
5. Keep native runtime, physical-device, signed-package, cargo-deny, frontend,
   Android, authentication, pairing, session, and network claims explicitly
   unperformed.

Per the requested boundary, no authentication, pairing, session management,
network input admission, or production live native backend work begins before
the next independent Gemini milestone review.
