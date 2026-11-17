# Session safety and exclusive ownership

**Task:** SS-001

**Status:** Implementation contract complete; SS-002/003/004 runtime and tests
not implemented

**Authority:** `SPEC.md` sections 11, 15–16, 17.6, 19–21 and 33;
[protocol](protocol.md), [wire validation](protocol-validation.md),
[dispatcher](input-dispatcher.md), [network](network-server.md) and
[identity reset](pairing-lifecycle.md)

## Trust and execution boundary

One Rust session actor serializes control authority for one dispatcher/backend.
Only it submits native-domain actions. Transport tasks, pairing, lifecycle
adapters, UI and Tauri request typed events; none inject input or grant ownership.
The dispatcher worker remains the backend's exclusive owner. Session handling
never waits synchronously for a native call, socket write, close handshake,
pairing decision or storage operation on its safety/event-loop thread.

The core drives timers and cleanup even without inbound traffic. The future
network bridge delivers authenticated device/scope facts, an owned connection
identity and current transport generation. Nothing in a protobuf payload can
assert authentication, authority or a trusted backend capability. Registration
is gated on actual PA/NW verification; do not add a default-allow authentication
stub to implement SS-002 tests or enable an unfinished server.

Connection identity, persistent device identity, transport generation, session
epoch, envelope sequence, heartbeat identifier and dispatcher position are
distinct types/counters. No equality or reuse implies interchangeability.
All monotonically increasing counters use checked arithmetic; exhaustion closes
admission and starts cleanup, never wraps. A stale connection/receipt cannot
act on another connection which reused a table slot.

## States and invariants

Actor availability is Ready, Releasing, Suspended, Faulted or Stopped. These are
not dispatcher modes or wire control states. Ready still requires backend
health/permission/capability checks before every admission. Releasing has no
input-admitting owner. Faulted is terminal for that actor instance; shutdown may
finish in Stopped but cannot make it resumable. Neither a successful later
release nor an ordinary recovery event clears the dispatcher fault latch.

Per-connection states:

| State | Meaning and permitted work |
| --- | --- |
| Unauthenticated | Pre-upgrade rejection/model boundary only; no authenticated session allocation, hello, ownership or dispatcher access |
| Negotiating | Verified upgrade, bounded hello deadline; only one valid client hello, no input/control request/heartbeat until server hello completes |
| Standby | Negotiated and authenticated; heartbeat, leaving and request-control only; no native input or release of another owner's input |
| Requesting | At most one outstanding explicit ownership request per connection; no input while the release/grant barrier is pending |
| Controlling | Exactly the current owner/epoch may admit supported input; heartbeat/leaving/release/relinquish remain available |
| Suspended | No input authority during lifecycle, permission or explicit safety suspension; no deferred grant or held-state replay |
| Revoking | Admission closed, queued authority invalidated and reserved cleanup outstanding; never eligible for input or grant |
| Closed | Terminal for this connection identity; discard late traffic/completions without affecting a newer owner |

At most one connection is Controlling. A pending transfer target is not an
owner. Wire states project this model: Standby is used when another owner exists;
NoController when none exists; Controlling only for the owner; Suspended for
global/lifecycle suspension. A noncontrolling initial connection can echo epoch
zero under NoController/Suspended; wire Standby/Controlling require nonzero epochs.
Negotiating/Requesting/Revoking are internal phases, not new wire enum values.

## Total transition policy

| From / event | Result and required guard |
| --- | --- |
| Pre-upgrade / failed authentication, peer or Host/Origin check | Reject transport; no session, no release of an unrelated owner |
| Verified upgrade / register | Negotiating only if current generation/device/quota checks pass atomically |
| Negotiating / valid hello and bounded server-hello acceptance | Standby; set negotiated limits, fresh connection-local counters and initial liveness deadline; do not grant control implicitly |
| Negotiating / duplicate hello, runtime payload, timeout or output failure | Closed; no native authority existed |
| Standby / request control while Ready | Requesting; select one transfer target in actor event order; do not silently replace a pending target or maintain an unbounded request queue |
| Controlling / self request | Idempotent ownership status response, not another grant or permission to replay input |
| Requesting / confirmed neutral barrier and still-eligible target | Advance epoch, explicitly resume neutral nonfaulted dispatcher, publish new owner/state, then admit that owner's new-epoch work |
| Requesting / denial, target loss, timeout, revocation, suspend or health failure | Cancel pending target; no grant from a stale request; retain closed admission until mandatory cleanup resolves |
| Controlling / valid input batch | Whole-batch validation, bounded ordered admission and receipt tracking; no successful prefix of an invalid envelope |
| Any negotiated live connection / valid heartbeat | Validate sequence/epoch and increasing identifier, refresh only page-runtime liveness and echo a bounded pong; no grant, no extension of release/transfer deadlines and no revival after expiry |
| Controlling / explicit release-all | Stop admission and neutralize; a fresh epoch may regrant this same live, eligible owner only after the barrier succeeds; client must discard old pending work and wait for new control state |
| Controlling / relinquish | Revoke, release, advance epoch, then Standby/NoController; no automatic regrant |
| Any authenticated state / disconnect, leaving, expiry or protocol fault | Close that connection; owner loss invokes scoped release, a nonowner cannot release someone else's input |
| Any active state / global suspension, stop, reset or shutdown | Invalidate pending grants/queued authority and invoke global cleanup; retain or close transports according to the triggering policy below |
| Suspended / explicit successful core recovery | Standby, empty held state, current epoch; a fresh ownership request is required |
| Any state / internal invariant failure, unknown internal event or uncertain cleanup | Faulted with admission closed and best-effort reserved cleanup; never guess a valid transition |
| Closed / any late connection event | Ignore/acknowledge teardown idempotently; no authority change or release of a newer owner |

Every other external state/event pair is a safe rejection, not an implicit
transition. Invalid state-changing traffic from the current owner closes and
neutralizes that owner; invalid nonowner traffic only closes that connection.
An internal contradiction is different from a malicious client's rejected
transition: the former faults the actor, while the latter need not permanently
fault a healthy dispatcher. A new grant never restores keys/buttons or resumes
old buffered gestures. Mandatory status output failure is a connection failure,
not permission to continue controlling with an unaware client.

## Epoch barrier and queued-state model

Ownership transfer/loss uses one non-interleavable logical transition:

1. Remove old-owner admission authority before any await; invalidate its queued
   envelopes and cancel all pending grants incompatible with the stop reason.
2. Request the reserved dispatcher release, which cancels waiting actions.
   Already executing native work must finish before the release barrier.
3. Await confirmed neutral completion without blocking safety/timer handling.
   A report is neutral only with `neutral`, no failures and equal attempted/
   released counts; also require a live, nonfaulted, neutral dispatcher before
   any resume. A neutral report alone never clears its terminal-fault latch.
4. Advance the ownership epoch without reuse. Recheck target authentication,
   transport generation, liveness, backend status and absence of a newer stop.
5. If a grant remains authorized, explicitly resume the dispatcher, assign that
   owner/epoch and enqueue bounded control-state output before admitting its
   new-epoch actions. Otherwise publish no controller or suspension.

If any step fails, nobody gains ownership. Unknown physical neutrality is an
error, never reported as successful cleanup. No timeout/late completion can
resurrect the old owner, target or queued actions. Repeated release-all requests
coalesce; leaving/revocation/global stop overrides any same-owner regrant.

Maintain three distinct views:

- The dispatcher/native ledger tracks actual successful input and partial native
  outcomes. Session confirmed state changes only in receipt/position order.
- A private admission projection describes the expected held state after all
  earlier accepted ordered work. PR-004 validates the next whole batch against
  this projection, not an old confirmed ledger while a down is still queued.
  Commit the projection only after the whole envelope is validated and accepted
  into bounded actor work; rejected batches leave it unchanged.
- Connection UI/status is advisory; it cannot override either ledger or grant.

Before dispatcher submission recheck authority/epoch, negotiated policy and
current capabilities. Revalidation uses the ordered projection at that submission
position. On failed/cancelled receipt, lost generation or capability change,
invalidate all dependent queued work and release; never repair a speculative
projection by pretending failed native input succeeded. Do not submit up based
solely on stale completed state, or issue an unintended duplicate down. Preserve
ordering across batches, not only within one batch.

## Release trigger table

The table scopes cleanup to the current owner or to the whole service. A
nonowner event may close its own session but cannot touch a different owner.
The reason column maps to existing `EmergencyReason`; a dispatcher-established
terminal fault always remains terminal regardless of a later weaker reason.

| Observable trigger | Scope / dispatcher reason | Post-cleanup disposition |
| --- | --- | --- |
| WS EOF, close, read/write error, client user disconnect, navigation/page-hide/leaving, task cancellation | Affected owner / ConnectionLost | Close connection, invalidate epoch; others remain standby |
| Valid application heartbeat timeout or event-loop suspension detected by timeout | Affected owner / ConnectionLost | Close; no input replay on reconnect |
| Credential expiry, revoke, revoke-all, device removal | Affected credential's owner / ConnectionLost; global for revoke-all | Close invalidated sessions before any subsequent grant |
| Unknown/malformed/oversize/unauthorized input, stale sequence/epoch, invalid transition rejected by PR-004 | Affected owner / ConnectionLost | Stop, close and neutralize; never dispatch an otherwise valid prefix |
| Actor/network input-rate, retained-byte, normal-queue or mandatory-output overflow | Affected owner / ConnectionLost | Close and neutralize through reserved path, never discard just an up |
| Dispatcher queue saturation, dispatch-position exhaustion or dispatcher ledger contradiction | Global / QueueSaturated, PositionExhausted or InvalidTransition | Terminal dispatcher/actor fault, no resume |
| Explicit owner ReleaseAll | Current owner / ExplicitStop | Neutral barrier, cleared state and fresh epoch before any permitted same-owner continuation |
| Relinquish or another device's accepted takeover | Current owner / OwnershipTransfer | Neutral before assignment; no owner on failed transfer |
| Desktop stop-all/input-disable | Global / ExplicitStop | Suspended; explicit local recovery/start only |
| OS suspend/session lock, lost native permission, backend availability/capability loss | Global / Lifecycle | No grant while unsafe; fresh request after explicit core recovery |
| Interface/address loss, listener rebind/stop, discovery authority invalidation | Global / Lifecycle | Quiesce obsolete sessions before listener/authority publication |
| Fatal/uncertain native result, inconsistent release, worker death or failed release | Global / BackendFault | Terminal fault; keep best-effort cleanup pending without claiming neutrality |
| Actor panic/unwind, worker panic, poisoned state or lost safety channel | Global / WorkerPanic or BackendFault | Close admission, best-effort independent cleanup; terminal fault |
| Identity reset/rotation or factory reset | Global / Lifecycle | All sessions terminated and neutral before credential invalidation/identity commit; fail closed at any mandatory stage |
| Orderly application quit, shutdown/drop while cleanup remains possible | Global / Shutdown | Stop admission, release, drain bounded acknowledgements and join worker at composition-root teardown |
| Fresh backend startup or reconnect before first possible grant | Global / ExplicitStop | Confirm neutral startup; do not synthesize old held state or clear a terminal fault |

Raw transport Ping/Pong, typed input or arbitrary TCP activity never refreshes
page-runtime liveness. Heartbeat and hello deadlines use injected server
monotonic time; no comparisons against client wall time. SS-003 starts with
1,000/4,000 ms interval/timeout under the protocol's hard policy and tests
deadline equality, delayed scheduling, clock overflow and open-socket inactivity.
An expired deadline wins over later processing of a queued heartbeat; receipt
timestamps are server-monotonic observations, never browser assertions. Backend
completion and ordinary input traffic do not extend the deadline.
Normal connection closure is not global network quiescence.

## Priority, cancellation and resource bounds

Use a reserved, independently polled, coalescing safety mailbox, not a normal
bounded-input send that may be full. Global stop dominates scoped release and
pending grants. Bound scoped reasons by the eight registered WS sessions;
retain one cleanup generation/ticket, not a list per retry. Dropping a request
handle never cancels accepted cleanup or extends its deadline. A stale scoped
event is matched by connection identity and epoch, never a recycled slot.

Preserve NW's one-decode-per-connection, eight validated envelopes/128 KiB per
connection, eight registered WS sessions, bounded rates/outbound queues and
hello/IO deadlines. Account owned strings, action expansion and retained receipts,
not just encoded bytes. Bound pending logical dispatcher actions/receipts to
256 in total or a lower configured actor limit. Do not wait for capacity by
blocking the actor, spawn one waiter per action, retain unbounded raw graphs,
or reset quotas through reconnects/aliases.

Cancellation/release events and deadline checks run before another normal
admission. Each iteration has a finite work budget; a flood cannot indefinitely
postpone safety checks. A release/leaving operation is fully authenticated,
structurally and semantically validated before it obtains priority; if it cannot
be admitted safely, close its owner and request release instead. It does not
gain input authority, bypass byte/frame bounds or advance past unvalidated
earlier sequences. Key/button ups remain ordered normal transitions; overflow
invokes release-all rather than dropping them.

The NW-003 bridge's Neutral acknowledgement requires both obsolete-session
teardown and confirmed dispatcher neutrality. Transport generation is not a
session epoch, and a missing/unpublished generation does not skip global cleanup.
Drive its existing two-second quiescence deadline independently. Timeout keeps
input disabled, reports failure, continues best effort and forbids late regrant;
it is not a guarantee of preempting a blocked OS call. Error delivery, outbound
IO and close handshakes never delay release. Shutdown's worker join is likewise
not a promise that an unresponsive native facility can be forcibly interrupted.

No actor or RAII policy can execute after uncatchable process termination,
power loss or OS failure. Preserve D-05's exact guarantee: initiate idempotent
cleanup on every observable/graceful path, validate the outcome, and honestly
report unavailable native cleanup rather than fabricating neutral state.

## SS-002/003/004 verification gates

Implement a deterministic reducer/model plus fake-clock/mock-dispatcher tests.
Enumerate all state/event pairs and assert:

1. Zero or one owner; no input in any noncontrolling or releasing state.
2. An accepted new owner implies completed neutral release and a fresh epoch.
3. Every owner-loss trigger reaches reserved cleanup even with full normal/
   outbound queues, stalled peers, dropped handles and task cancellation.
4. Bad nonowner traffic cannot release another owner; late old receipts,
   reconnects, transport generations and cleanup completions cannot regrant.
5. Queued down then up across batches works before receipts finish; rejected
   batches cannot mutate projection; cancellation/rejection invalidates dependent
   work. Test click-on-held, modifiers, drag, both scroll axes and partial failure.
6. Repeated/concurrent release, same-owner release versus takeover, revoke versus
   grant, lock/resume, heartbeat equality/expiry, counter exhaustion and reset
   races never replay held state or clear terminal faults.
7. Each mandatory failure stage leaves no input authority; each confirmed
   successful cleanup ends with both mock and dispatcher neutral.
8. Bounds, accounted bytes, safe errors/diagnostics and explicit backend capability
   rejection hold under deterministic long traces and adversarial scheduling.

SS-001 completes this written model/trigger contract only. These runtime tests,
physical device/native checks, authentication and real LAN admission have not run
and are not marked complete by documentation review.
