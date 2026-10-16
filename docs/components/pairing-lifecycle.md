# Pairing and identity lifecycle contract

**Tasks:** F-07 contract prerequisite for PA-001, PA-003, and future reset coordination
**Status:** Contract accepted; coordinator and pairing behavior not implemented
**Authority:** `SPEC.md` sections 17.6, 18.1, 19.2, and 27; `storage.md`

This contract records the security ordering between control sessions, persisted
pairing credentials, and installation identity. It does not implement pairing,
authentication, session management, or LAN admission.

## Identity reset and rotation coordination

One core-owned coordinator serializes the entire operation. Neither Tauri IPC
nor a storage caller may invoke identity reset/rotation independently. After
explicit desktop confirmation, the coordinator must complete these stages in
order:

1. Close pairing and control admission so no new controller or standby session
   can be created. Keep admission closed for the remainder of the operation.
2. Revoke/terminate every active and standby control session. Request the
   dispatcher's bounded emergency release and wait for its outcome before
   proceeding. A cleanup failure leaves input disabled and is not treated as a
   successful reset.
3. Invalidate all persisted pairing credentials and remove the owned
   `paired-devices.json` entry using the reviewed storage path. Do not merely
   clear an in-memory cache. Confirm under the process-lifetime storage lock
   that the credential file is absent, as required by the current identity
   store precondition.
4. Only after stages 1–3 hold may storage create a replacement identity after
   corruption or rotate a healthy identity. The operation uses the existing
   revision checks and OS-backed entropy boundary.
5. Recreate an empty paired-device store for the new identity only after the
   identity commit succeeds. Reopening discovery, pairing, listeners, or input
   admission is a separate explicit recovery/start decision; reset itself does
   not silently re-enable them.

Every mandatory stage fails closed. A failure preserves closed admission,
invalidates in-memory session authority, and returns a safe administrative
error. It must never restore old sessions, held input, credentials, or a prior
identity into active service. An indeterminate persistent commit requires a
locked reload/reconciliation before any retry. Restart follows the same
recovery state machine rather than guessing which multi-file stage completed.

The ordinary paired-device-only reset may atomically persist an empty device
store without rotating identity. Factory identity reset is different: because
the implemented identity APIs require the paired-device file to be absent, its
coordinator must remove/invalidate that file before calling reset or rotation.
This distinction resolves the otherwise ambiguous "empty store" sequencing in
the general storage reset description.

## Verification obligations for PA-001/PA-003

Tests must inject failure and cancellation at every boundary above, including
concurrent reset requests, a newly arriving admission request, release timeout,
session termination failure, credential-store removal failure, stale identity
revision, entropy failure, indeterminate replacement, and empty-store
recreation failure. Each case must prove admission remains closed and no stale
credential authenticates. Active-session tests must also prove held input is
released before credential and identity changes.
