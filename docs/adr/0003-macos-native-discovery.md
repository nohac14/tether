# ADR 0003 — Mac-native discovery adapter

**Status:** ACCEPTED — explicitly approved by the owner in conversation, 2026-10-25

## Context

DS-002 source review found that mdns-sd 0.21.6 starts its worker and initial
all-interface multicast setup before processing its client's interface-selection
commands. See [the versioned source evidence](../reviews/2026-10-25-mdns-candidate-review.md).
It cannot be adopted unchanged under D-03/DS-001. No current requirement is
waived and no candidate library is a validated production dependency today.

## Decision

For the immediate Mac-first path, permit a platform-isolated Bonjour/DNS-SD
adapter within `tether-discovery`, retaining the existing platform-neutral
contract and lifecycle coordinator. Prefer maintained safe bindings that expose
all required scoped registration/record/lifetime APIs. Any necessary local FFI
must first receive a narrowly documented ownership, callback, ABI and cancellation
review; no crate-wide unsafe waiver, unsafe Send shortcut or background
all-interface Tether daemon is approved by this decision.

Before a native registration, require the same explicit trusted-LAN approval and
successful literal listener generation. Resolve approved current identities to
current nonzero native interface indexes; never persist/reuse indexes as identity
or pass zero/all-interface defaults. Publish exactly current bound A/AAAA records,
the stable installation-derived hostname and bounded service metadata. Own and
withdraw every registration handle within the coordinator's bounded lifecycle.
The existing OS Bonjour daemon is a trusted OS facility, not a new cloud or
privileged helper; Tether neither controls nor claims to stop unrelated OS traffic.

This approval authorizes implementing/evaluating this adapter, not claiming
it passes physical validation, selecting unreviewed bindings, calling live LAN
registration APIs without explicit test scope, or skipping any DS-002 gate.
Windows/Linux stay behind the shared discovery boundary and require separately
reviewed/validated platform choices. Do not silently install Bonjour on Windows.

## Alternatives

- **Wait for an upstream mdns-sd fix:** acceptable if its official release exposes
  a verified pre-start interface allowlist and satisfies resource/lifecycle tests.
  No such capability is assumed for the reviewed 0.21.6 package.
- **Maintain a scoped fork:** not selected. It would require a reviewed patch,
  reproducible provenance, source-policy exception and ongoing maintenance;
  a delay/disable-after-start wrapper cannot repair the original ordering.
- **Disable discovery/use only IPs:** rejected as a permanent substitute because
  stable `.local` reconnection/discovery remains part of the complete product.
- **Permit temporary all-interface startup:** rejected; changes the approved
  exposure requirement and can trigger unintended privacy/network activity.

## Required verification and consequences

The spike must prove explicit per-interface service and custom-host A/AAAA record
registration, no auto-rename escape from the bounded conflict policy, actual
callback/readiness handling, bounded handles/queues/bytes, failure/timeout/drop
ordering, no stale generation resurrection, and safe native deallocation.
The SDK explicitly requires separate address records for a custom SRV host;
advertising only the service is insufficient. Do not use blocking event
processing without readiness or deallocate concurrently with callbacks.

Begin with inert event/FFI drivers. Real multicast, name-conflict, Bonjour
coexistence, Local Network denial/restore, IPv4/IPv6 resolution, DHCP and goodbye
tests remain explicitly opted in and recorded on the actual Mac/iPhone network.
Public signing gates remain deferred under ADR 0001, not marked passed.

SPEC, the architecture record and plan now follow this accepted decision.
DS-002 may proceed with binding review, compilation and inert validation;
actual registration/physical validation still requires its explicit test scope.
DS-003 remains gated on DS-002's completion, not merely this approval.
