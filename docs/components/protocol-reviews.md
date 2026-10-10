# PR-001 protocol reviews

**Review date:** 2026-10-09  
**Contract reviewed:** `docs/components/protocol.md`  
**Specification baseline:** Revised and owner-approved `SPEC.md`

These are design reviews of the written contract by the primary implementation
agent under two separate review roles. They are not independent third-party
reviews, executable protocol tests, native hardware validation, or a release
security assessment.

## Security review

**Disposition:** Approved for PR-002 schema work.

Checks performed:

- Authentication remains in the bounded HTTP upgrade/session boundary, before
  hello or input admission; the schema contains no credential fallback.
- `ClientHello` cannot carry a cookie, persistent credential, pairing token, or
  self-asserted authorization.
- Host/Origin/interface/peer checks remain transport prerequisites and are not
  confused with protobuf negotiation.
- Every encoded envelope, batch, string, text chunk, metadata value, numeric
  delta, and enum crosses a stated hard or domain-validation boundary.
- Open proto3 enum integers, unknown operations, unknown state-changing
  payloads, stale epochs, and impossible sequences fail closed.
- Sequence and timestamps are not authentication, acknowledgements, or replay
  recovery. Reconnect cannot recover held state.
- Ordered validation prevents prefix execution of a malformed batch. Motion may
  be coalesced only between state barriers; release has a reserved non-starvable
  path.
- Error and close payloads cannot echo input, text, metadata, credentials,
  frames, origins, URLs, or native errors.
- The contract adds no LAN administration, filesystem, clipboard-read, shell,
  script, or arbitrary command surface.
- WebSocket compression remains disabled and private close reasons are fixed,
  bounded phrases.
- Application JavaScript heartbeats—not socket activity—are the only page
  liveness evidence, with server-monotonic expiry and release.

No requirement contradicts the trusted-private-LAN model or claims encryption.
Transport confidentiality remains open risk R-001 rather than being obscured by
the protocol.

## Native-backend review

**Disposition:** Approved for PR-002 schema work, subject to each later native
implementation and hardware gate.

Checks performed:

- Physical keys use the USB HID Keyboard/Keypad page and never browser labels,
  platform scan codes, Unicode values, or consumer-page media values.
- Windows, macOS, X11, and Wayland can each map an allowlisted HID usage through
  their isolated backend or return unsupported without changing the wire
  identity.
- Text is distinct from physical keys. The capability model preserves Windows
  UTF-16/surrogate handling, macOS Unicode event strings, X11 layout-mappable
  limits, and optional Wayland EI UTF-8 text.
- Logical application and media commands are closed semantic enums; they do not
  permit arbitrary scripts and do not make clipboard access implicit.
- Independent signed fixed-point pointer, vertical-scroll, and
  horizontal-scroll values preserve fraction without floating-point anomalies.
  X11 can accumulate each scroll axis into balanced discrete wheel buttons.
- Button down/up, atomic click, key down/up, and release-all provide the state
  distinctions needed by every backend and the server held-state ledger.
- Capability revisions allow permission, compositor, device, and backend
  changes to disable actions immediately instead of faking success.
- Ownership transfer and release ordering leave the backend neutral on failure;
  no backend is asked to reconstruct held state after reconnect/resume.
- The protocol does not assume identical OS acceleration, wheel units, text
  acceptance, media support, or elevated-application access.

This approval establishes representability, not native correctness. WIN-003,
MAC-003, X11-003, and WAY-004 remain mandatory real-platform gates.
