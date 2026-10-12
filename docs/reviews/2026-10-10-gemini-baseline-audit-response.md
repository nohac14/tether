# Response to the Gemini baseline audit

- **Reviewed document:** `2026-10-09-gemini-baseline-audit.md`
- **Evaluation date:** 2026-10-10
- **Scope:** Repository baseline and IC-003

The audit was evaluated against the repository at `abb5e5b`, the complete
`SPEC.md`, `AGENTS.md`, accepted D-01 through D-05, the input-backend contract,
and the implementation plan. Its conditional go is accepted as authorization
for IC-003, not as evidence that IC-003 or any platform behavior was already
verified.

## Accepted and resolved for IC-003

### Mock release observability

The finding was correct. `MockInputBackend::release_all` previously changed no
observable value when already neutral and did not apply simulated latency. The
mock now exposes a sanitized `release_calls()` count and advances deterministic
simulated time for every release. It still does not record key, button, text, or
delta identities.

### Ordered dispatcher ledger

The finding was correct with one boundary clarification. The dispatcher now
stores buttons and keys in press order and tests the required buttons,
non-modifiers, then modifiers reverse-release plan. Native cleanup still calls
`InputBackend::release_all`; the dispatcher must not manufacture individual
`Up` calls that could conflict with the backend's mirror of native calls that
actually succeeded. This follows `docs/components/input-backend.md` rather than
using the mock's identity-sorted `BTreeSet` as the authoritative release order.

### Fixed-point coalescing headroom

The finding was correct. Coalescing uses `i64` intermediate sums and reconstructs
bounded domain values. A sum beyond the hard bound flushes into another queue
entry. Tests cover `5,000,000 + 5,000,000`, the exact positive limit, opposite
directions, and cross-axis scroll isolation.

### Monotonic positions, saturation, cancellation, faults, and poison

IC-003 assigns monotonic positions to actions and release barriers, keeps
completion metrics monotonic during cancellation races, and reserves the final
position for emergency release if ordinary position space is exhausted. Tests
cover bounded saturation, emergency priority, dropped receipts, backend error
and panic, inconsistent cleanup reports, mutex poison recovery, and an
exhaustive small held-state model.

### Verification while frontend tooling is unavailable

The audit correctly observed that the aggregate `cargo xtask check` cannot pass
while F-002's pinned npm tools are unavailable. IC-003 therefore uses the four
direct Rust gates required by its implementation plan. This is not a waiver of
the frontend gates and does not change F-002's blocked status.

## Valid finding deferred to its owning task

### Atomic-click capability on the wire

The schema currently has a maximum atomic-click count but no explicit
`atomic_click_available` field, while the Rust backend capability model has an
independent state. This is a valid PR-002/PR-004 alignment issue, not an IC-003
dispatcher issue. PR-002 remains incomplete until Buf validation is available;
that task must either add the explicit capability without reusing a reserved
field incorrectly or specify and test a zero-limit encoding. No wire behavior
is inferred in IC-003.

## Recommendations not adopted as stated

### Aggregate check should not succeed when prerequisites are missing

The proposed graceful non-failing behavior for `cargo xtask check` is rejected.
That command is intentionally the aggregate repository gate, so returning
success while required pinned tools or checks are absent would permit a false
green result contrary to `AGENTS.md`. A separately named Rust-only convenience
command could be considered in the tooling task, but direct Cargo commands are
unambiguous and sufficient for IC-003.

### Missing Git repository finding was stale

The audited workspace is a Git repository. At evaluation it was clean on
`main`, with `HEAD` and `origin/main` at `abb5e5b` (`chore: establish Tether
implementation baseline`). No initialization or history replacement is needed.

## Unchanged external blockers

- F-002 still requires the pinned npm dependencies and lockfile verification.
- PR-002 still requires Buf 1.72 format, lint, and breaking-baseline checks.
- BV-001 still requires a current physical Android/Chrome device on a LAN.
- No native backend, cross-platform package, or real-device behavior was tested
  by IC-003.
