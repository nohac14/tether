# Input dispatcher

- **Task:** IC-003
- **Status:** Implemented contract
- **Authority:** `SPEC.md` sections 11, 16, 20–21, 30, and 33

The dispatcher is the sole serialized route from validated session actions to
one `InputBackend`. It owns no authentication or session epoch policy; those
remain in `session`. It accepts only platform-neutral `InputAction` values and
never exposes or logs action payloads.

## Execution and resource model

- One named worker thread exclusively owns the backend. `InputBackend` remains
  `Send`, not `Sync`, and native calls never run on an async network task.
- The default waiting bound is 256 logical actions. Configuration must be
  non-zero and cannot exceed 4,096 without review.
- The bound counts every accepted logical action even when compatible movement
  is coalesced into one native call. Coalescing therefore cannot create an
  unbounded vector of receipts or hidden queued bytes.
- Every admitted action and release barrier receives one monotonic
  `DispatchPosition`. Dropping an action receipt does not cancel accepted work.
- A state-changing action is a barrier because only an immediately adjacent
  compatible pointer delta or same-axis scroll delta can coalesce. No
  coalescing crosses any other action.
- Fixed-point sums use widened arithmetic and the domain constructors. If a sum
  would exceed `MAX_ABSOLUTE_FIXED_DELTA`, the existing item is left intact and
  the incoming delta becomes a separate queue entry. It is never wrapped,
  clamped, rejected prematurely, or allowed to leak between scroll axes.

## Emergency path

The normal queue and emergency release mailbox are distinct state. The mailbox
has one coalescing slot and is inspected before ordinary work. It remains
available when the normal queue reaches its bound.

Requesting release immediately closes normal admission and cancels every
waiting action. An already executing native call cannot be preempted safely; it
finishes before the higher-positioned release barrier runs. Concurrent release
requests share a ticket and may upgrade the pending reason by severity rather
than enqueueing unbounded cleanup requests.

Queue saturation, position exhaustion, impossible held-state transitions,
uncertain/fatal backend errors, backend panic, explicit stop, connection loss,
ownership transfer, lifecycle events, and shutdown all use this path. Session
work maps its more specific events to these closed reasons in SS-001/SS-002.

## Modes and recovery

The public modes are `Running`, `Releasing`, `Suspended`, `Faulted`, and
`Stopped`.

- A confirmed neutral release for an ordinary stop, connection loss,
  ownership transfer, or lifecycle event ends in `Suspended`.
- Only an explicit `resume` reopens a neutral suspended dispatcher. Reconnect
  and ownership code must establish a fresh session epoch before doing so.
- Saturation, an impossible transition, backend fault, backend panic, an
  internally inconsistent release report, or uncertain neutrality ends in
  `Faulted`; constructing a reviewed replacement is required.
- A separate terminal-fault latch is set as soon as any terminal reason is
  observed. It survives intermediate `Releasing` states and all later ordinary
  cleanup requests. Only shutdown may move that instance from the latched
  condition to `Stopped`; a neutral later release can never make it resumable.
- Shutdown performs priority release, joins the worker, and returns the backend
  for deterministic composition-root teardown or tests.

A release report is accepted as neutral only when `neutral` is true, there are
no failure categories, and `released == attempted`. Duplicate cleanup remains
safe and observable. `Drop` requests shutdown and joins as best effort; D-05's
uncatchable-process and failed-native-facility limitations still apply.

## Held-state invariants

The dispatcher ledger rejects duplicate downs, unmatched ups, and an atomic
click for a button that is already held before another native call. Validation
does not mutate the ledger. It preserves button and key insertion order. Its
release plan is:

1. mouse buttons in reverse press order;
2. non-modifier keys in reverse press order;
3. modifier keys in reverse press order.

The backend independently mirrors native downs that actually succeeded and its
`release_all` performs the native releases. The dispatcher intentionally does
not synthesize individual `Up` actions during cleanup: doing so could disagree
with partial native success, bypass backend-local state cleanup, or duplicate a
backend release. The two ledgers are complementary safety layers.

## Fault and concurrency behavior

- Fatal/uncertain errors close admission before their rejected receipt is made
  observable, preventing a racing submit from entering after a known fault.
- Cancellation and completion update only monotonic safe metrics; they contain
  no key, button, text, or delta payload.
- Mutex poison is recovered so the worker can enter the fail-safe path rather
  than hanging. Backend calls are individually unwind-contained. A backend
  panic schedules best-effort release and terminates the worker.
- Release tickets use a reserved shared completion slot, not an unbounded list
  of acknowledgements. Ordinary receipts are bounded by logical queue capacity.

## Automated coverage

Unit tests cover queue bounds, deterministic positions, movement/scroll
coalescing, state-change barriers, fixed-point headroom, axis separation,
saturation cleanup, concurrent release coalescing, dropped receipts, duplicate
and unmatched transitions, clicks during held-button drag, backend failures,
inconsistent release reports, terminal-fault persistence across repeated and
racing releases, shutdown from fault, backend panic, mutex poison, explicit
resume, release ordering, and a small exhaustive held-state transition model.
The mock records release-call count and applies its configured deterministic
latency to both execute and release calls.

These tests use no native input backend and do not establish platform support.
