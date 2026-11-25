# Transactional session admission

**Task:** SS-002, supporting admission component

**Status:** Implemented and wired into the session actor. Production transport
and desktop composition remain downstream.

**Authority:** [session safety](session-safety.md),
[protocol validation](protocol-validation.md), `SPEC.md` sections 11 and 15–16

## Boundary

`tether_session::admission::AdmissionCursor` owns connection-local sequence,
client timestamp, heartbeat identifier, current epoch and the expected held
state after earlier admitted work. It has no transport, dispatcher, credential
verifier, OS input handle or authority to grant control. It reuses PR-004 for
complete bounded decoding and semantic validation; it is not another codec.

`AdmissionFacts` must come from the serialized actor and live verifier/core,
never from wire fields. Positive authorization revision, current ownership,
epoch and backend status are trusted caller preconditions. Comparing these
facts detects stale preparation; it does not authenticate a cookie, prove that
an external snapshot is current or replace final native preflight. There is no
default-allow authorizer or production credential stub.

## Prepare, reserve and commit

1. `prepare` validates the whole envelope against the current ordered projection.
   It returns an owned, non-Clone preparation with private message/state fields.
   Dropping or rejecting it changes nothing and never executes input.
2. The actor reserves its bounded local queue capacity and byte/receipt budget.
   Preparation exposes expanded logical-operation count, retained text size,
   and a barrier kind. The actor counts fixed action and receipt records and
   reserves the combined budget before commit.
3. Under the same serialized authority transition, `commit` checks cursor identity,
   cursor revision and the complete captured facts again. Backend equality is
   checked even if a buggy provider retained its status revision. Cross-cursor,
   obsolete or changed-context preparation is rejected with no state mutation.
4. Only successful commit returns the validated message for the reserved local
   work slot. Never submit native work before this succeeds. Discard the reserved
   slot if commit fails; failure/cancellation after accepted work still requires
   the actor's reserved cleanup path, not silent loss or sequence reuse.

Committing changes expected admission state, not confirmed native state. A queued
down therefore permits the next queued up before any native receipt completes.
The dispatcher/backend keep their independent actual-execution ledgers. A failed
or cancelled ancestor invalidates dependent queued work; actor integration must
neutralize rather than pretend that projected input executed successfully.

## Barriers, counters and privacy

Ownership requests, relinquishment and release-all terminate their batch and
block further batches until resolved. Valid heartbeats/leaving can still advance
connection counters while a barrier is pending. Only a request barrier can be
resolved without transfer, for denial or an idempotent self request; that never
grants control. Release/relinquish cannot be cleared through that method.

`advance_epoch` accepts only a strictly newer actor-authorized epoch and clears
the projected held state/barrier. The actor must first establish its required
neutrality and grant preconditions. This method does not acknowledge release,
resume a dispatcher, clear terminal faults or restore held input. Direction-local
sequence, client time and heartbeat identifiers remain monotonic across epoch
changes. A new negotiated connection instead starts with empty state/sequence one.

Sequence and revision arithmetic never wraps. At sequence exhaustion, the actor
must close/release rather than reuse the connection. Preparations are bound to
a unique checked process-local cursor number; it is neither a wire identifier,
cryptographic nonce, installation identity nor credential. Production installation
and pairing entropy requirements remain unchanged.

Cursor, preparation, facts and snapshot diagnostics are unconditionally redacted.
Errors contain only closed safe categories. All input still passes through the
ordered dispatcher after actual authentication and ownership checks; this module
does not enable remote input or a live LAN service.

## Verification scope

Eleven unit tests cover whole-batch rollback, dropped/obsolete/cross-cursor
preparations, authorization/ownership/epoch/backend changes, queued button/key/
modifier transitions, held-click rejection, barriers and request denial,
monotonic time/heartbeat state, counter exhaustion, concurrent private instance
allocation, redaction and a 10,000-step seeded independent button-state model.
Seeded randomness is dev-only. These tests make no native calls.

The actor owns heartbeat deadlines, revocation signals and release receipts.
The [admission checkpoint](../manual-tests/2026-11-19-session-admission.md)
records standalone cursor coverage. The
[ownership and heartbeat checkpoint](../manual-tests/2026-11-20-session-control.md)
records the mock integration.
