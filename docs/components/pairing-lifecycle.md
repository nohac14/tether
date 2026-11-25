# Pairing and identity lifecycle contract

**Tasks:** PA-001 through PA-004 and identity reset coordination
**Status:** Volatile approval, durable credential publication, HTTP routes and
browser fragment handling are implemented. Desktop IPC and WebSocket composition
remain separate gates.
**Authority:** `SPEC.md` sections 5, 17–20, 27 and 28, [network server](network-server.md)
and [storage](storage.md)

## QR submission and desktop approval

The core owns one serialized pairing state for all approved listeners. The
desktop selects the advertised hostname or an approved IP fallback, then issues
one QR for that exact browser host. The QR contains `/#pair=<token>` with 256 bits
of OS entropy encoded as exactly 64 lowercase hexadecimal characters. Issuing
a replacement retires the previous unsubmitted token. Rebind, pairing stop,
revoke-all and installation reset clear both the token and pending approvals.

The browser removes the fragment with `history.replaceState` before submitting
it once to `POST /pairing/request`. The bounded JSON body contains `token` and
`deviceName`. The name is an untrusted display label of at most 80 UTF-8 bytes,
with surrounding whitespace rejected along with control characters and bidi
overrides. Show it as plain text alongside the validated numeric peer and exact
host in the local desktop dialog. Only local desktop IPC can approve or deny.

The request must pass the network component's approved-interface, on-link,
Host/Origin, body and rate checks before it can claim the token. One accepted
claim consumes the token and creates one pending approval. The reply returns
`completion` as a separate 256-bit lowercase hex value for memory-only polling.
It is never added to a URL, logs, browser storage, or referrers. The service
retains only digests of the QR and completion values. The completion is bound to
the exact browser host and canonical source IP that submitted the QR.

The QR expires 120 seconds after issue. A pending approval and its completion
expire 120 seconds after accepted submission. The core uses monotonic time and
rejects the value at its deadline. There are at most eight pending or approved
entries globally and two per canonical peer across listeners. One completion
poll is accepted per two seconds, yielding at most 60 within the lifetime.
New QR issuance can continue while earlier spent tokens await their decisions.
Denial removes the entry. Invalid, denied, expired and already claimed
completion capabilities share the same terminal result.

## Completion and persistent credentials

`POST /pairing/complete` uses the same exact host and peer checks and accepts only
`completion` in its bounded JSON body. An unexpired pending approval returns
202. A matching approved entry can be claimed exactly once. Claiming an approval
produces metadata for the device-store coordinator. It grants no control and
cannot produce a successful HTTP reply by itself.

The coordinator reserves bounded capacity for its serialized device-store
mutation before claiming an approved entry and releases the reservation when
the poll produces no claim. PA-003 generates the persistent credential from a
separate 256-bit OS draw and stores only its SHA-256 digest, exact browser host,
and absolute expiration under the installation's owned data lock. The store's
current view must confirm the
credential remains valid at delivery. Before committing and again before cookie
publication, recheck the completion deadline, authority generation and validity
of the approval. If a durable commit has become stale, invalidate that credential
before reopening admission. Cancellation or response loss after commit cannot
replay completion or recover the plaintext from storage. The phone uses a fresh
pairing request when needed.

Successful delivery sets `tether_device` with `HttpOnly; SameSite=Strict;
Path=/control` on baseline HTTP. Set `Max-Age` to the remaining stored lifetime,
up to 2592000 seconds. Omit `Domain`, `Secure` and
secure-context name prefixes. Clearing uses the same name and path with
`Max-Age=0`. Never put the persistent credential in JSON or JavaScript. Pairing,
completion and the mobile entry use `Cache-Control: no-store` and
`Referrer-Policy: no-referrer`.

A valid, unexpired credential presented on an admitted control connection
renews the persisted 30-day inactivity deadline. At most one renewal is
written per device per minute. A successful WebSocket upgrade returns the
same host-scoped cookie with the remaining lifetime. Rejected upgrades never
return a renewed cookie. An expired or revoked credential requires re-pairing.

The cookie scope is the browser host without a port. DNS letter case is folded.
An allowed trailing-dot DNS form keeps its trailing dot in stored scope, so the
agent never merges two browser stores. Each IP literal and the advertised
hostname have distinct scopes. At `/control`, check the one exact cookie, its
scope, deadline and revocation against the current authoritative store, then
compare the fixed-size SHA-256 digest in constant time. Failure is a uniform
unauthorized result. Transport and session admission still apply independently.
Beginning a revocation closes credential verification and returns the scope and
an opaque request identifier. The coordinator uses the scope to initiate
session teardown and emergency release for the selected device or every device.
Revoke-all also invalidates outstanding pairing authority. The coordinator
commits the matching request after initiating teardown. When the targeted
commit succeeds, the store can verify unaffected credentials again.

`tether-pairing` implements the volatile portion through one `Pairing` instance
owned by the serialized coordinator. It accepts monotonic server time and
typed, transport-validated host/peer facts. On a clock regression it retires
all volatile state. `tether-config` now implements durable enrollment and
verification under the installation storage lock. `CredentialDigest` hashes
the exact cookie value once and compares fixed-size SHA-256 digests using
RustCrypto `ctutils`. The core coordinator reserves an exclusive storage turn
before claim, resamples time after durable enrollment, and checks the transport
lease before publication. A cancelled or expired response invalidates the
unpublished credential. A shared per-device revocation flag fences every active
or queued proof of that credential. Retiring a generation clears volatile
pairing and signals the session actor. Revoke-all invalidates stored proofs and
pending approvals while preserving the existing desktop suspension state.
Tests cover expiry, poll budgets, canonical peer limits, replays, concurrent
completion, denial, reset, storage-independent restart and entropy failure.
Windows MSVC also tests the actual OS entropy path. See the
[volatile pairing checkpoint](../manual-tests/2026-11-19-pairing-and-assets.md)
and the [paired-device checkpoint](../manual-tests/2026-11-19-paired-device-store.md).

The browser strips the fragment before POST, uses the provisional display label
`Phone`, and keeps only the transient completion proof while awaiting approval.
It polls every two seconds within a two-minute window, stops on page hide, and
accepts a fresh QR link in an existing tab. The page displays approval after a
successful HTTP response. The authenticated WebSocket hello supplies the
control state. [Current pairing evidence](../manual-tests/2026-11-21-windows-pairing.md)

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
