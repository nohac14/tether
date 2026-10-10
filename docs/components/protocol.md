# Control protocol contract

**Task:** PR-001  
**Status:** Accepted implementation contract; reviews recorded in `protocol-reviews.md`  
**Authority:** `SPEC.md`, especially sections 11, 13–16, 19–21, 26, and 33

This document fixes the application-level contract that PR-002 will express in
Protocol Buffers. It does not authorize a second transport, remote
administration, clipboard mutation, shell commands, or unauthenticated input.

## Boundary and invariants

- Each WebSocket binary message contains exactly one protobuf `Envelope`.
- Only the `/control` WebSocket carries this protocol. Desktop administration
  remains narrow Tauri IPC and is never added to this schema.
- The authenticated session actor is the only consumer of validated client
  operations. Wire messages never reach an `InputBackend` directly.
- All accepted input operations enter one ordered dispatcher. Motion and scroll
  preceding a state-changing operation are flushed before it and may be earlier
  operations in the same ordered batch.
- Unknown, malformed, unauthorized, stale-epoch, impossible-sequence,
  over-limit, or unsupported operations fail closed. Any path that could have
  held input invokes the reserved release path before or while closing.
- No diagnostic representation may expose text, physical-key identity, control
  frames, cookies, credentials, pairing tokens, or user-supplied metadata.

## Transport binding

The browser derives the URL from the loaded page: map `http:` to `ws:` and
`https:` to `wss:`, retain `window.location.host` exactly enough to preserve its
active port and bracketed IPv6 authority, and replace only the path with
`/control`. A hard-coded host, default port, or alternate WebSocket authority is
not permitted.

The upgrade requires the exact WebSocket subprotocol `tether.control.v1`.
Per-message compression is disabled. A future incompatible major uses another
subprotocol token and schema package rather than guessing from payload bytes.
The HTTP upgrade layer validates the host-only cookie, peer/interface policy,
`Host`, `Origin`, path, connection quota, and frame limit before registering a
protocol session.

## Envelope and direction

PR-002 must define separate client and server envelope payload variants so a
client cannot encode a server-only command and vice versa. Field names may be
refined during schema linting, but the following semantics are fixed.

Every envelope carries:

- selected protocol major and minor after negotiation;
- a direction-local `uint64` sequence number;
- the current server-issued session epoch when one exists;
- for client runtime messages, a monotonic client timestamp in integer
  microseconds from an arbitrary client-local origin;
- exactly one payload selected with `oneof`.

`ClientHello` and `ServerHello` use sequence zero and omit an epoch. Every later
envelope in each direction starts at sequence one and increments by exactly one.
WebSocket already preserves delivery order, so a gap, duplicate, regression, or
wrap is a protocol fault rather than a retransmission request. Sequence numbers
are not acknowledgements and never restore state after reconnect.

Client timestamps may be equal but must not decrease within a connection. They
support local sampling, latency, and anomaly checks only. The server never
compares them to its wall or monotonic clock for authentication, expiry,
ordering, or ownership.

### Client payload families

- `ClientHello`
- `ClientOperationBatch`
- `HeartbeatPing`
- `ClientLeaving`

### Server payload families

- `ServerHello`
- `HeartbeatPong`
- `CapabilitySnapshot`
- `ControlState`
- `ProtocolError`

The close frame remains WebSocket control data and is not wrapped in protobuf.
A safe protobuf error may precede a close when doing so does not delay input
cleanup.

## Hello and version negotiation

`ClientHello` is the first binary message and declares:

- protocol major `1`;
- minimum and maximum supported minor versions;
- a bounded application build identifier;
- a bounded, display-only client name marked as untrusted;
- client limits needed to select a safe batch size;
- optional client feature bits whose semantics are defined in the schema.

It never contains a cookie, persistent device credential, pairing token, or
authorization claim. Authentication belongs to the upgrade/session layer.

The server chooses the highest mutually supported minor and returns it in
`ServerHello`, together with the connection identifier, heartbeat policy,
effective hard limits, current capability revision, and initial control state.
The server does not select a major other than the subprotocol major. With no
minor overlap it sends `INCOMPATIBLE_VERSION` when safe and closes without
creating input ownership.

Hello must complete within a network-contract deadline. No input, ownership
request, or heartbeat is valid before a successful `ServerHello`.

## Session epochs and ownership

An epoch is a non-zero, server-issued `uint64` generation for control ownership.
It is scoped to one running agent and is not a secret or authentication token.
The server advances it whenever ownership is granted, transferred, revoked,
released on timeout, or invalidated by suspend, lock, listener rebind, backend
failure, or shutdown. Values are never reused within the process.

Every operation batch that can affect control echoes the current epoch. A
standby client uses the epoch advertised in its `ControlState` when requesting
control but cannot send input actions until a later state grants ownership with
a new epoch. An old epoch is always rejected, even if its credential remains
paired. Reconnect starts a new connection sequence and never replays held state.

Ownership requests and relinquishment are explicit ordered operations. Before
the client enqueues either, it clears or sends all preceding motion/scroll work.
The server serializes transfer as: stop old-owner admission, discard discardable
old-owner motion, release all old-owner state, advance the epoch, assign the new
owner, then admit its actions. Failure leaves no controller and neutral input.

## Ordered operation batches

`ClientOperationBatch` contains a non-empty repeated `ClientOperation`. Array
order is semantic. The client dispatch pipeline is the sole producer and the
server validates the entire envelope before admitting any operation.

The operation variants are:

- relative pointer movement;
- normalized vertical or horizontal scroll;
- mouse-button down, up, or atomic click;
- physical-key down or up;
- validated UTF-8 text insertion;
- representable text-edit command;
- logical application command;
- media command;
- request control;
- relinquish control;
- release all.

Motion and scroll use signed fixed-point integers with 1/1024-unit resolution,
not floating-point values. This excludes NaN/infinity and retains fractional
input deterministically. Each action may include a nondecreasing microsecond
offset from its envelope timestamp. Backends convert normalized values to their
native units and keep any required fractional accumulators.

A state-changing operation is a button/key transition, click, text/edit/media
operation, logical command, ownership operation, or release. The client flushes
all older motion/scroll into earlier batch positions before adding one. The
server must neither coalesce across such a boundary nor execute only a prefix of
an invalid batch.

Motion and compatible same-axis scroll may be coalesced while preserving their
position between barriers. State-changing operations are never silently
dropped. On saturation, a release operation uses the reserved control path:
discard safe queued motion, stop normal admission, neutralize state, and close
or explicitly resynchronize. `ReleaseAll` terminates processing of later
operations in its batch; PR-002 validation rejects a batch that places another
operation after it.

## Canonical input identities

### Physical keys

A physical key is the numeric pair `(usage_page, usage_id)` from the USB-IF
[HID Usage Tables 1.5](https://usb.org/sites/default/files/hut1_5.pdf). Version
1 accepts only the Keyboard/Keypad usage page `0x07` and an explicit allowlist
of defined keyboard usages required by the product. Modifier usages `0xE0`
through `0xE7` remain ordinary physical keys with down/up state. Reserved,
error, vendor-defined, consumer-page, and unknown usages are rejected before
the dispatcher.

Navigation, function, and printable physical keys use this representation. A
JavaScript `KeyboardEvent.key`, localized label, platform scan code, or Unicode
scalar is never the canonical physical-key identity. Each backend owns the
mapping from an allowed HID usage to its native representation and returns
unsupported rather than inventing a mapping.

### Mouse buttons

The closed version-1 set is left, right, middle, back, and forward. Capability
bits report which are available. Down/up is used for held state; atomic click
accepts a bounded click count of one or two and creates no persistent held state
after the operation completes.

### Logical and media commands

Logical application commands are a closed enum separate from physical keys:
copy, cut, paste shortcut, undo, redo, select all, save, find, close tab, switch
application, and task-manager/platform equivalent. They mean “invoke the
controlled platform's conventional key chord”; `paste` does not authorize
reading or replacing either clipboard. Custom shortcut decks encode reviewed
physical-key transitions, not arbitrary command strings.

Media commands are a separate closed enum: play/pause, next track, previous
track, volume up, volume down, and mute. Unsupported logical or media commands
are disabled from the current capability snapshot and are rejected if sent.

### Text and editing

Text insertion is a protobuf string containing well-formed UTF-8 and is distinct
from physical keys. Empty chunks, invalid Unicode, chunks over the negotiated
limit, and chunks unsupported by the active backend capability are rejected.
Chunking occurs at Unicode scalar boundaries; a backend may apply a smaller
native boundary. Protocol text must never appear in logs or derived `Debug`.

Text-edit commands are closed semantic operations for backward deletion,
forward deletion, and bounded line-break insertion when the mobile edit engine
can map them safely. Platform shortcuts and navigation remain logical or
physical key operations rather than opaque text edits. A client falls back to
Compose and Send when live edits cannot be represented.

## Capability snapshots

The server is authoritative. A monotonically increasing capability revision is
sent in `ServerHello` and every `CapabilitySnapshot`. The snapshot reports:

- backend initialized and permission state;
- pointer and physical-key availability;
- supported mouse buttons and atomic click;
- vertical and horizontal scrolling independently;
- text modes: none, active-layout-mappable, native Unicode best effort, and EI
  UTF-8 when advertised and validated;
- supported logical and media commands;
- effective numeric, text, and batch limits;
- whether user authorization or troubleshooting is required.

Capability changes caused by permission revocation, compositor/device changes,
session lock, or backend faults advance the revision. The server rejects an
operation that is no longer supported even if it was valid under an older
snapshot. The UI disables unsupported functions and never treats a requested
capability as proof it exists.

## Application-layer heartbeat

Browser JavaScript schedules `HeartbeatPing` approximately once per second while
connected. Its monotonically increasing heartbeat identifier is independent of
the envelope sequence. The server validates it in session actor order, records
the receipt time using the server monotonic clock, and echoes the identifier in
`HeartbeatPong` for client-local RTT estimation.

Only a valid protobuf heartbeat produced by the page runtime refreshes this
liveness deadline. WebSocket Ping/Pong, TCP activity, control traffic, and an
open socket do not. Approximately four seconds without a valid heartbeat
revokes ownership and invokes release-all through the emergency path. Exact
interval and timeout values are sent by `ServerHello`, remain within reviewed
bounds, and are tested with a fake clock.

## Errors and WebSocket closure

`ProtocolError` contains only a closed code, retryability, safe numeric context,
and optional supported-version/limit values. It must not echo an offending
payload, text, key, cookie, token, raw frame, client name, host, origin, or URL.

Closed error codes include:

- `MALFORMED_MESSAGE`
- `MESSAGE_TOO_LARGE`
- `INCOMPATIBLE_VERSION`
- `UNKNOWN_OR_UNSUPPORTED_OPERATION`
- `AUTHENTICATION_REQUIRED`
- `NOT_CONTROLLER`
- `STALE_EPOCH`
- `INVALID_SEQUENCE`
- `CAPABILITY_UNAVAILABLE`
- `RATE_LIMITED`
- `QUEUE_SATURATED`
- `BACKEND_UNAVAILABLE`
- `SERVER_SUSPENDING`
- `INTERNAL_FAILURE`

Use standard WebSocket close code `1000` for an orderly client departure,
`1001` for server shutdown/rebind when appropriate, `1008` for a post-upgrade
policy violation, `1009` for an oversized message, and `1011` for an unexpected
server/backend failure. Version 1 reserves private codes `4001` authentication
required, `4002` incompatible version, `4003` invalid sequence/epoch, `4004`
rate or queue safety shutdown, `4005` control revoked, and `4006` heartbeat
timeout. A close reason is a short fixed phrase selected by code, never
user-derived data. RFC 6455 defines close frames and the private-use range in
the [WebSocket protocol](https://www.rfc-editor.org/rfc/rfc6455).

Cleanup is authoritative over error delivery. If an error frame could delay or
block neutralization, release and close without it.

## Hard limits

The server may advertise lower effective limits, but neither side may negotiate
above these version-1 ceilings:

| Item | Hard ceiling |
|---|---:|
| Encoded WebSocket binary message / protobuf envelope | 65,536 bytes |
| Operations in one batch | 256 |
| UTF-8 bytes in one text operation | 4,096 |
| Aggregate UTF-8 text bytes in one batch | 16,384 |
| UTF-8 client display name | 128 bytes |
| ASCII build identifier | 64 bytes |
| Absolute pointer or scroll fixed-point delta per operation | 8,388,608 |
| Atomic click count | 2 |

The network contract adds smaller frame/body/header limits where appropriate,
connection and rate quotas, aggregate queued-byte bounds, deadlines, and a
maximum hello time. Protobuf parsers receive an explicit decode limit. Counts,
numeric ranges, enum integers, string sizes, UTF-8, epoch, sequence, capability,
and direction are validated again during wire-to-domain conversion.

## Evolution rules

- The authoritative schema package is `tether.control.v1`.
- Minor versions are additive and must preserve current semantics. A sender uses
  a new field or payload only after negotiating the minor/capability that
  defines it.
- A semantic change to ordering, authentication, ownership, held-state, an
  existing field, or an existing enum value requires a new major protocol.
- Every enum defines zero as `UNSPECIFIED`; domain validation rejects zero and
  unknown integers where a concrete value is required. Proto3 enums are open at
  the wire level, so generated parsing alone is never validation.
- Deleted field numbers and names, enum values, and message names are reserved.
  The Protocol Buffers guide requires reserving deleted field numbers; see the
  [proto3 language guide](https://protobuf.dev/programming-guides/proto3/#reserved).
- Unknown fields inside an otherwise known message may be retained/ignored only
  under a negotiated compatible minor. An unknown `oneof` payload or operation
  is not safe to ignore and produces an unsupported-operation error.
- Buf `STANDARD` lint and `FILE` breaking rules gate schema changes. Buf notes
  that `FILE` also implies the package and wire checks; see
  [breaking-change detection](https://buf.build/docs/breaking/).
- Rust and TypeScript generated sources, golden bidirectional fixtures, maximum
  sizes, unknown enums/fields, and incompatible versions must agree before a
  protocol change merges.

## Required review checklist

Security review must confirm authentication occurs before session admission,
no secret or input content enters diagnostics, all untrusted values are bounded,
unknown values fail closed, release is prioritized over error delivery, and no
LAN administration or remote-command surface entered the schema.

Native-backend review must confirm every physical/logical/media identifier has
an explicit backend mapping or unsupported result, text stays separate from
keys, fixed-point axes and signs are unambiguous, capability changes are
authoritative, and release/ownership ordering can be implemented on every
backend. The completed reviews are recorded in `protocol-reviews.md`.
