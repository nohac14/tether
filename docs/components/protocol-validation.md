# Protocol validation contract

**Task:** PR-004

**Status:** Implemented; [dated verification](../manual-tests/2026-11-16-protocol-validation.md)

## Boundary

The public validator accepts bounded encoded bytes plus trusted, immutable caller
context. It returns owned validated values or a closed payload-free error. It has
no backend, dispatcher, network, clock, storage or session-actor handle and cannot
execute input or mutate ownership. Actual cookie verification, session mutation,
rate limiting and scoped emergency cleanup remain NW/PA/SS responsibilities.
An authentication/ownership assertion in caller context never comes from a wire
message. Context must be rechecked by the serialized session actor before use.

Validation order is authentication where applicable, encoded-size bound, bounded
structural preflight, prost decode, then whole-envelope semantic validation.
There is no successful prefix of an invalid batch. A private temporary held-state
projection checks transitions without changing the caller's ledger. Only actual
successful dispatcher execution can update the future actor's authoritative ledger.

## Structural preflight

Protobuf deliberately supports last-one-wins/merge semantics and discards unknown
fields. Therefore inspecting only a generated value cannot prove that an encoded
envelope contained exactly one payload or no unknown action. See the official
[encoding rules](https://protobuf.dev/programming-guides/encoding/#last-one-wins).

The existing descriptor-based Rust generator also emits private field metadata
from the authoritative `.proto` into the same generated file. The guard uses that
metadata, not a second handwritten field schema. It bounds depth (16), total
field/packed-element visits (4,096), singular occurrence, repeated cardinality,
scalar width, booleans, enum values, UTF-8 and string/bytes lengths before prost
allocates collections. Existing 64 KiB/256-operation/4 KiB-text ceilings remain.

Client/server envelopes and client operations are closed union containers:
metadata occupies the low field numbers and the remaining extension space is
the payload/action union. Unknown fields there are unsupported rather than
silently ignored. A duplicate known singular field or second real oneof member
is malformed, including repeated occurrences of the same member. Ordinary
unknown fields inside known payload messages may be skipped under the selected
compatible minor, within the same visit/size budgets. Field order is unrestricted;
packed, unpacked and multiple packed segments of repeated enum fields work.
Deprecated group encodings are not emitted by the proto3 schema and are rejected.
The stricter framing policy is Tether admission policy, not a generic protobuf
conformance claim. Synthetic proto3-optional fields are not real union members.

## Semantic validation

Protocol major is 1 and implemented minor is 0. Hello envelopes use bootstrap
minor 0, sequence 0 and epoch 0; ranges negotiate the highest implemented mutual
minor. Both ordered-batch and page-runtime-heartbeat client capabilities are
mandatory; duplicates/unknown values fail closed. Runtime sequence must equal
the actor-provided next nonzero sequence; timestamps never regress and offsets
must be nondecreasing without u64 overflow. Epoch must match current ownership.
An initial noncontrolling connection may echo epoch 0 while its advertised state
is NoController or Suspended; Standby and Controlling snapshots require a
nonzero epoch. Heartbeat identifiers must strictly increase relative to the
caller's last accepted heartbeat, without wrapping.

Limits are positive, never above version hard ceilings and may only decrease
through negotiation. Names are nonempty, at most 128 UTF-8 bytes, display-only,
untrusted and control-character-free; build IDs are nonempty printable ASCII
up to 64 bytes. All metadata diagnostics are redacted. Connection identifiers
are opaque 16-byte non-authentication correlation values; their generation is
not implemented by this task. Version-one heartbeat validation accepts intervals
250–1,000 ms and timeouts 1,000–4,000 ms, at least two intervals; the network
contract's initial server policy stays 1,000/4,000 ms.

Every input maps to existing input-core types and is checked against current
backend status/capabilities and lower negotiated limits. Unsupported individual
HID mappings still belong to the native backend's last-moment preflight. Text
must be nonempty valid UTF-8 within per-operation and aggregate byte budgets;
native layout/application-specific support is not inferred. Atomic clicks need
their explicit flag, supported button and count, and must not target a held button.
Down/up transitions must agree with projected held state. Text-edit repeats are
positive and expanded in place within the same 256-action budget. Ownership and
release operations remain separate from native actions; they terminate their
batch so admission cannot accidentally execute work across an ownership barrier.

Server messages also validate negotiated limits, all capability/control/error
enums, revisions, coherent permission/availability, duplicate capability lists,
initial hello and subsequent sequence/epoch rules. Capability revisions cannot
regress. A control update may advance epoch, but unrelated messages may not.
Backend-to-wire conversion advertises no input when the native status is
unavailable/faulted; transiently unknown permission maps conservatively to the
wire Unavailable state. Positive click limits never grant atomic capability.

## Failure handling and tests

Errors contain no library error, text, names, HID/button identities, frame bytes
or offending values. Mapping to protocol errors/close codes is fixed and safe.
Post-hello failures require the future actor to stop admission and use its
reserved release path for the affected owner before/while closing. A failed
unauthenticated/standby connection must not release some other controller.

Tests cover every operation family and direction, numeric/text/size boundaries,
malformed/unknown/duplicate/packed fields, pre-allocation limits, whole-batch
rollback, held transitions, capability revocation, stale epoch/sequence,
counter/timestamp overflow, metadata/error redaction and deterministic bounded
mutational/arbitrary-byte fuzzing using F-006 test-only seeded randomness.
No native input or LAN listener is part of this task. Complete cross-language
goldens remain PR-005; client connection integration remains MW-002/NW-006.
