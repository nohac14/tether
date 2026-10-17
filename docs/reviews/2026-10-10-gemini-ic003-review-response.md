# Response to Gemini IC-003 implementation review

- **Review:** `2026-10-10-gemini-ic003-review.md`
- **Disposition date:** 2026-10-10
- **Scope:** IC-003 findings D-01 and C-01

## Disposition

### D-01 — accepted and corrected

The dispatcher now records terminal failure in a persistent latch independent
of the transient public mode. Terminal reasons latch the condition when their
emergency release is requested; a release panic or internally inconsistent
neutrality report latches it at completion. Every non-shutdown release then
ends in `Faulted` while the latch is set. Shutdown remains the sole permitted
transition to `Stopped`.

This deliberately does not use the review's illustrative `prior_mode` check:
`begin_emergency_locked` changes the public mode to `Releasing` before native
cleanup starts, so the prior terminal condition cannot reliably be recovered
from `mode` during completion.

The regression `terminal_fault_survives_ordinary_and_repeated_releases`
executes the complete sequence identified by the review: an invalid transition
faults the instance, a confirmed-neutral ordinary release follows, and both
`resume` and new admission must still fail with `Faulted`. Against the original
completion logic, the first ordinary release computes `Suspended`, so this test
fails at the post-release mode assertion (and `resume` would succeed). Further
tests cover repeated release requests, shutdown from `Faulted`, and an ordinary
release racing a blocked terminal release. Snapshots expose the safe boolean
terminal latch so the invariant is observable even while mode is `Releasing`.

### C-01 — accepted and corrected

`HeldLedger` rejects an atomic click for a button already present in its held
set with `RejectionReason::ButtonAlreadyHeld`. Validation occurs before backend
execution and does not mutate the ledger. Tests prove the held-button release
plan is unchanged, the backend never receives the invalid click, an unheld
click remains valid, and the ordinary down/move/up drag sequence remains valid.

## Verification at the correction boundary

The following commands completed successfully on macOS arm64 with Rust 1.99.0:

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed, zero warnings |
| `cargo test --workspace --all-targets` | Passed, 50 tests (33 input-core, 7 test-support, 10 xtask) |
| `cargo build --workspace --all-targets` | Passed |

No native input backend and no physical-device or cross-platform validation ran
at this boundary. Repository documentation/link and hygiene checks are repeated
at the milestone boundary after IC-004, ST-002, and F-004.

## Remaining findings

No finding from this review is deferred. The review's conditional gate is
satisfied without changing the approved architecture.
