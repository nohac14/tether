# Input backend contract

**Task:** IC-001  
**Status:** Accepted implementation contract  
**Authority:** `SPEC.md`, especially sections 11–16 and 20–26

`input-core` owns platform-neutral semantic input, capability/status types, the
serialized dispatcher, held-state ledger, fixed-point normalization hooks, and
the mock backend. Each `input-*` crate implements the one `InputBackend`
contract. Protocol, network, session, and Tauri code never call native input APIs
or pass wire/generated types to a backend.

## Core invariants

- Only the authenticated ordered dispatcher invokes an `InputBackend`.
- One backend instance is selected for the actual desktop session; X11 is never
  used as a hidden fallback from a Wayland session.
- Backend calls are serialized. An implementation is `Send` but need not be
  `Sync`; no caller invokes it concurrently or from an async network task.
- Every semantic down/up, click, movement, scroll, text, logical, media, and
  release action has one defined position in dispatcher order.
- Malformed wire values and unsupported capabilities are rejected before the
  backend. A backend still validates platform-specific preconditions and never
  fakes success.
- Successful held-state changes are mirrored in both the dispatcher ledger and
  backend release state. Reconnect, resume, or ownership transfer never restores
  them.
- `release_all` is idempotent, best effort across every tracked item, and has a
  reserved control path that cannot wait behind the normal action queue.

## Semantic domain values

IC-002 defines owned domain values independent of protobuf and OS APIs.

### Motion and scrolling

`PointerDelta` and `ScrollDelta` contain finite signed fixed-point values at
1/1024 normalized-unit resolution. The protocol validator enforces version hard
limits before conversion. Input-core applies configured sensitivity,
acceleration, smoothing if enabled, clamping, and fractional accumulation in a
deterministic step. Backends translate normalized output to native units.

Scroll always names exactly one axis. Horizontal scroll is never encoded as
Shift plus vertical scroll. Backends with discrete wheels maintain independent
axis accumulators and emit balanced press/release pairs for every generated
wheel button.

### Buttons and physical keys

`MouseButton` is the closed left/right/middle/back/forward set. `ButtonAction`
is down, up, or atomic click with count one or two. Explicit transitions support
drag/hold; an atomic click must finish neutral even if the native API partially
fails.

`PhysicalKey` is an allowlisted USB HID Keyboard/Keypad-page usage. Modifiers are
normal usages and held state, not a separate implicit bitmask. `KeyAction` is
down or up. Platform scan codes, browser labels, Unicode scalars, XKB keysyms,
and native event objects cannot enter the domain type.

### Text, logical, and media actions

`SensitiveText` contains bounded valid UTF-8 and has an unconditionally redacted
`Debug` representation. Text insertion remains distinct from physical keys.
The active backend returns capability/unsupported information for Windows
Unicode packets, macOS event strings, X11 layout mapping, or optional Wayland EI
UTF-8 rather than falling through to clipboard or global keymap mutation.

Logical application commands and media commands use the closed protocol
semantics. They map to reviewed native chords/events. No action carries a shell
command, script, path, arbitrary key label, or clipboard content.

## Backend interface

The initial synchronous trait semantics are:

```rust
trait InputBackend: Send {
    fn status(&self) -> BackendStatus;
    fn refresh_status(&mut self, reason: StatusRefreshReason) -> BackendStatus;
    fn execute(&mut self, action: &InputAction) -> Result<(), BackendError>;
    fn release_all(&mut self) -> ReleaseReport;
}
```

Names may be refined in IC-002 without changing the behavior. `execute` accepts
only semantic input actions; ownership/lifecycle operations are dispatcher or
session commands that call `release_all` as required. Implementations maintain a
private mirror of native downs that actually succeeded. They add an item only
after a successful native down and remove it only after a successful native up
or a release report that confirms release.

`status` is a cached non-blocking snapshot. `refresh_status` may perform bounded
platform checks on the dispatcher execution context and advances its revision
when anything changes. It never injects a visible canary event.

## Capability and status snapshot

`BackendStatus` contains:

- monotonically increasing local revision;
- backend kind and initialized/available state;
- permission state: not required, granted, denied, prompt required, unavailable,
  or transiently unknown;
- relative pointer availability;
- supported mouse buttons and atomic click;
- vertical and horizontal scroll independently;
- physical-key availability;
- text modes: active-layout-mappable, native Unicode best effort, EI UTF-8;
- supported logical and media commands;
- a closed safe troubleshooting/error category.

Capability sets are conservative current facts, not intended product support.
An absent capability disables the UI and causes pre-dispatch rejection. Dynamic
loss—macOS Accessibility revocation, Wayland EI pause/removal, X11 extension or
keymap loss, Windows desktop/session transition—stops normal admission and
forces release before the new snapshot is published.

The server capability revision is composed from backend and service revisions;
the backend never trusts a client capability claim.

## Error categories

`BackendError` is a closed category plus safe operation class and retry scope:

- `Unsupported`
- `PermissionRequired`
- `PermissionDenied`
- `TemporarilyUnavailable`
- `InvalidPlatformState`
- `NativeCallFailed`
- `PartialStateChange`
- `Fatal`

It never stores typed text, HID usage, button identity, native event bytes,
target application, clipboard data, native localized error text, or raw OS error
message. A platform numeric error code may be retained in a redacted internal
wrapper for local branching but is not emitted to ordinary logs/diagnostics.

Unsupported and precondition errors reject only the action when no held/native
state changed. `PartialStateChange`, permission loss, fatal errors, and any
uncertain down/up result immediately stop normal admission and invoke emergency
release. The dispatcher reports safe capability/fault state after cleanup.

## Held-state and release semantics

The dispatcher ledger records successful physical-key and button downs in press
order. Duplicate down and unmatched up are invalid session actions rather than
extra native calls. Atomic clicks and text/media/logical actions do not remain
held.

The backend mirror is a safety backstop for native calls it observed. On
`release_all`, it attempts every mirrored held item even if one release fails:

1. stop accepting ordinary execute calls;
2. release mouse buttons, preventing continued drag;
3. release non-modifier keys in reverse successful-press order;
4. release modifier keys in reverse successful-press order;
5. cancel backend-local scroll/momentum/text composition state;
6. report each safe category of failure without payload identity;
7. remain in a non-admitting fault state if neutrality is uncertain.

Platform constraints may require a safer order, but deviations need a backend
contract note and tests showing no sticky state. The dispatcher invokes release
again safely on duplicate triggers. A successful report guarantees that the
backend mirror is empty; a partial report never claims neutrality and causes
control/session closure.

Observable release triggers include disconnect/close/error, application
heartbeat timeout, client departure, queue/rate saturation, protocol failure,
credential revocation, user Stop Control, ownership transfer, listener rebind,
suspend/session lock/logout, permission/capability loss, backend fault, panic
unwind where cleanup still runs, and graceful quit. SS-001 owns the exhaustive
state-transition table. Uncatchable termination remains D-05's explicit limit.

## Dispatch and threading rules

The dispatcher has a bounded ordinary queue and a separately reserved control
mailbox/flag for release and stop. It assigns one monotonically increasing
dispatch position to accepted work. A barrier flushes prior coalesced motion and
scroll before any state-changing action. Coalescing is allowed only for
compatible deltas between barriers.

If the ordinary queue saturates, admission closes for that session, discardable
motion is cleared, and release executes through the reserved path. Releases and
ups are never silently dropped or queued behind unlimited work. Cancellation of
the submitting async request does not cancel an action already accepted by the
dispatcher.

Blocking native calls run on the dedicated dispatcher thread or an explicitly
documented platform execution context. Callbacks from permission/session APIs
enqueue bounded status/control messages; they do not mutate the backend
concurrently. Shutdown joins the context after release or reports an uncertain
neutrality failure.

## Platform obligations

- Windows maps HID usage to scan codes, sends well-formed UTF-16 units including
  surrogate pairs for supplementary text, reports UIPI limitations, and remains
  unelevated.
- macOS preflights Accessibility without a visible synthetic event, treats
  Local Network privacy outside the input backend, and preserves signed identity
  validation as a package gate.
- X11 verifies XTEST/XKB, tracks active layout/group changes, rejects unmappable
  text, uses independent fractional scroll accumulators and balanced Buttons
  4–7, and never changes the clipboard or global keymap.
- Wayland validates the selected Rust EI adapter before commitment, treats
  portal/EI devices as dynamic, uses UTF-8 text only when advertised, and
  releases/pauses on device removal or compositor lifecycle loss.

Each implementation documents native error mapping, execution-thread
requirements, capability refresh, and the exact hardware/manual tests it passed.

Canonical HID ranges, conventional logical-command chord plans, last-moment
capability preflight, and the inert shared suite are defined in
[`backend-conformance.md`](backend-conformance.md). A mapping plan does not make
a capability available; each backend advertises only its validated subset.

## Mock backend and required tests

`MockInputBackend` records semantic operation classes and deterministic safe
identifiers for assertions in memory only; its diagnostics remain redacted. It
supports injected per-call errors, partial failures, permission/capability
changes, delays through fake time, and release failures without touching the OS.

IC-002 tests domain bounds, fixed-point types, redacted `Debug`, capability
defaults, status revisions, and mock error injection. IC-003 model/race tests
cover ordering, barriers, coalescing, saturation, cancellation, duplicate state,
every release trigger, partial native failure, and final neutral state.
NB-001 additionally requires every native mapper to pass the shared inert
conformance suite before its guarded native validation can establish support.
