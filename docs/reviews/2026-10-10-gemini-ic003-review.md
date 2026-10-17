# Tether Independent Implementation Review: Task IC-003

Ran command: List review and component files (`ls -la docs/reviews docs/components crates/input-core/src`)
Ran command: Run cargo fmt, clippy, and test (`export PATH="/Users/jl/.cargo/bin:$PATH" && cargo fmt --all --check && cargo clippy --all-targets --workspace -- -D warnings && cargo test --all-targets --workspace`)
Viewed 2026-10-10-gemini-baseline-audit-response.md:1-92
Viewed input-dispatcher.md:1-106
Viewed lib.rs:1-28
Ran command: Git diff on backend and mock (`git diff crates/input-core/src/backend.rs crates/input-core/src/mock.rs`)
Ran command: Git status check (`git status`)
Ran command: Git diff on docs (`git diff README.md docs/components/README.md docs/implementation-plan.md`)
Ran command: Count lines in dispatcher.rs (`wc -l crates/input-core/src/dispatcher.rs`)
Viewed dispatcher.rs:1-250
Viewed dispatcher.rs:251-500
Viewed dispatcher.rs:501-750
Viewed dispatcher.rs:751-1050
Viewed dispatcher.rs:1050-1350
Viewed dispatcher.rs:1351-1687
Ran command: Check local links tool (`node tooling/check-links.mjs`)
Ran command: Check git diff whitespace (`git diff --check`)

**Review Target:** Implementation of Task `IC-003` (*Bounded serialized input dispatcher*)
**Review Date:** 2026-10-10
**Baseline Reference:** [`docs/reviews/2026-10-09-gemini-baseline-audit.md`](file:///Users/jl/tether/docs/reviews/2026-10-09-gemini-baseline-audit.md) & [`docs/reviews/2026-10-10-gemini-baseline-audit-response.md`](file:///Users/jl/tether/docs/reviews/2026-10-10-gemini-baseline-audit-response.md)
**Overall Decision:** **CONDITIONAL GO** for beginning `IC-004`

---

## 1. Executive Summary & Review Decision

### Review Decision: **CONDITIONAL GO**

Task `IC-003` has delivered a high-quality, production-grade serialized input dispatcher in [`crates/input-core/src/dispatcher.rs`](file:///Users/jl/tether/crates/input-core/src/dispatcher.rs) accompanied by an accepted component specification in [`docs/components/input-dispatcher.md`](file:///Users/jl/tether/docs/components/input-dispatcher.md). The implementation satisfies the vast majority of architectural requirements:

- Exclusive backend ownership on a single dedicated OS thread (`tether-input-dispatch`).
- A separate, priority-escalating 1-slot emergency release mailbox that cannot be starved by a saturated logical queue.
- Strict $O(1)$ memory bound on emergency releases and bounded logical action queues.
- Widened $i64$ fixed-point coalescing arithmetic with domain constructor validation and barrier boundaries.
- Unwind containment for backend execution and cleanup panics, paired with poison-resilient mutex locking.
- Enhanced mock observability ([`MockInputBackend::release_calls`](file:///Users/jl/tether/crates/input-core/src/mock.rs#L102-L105)) and deterministic simulated latency across all execution and release paths.

**Condition for Proceeding to `IC-004` / `SS-001`:**
Codex must resolve **Finding D-01** (Fault Mode Overwrite): A dispatcher that has entered `DispatcherMode::Faulted` (e.g. from queue saturation, transition violation, or backend fault) can currently be reset back to `DispatcherMode::Suspended` and reopened via `resume()` if a caller subsequent to the fault issues an ordinary release request (such as `EmergencyReason::ExplicitStop`) that completes neutrally. A faulted dispatcher must permanently remain `Faulted` until replaced.

---

## 2. Independent Verification Log

The following checks were independently executed in the repository:

| Verification Target | Command Executed | Result | Details |
| --- | --- | --- | --- |
| Workspace Formatting | `cargo fmt --all --check` | **PASSED** | Code formatting is compliant across all workspace crates. |
| Strict Workspace Clippy | `cargo clippy --all-targets --workspace -- -D warnings` | **PASSED** | Zero warnings under `-D warnings`, `unsafe_code = "deny"`, `all = "deny"`, `pedantic = "deny"`. |
| Workspace Test Suite | `cargo test --all-targets --workspace` | **PASSED** | **45 unit tests passed, 0 failed** (28 in `input-core`, 7 in `test-support`, 10 in `xtask`). |
| Git Hygiene Check | `git diff --check` | **PASSED** | Zero whitespace or formatting anomalies. |
| Local Link Validation | `node tooling/check-links.mjs` | **PASSED** | All local Markdown links across **42 documentation files** resolve cleanly. |

---

## 3. Evaluation of Key Areas

### 1. Emergency Release Under Normal Queue Saturation & Exclusive Backend Ownership

- **Status:** **Verified Compliant.**
- **Analysis:**
  - The ordinary queue ([`state.queue`](file:///Users/jl/tether/crates/input-core/src/dispatcher.rs#L414)) and the emergency release mailbox ([`state.emergency`](file:///Users/jl/tether/crates/input-core/src/dispatcher.rs#L418)) are completely segregated.
  - When the queue reaches capacity (`state.queued_actions == state.queue_capacity`), [`submit()`](file:///Users/jl/tether/crates/input-core/src/dispatcher.rs#L494-L501) invokes [`begin_emergency_locked`](file:///Users/jl/tether/crates/input-core/src/dispatcher.rs#L636), sets `state.mode = DispatcherMode::Releasing`, cancels all waiting actions with `CancellationReason::Emergency(QueueSaturated)`, places the release into `state.emergency`, and immediately rejects the incoming action with `AdmissionError::QueueSaturated`.
  - In the worker loop ([`next_work`](file:///Users/jl/tether/crates/input-core/src/dispatcher.rs#L771-L790)), `state.emergency.take()` is evaluated *before* `state.queue.pop_front()`.
  - Backend execution is strictly serialized on the worker thread: [`backend.release_all()`](file:///Users/jl/tether/crates/input-core/src/dispatcher.rs#L749) is called exclusively by the worker. Callers never touch the backend directly; they wait on a shared [`ReleaseSlot`](file:///Users/jl/tether/crates/input-core/src/dispatcher.rs#L282-L292) via [`ReleaseTicket`](file:///Users/jl/tether/crates/input-core/src/dispatcher.rs#L295-L355).

### 2. Ordering of Keys, Modifiers, Mouse Buttons, Barriers, and Releases

- **Status:** **Verified Compliant.**
- **Analysis:**
  - [`HeldLedger`](file:///Users/jl/tether/crates/input-core/src/dispatcher.rs#L962-L1047) stores pressed buttons and keys in explicit insertion order (`Vec<MouseButton>`, `Vec<PhysicalKey>`).
  - Validation rejects duplicate `Down` actions before native execution with `RejectionReason::DuplicateDown` and unmatched `Up` actions with `RejectionReason::UnmatchedUp`.
  - The tested release plan ([`HeldLedger::release_plan`](file:///Users/jl/tether/crates/input-core/src/dispatcher.rs#L1023-L1046)) strictly enforces:
    1. Mouse buttons in reverse press order (`buttons.iter().rev()`).
    2. Non-modifier physical keys in reverse press order (`keys.iter().rev().filter(!is_modifier)`).
    3. Modifier keys (`0xE0..=0xE7`) in reverse press order (`keys.iter().rev().filter(is_modifier)`).
  - The dispatcher does not synthesize synthetic native `Up` calls during cleanup; it delegates native cleanup to `backend.release_all()`, clearing its own ledger only when [`report.is_confirmed_neutral()`](file:///Users/jl/tether/crates/input-core/src/backend.rs#L252-L256) is true.

### 3. Coalescing Under Overflow, Opposite Signs, and State Changes

- **Status:** **Verified Compliant.**
- **Analysis:**
  - [`try_coalesce`](file:///Users/jl/tether/crates/input-core/src/dispatcher.rs#L685-L717) uses $i64$ widened arithmetic:
    `let x = i64::from(existing.x()) + i64::from(incoming.x());`
  - If the sum exceeds `MAX_ABSOLUTE_FIXED_DELTA` ($8,388,608$), `PointerDelta::new(x, y)` fails and `try_coalesce` returns `false`. The existing entry is left intact and the incoming delta is pushed as a separate queue entry. No clipping, integer wrapping, or silent dropping occurs.
  - Opposite-signed deltas (e.g. $+100$ and $-40$) algebraically combine to net $+60$ and $+0$ without issue.
  - Cross-axis scroll coalescing is explicitly prohibited: `try_coalesce` verifies `existing.axis() == incoming.axis()`.
  - State changes act as impermeable barriers: any action other than identical adjacent motion/scroll returns `false` in `try_coalesce`, preventing coalescing across button clicks, key presses, commands, or text insertion.

### 4. Race Conditions, Worker Panics, and Mutex Poisoning

- **Status:** **Verified Compliant.**
- **Analysis:**
  - All mutex acquisitions use [`lock_recover`](file:///Users/jl/tether/crates/input-core/src/dispatcher.rs#L617-L619) (`.lock().unwrap_or_else(PoisonError::into_inner)`), ensuring worker and caller threads recover from thread panics without hanging.
  - `backend.execute()` is wrapped in `panic::catch_unwind(AssertUnwindSafe(...))`. A backend panic maps to `BackendErrorCategory::Fatal`, schedules emergency release with `EmergencyReason::WorkerPanic`, rejects the receipt, and cleanly terminates the worker.
  - `backend.release_all()` is independently wrapped in `panic::catch_unwind`. A panic during release returns an `uncertain_release_report()` and marks the dispatcher as `Faulted`.
  - Dropping an admitted action's [`DispatchReceipt`](file:///Users/jl/tether/crates/input-core/src/dispatcher.rs#L226-L271) drops the internal receiver. When the worker completes execution, `completion.sender.send(...)` error is safely ignored (`let _receiver_may_be_gone = ...`), ensuring accepted work executes to completion.

### 5. Receipt & Outcome Categorization

- **Status:** **Verified Compliant.**
- **Analysis:**
  - [`DispatchOutcome`](file:///Users/jl/tether/crates/input-core/src/dispatcher.rs#L151-L169) explicitly separates:
    - `Executed { position }` (native success)
    - `BackendRejected { position, error }` (native call failure or panic)
    - `Rejected { position, reason }` (dispatcher ledger validation failure: `DuplicateDown` / `UnmatchedUp`)
    - `Cancelled { position, reason }` (cancelled in queue prior to execution: `Emergency` / `WorkerStopped`)
  - Receipts never expose typed text, key codes, button names, or motion coordinates in `Debug` or `Display`.

### 6. Resource Bounds Under Sustained Load

- **Status:** **Verified Compliant.**
- **Analysis:**
  - Logical action count is strictly tracked: `state.queued_actions += 1` increments for every admitted action, even when coalesced into an existing `QueueEntry`. Coalescing cannot create an unbounded completion list.
  - Emergency releases share a single coalescing mailbox slot ([`state.emergency`](file:///Users/jl/tether/crates/input-core/src/dispatcher.rs#L418)). Concurrent release calls upgrade `reason` by priority and share the single `ReleaseTicket` handle. Memory overhead is strictly $O(1)$.
  - Position counter exhaustion at $2^{64}-2$ reserves $2^{64}-1$ exclusively for the final emergency release barrier and closes ordinary admission.

### 7. Test Suite Depth & Failure Coverage

- **Status:** **Verified Compliant.**
- **Analysis:**
  - 18 targeted unit tests in `dispatcher::tests` test:
    - Bounded queue limits and invalid configurations.
    - Blocking native backend simulation using condvar gates.
    - Exact saturation shutdown and cancellation delivery.
    - $5,000,000 + 5,000,000$ fixed-point coalescing flush without overflow.
    - Dropped receipts and out-of-order client drops.
    - Combinatorial 64-path model checking over 3-action transition sequences.
    - Mutex poisoning injection.
    - Backend execution panic and release panic containment.
    - Inconsistent neutrality reports (`neutral: true` with non-zero failures or mismatched counts).

### 8. Architectural Alignment & Compatibility with IC-004

- **Status:** **Verified Compliant.**
- **Analysis:**
  - `InputDispatcher` consumes platform-neutral [`InputAction`](file:///Users/jl/tether/crates/input-core/src/action.rs#L361-L373) and delegates to any `B: InputBackend + 'static`.
  - Planned `IC-004` (pointer/scroll normalization, sensitivity, acceleration, and accumulators) will sit upstream of the dispatcher or integrate into translation hooks, feeding normalized `PointerDelta` and `ScrollDelta` into `submit()`.
  - Dispatcher shutdown yields ownership of the original backend (`ShutdownResult.backend`), supporting seamless composition-root integration in Tauri (`DT-002`).

---

## 4. Prioritized Findings

### A. Confirmed Defects

#### Finding D-01: Dispatcher Can Illegitimately Clear `Faulted` Mode Via Ordinary Release

- **Severity:** Medium
- **Location:** [`crates/input-core/src/dispatcher.rs:881-887`](file:///Users/jl/tether/crates/input-core/src/dispatcher.rs#L881-L887) and [`crates/input-core/src/dispatcher.rs:116-133`](file:///Users/jl/tether/crates/input-core/src/dispatcher.rs#L116-L133)
- **Evidence & Concrete Failure Scenario:**
  1. The dispatcher encounters an unrecoverable fault (e.g. `EmergencyReason::QueueSaturated`, `EmergencyReason::InvalidTransition`, or a fatal `BackendFault`).
  2. The worker completes cleanup and sets `state.mode = DispatcherMode::Faulted`. In this mode, `submit()` is rejected and `resume()` is rejected with `ResumeError::NotSuspended(Faulted)`.
  3. A caller subsequently invokes `dispatcher.request_release(EmergencyReason::ExplicitStop)` (e.g. from an automated session disconnect handler or a user clicking "Stop Control").
  4. `begin_emergency_locked` overwrites `state.mode = DispatcherMode::Releasing` and enqueues a `PendingEmergency` with `reason = EmergencyReason::ExplicitStop`.
  5. The backend executes `release_all()`, which returns `ReleaseReport::neutral()`.
  6. In `complete_release`, lines 884–887 evaluate:

     ```rust
     state.mode = if release_panicked {
         DispatcherMode::Faulted
     } else {
         pending
             .reason
             .mode_after_release(outcome.report.is_confirmed_neutral())
     };
     ```

  7. For `EmergencyReason::ExplicitStop` with confirmed neutrality, `mode_after_release(true)` evaluates to `DispatcherMode::Suspended`.
  8. `state.mode` is set to `DispatcherMode::Suspended`, forgetting that the dispatcher was previously `Faulted`.
  9. The caller now calls `dispatcher.resume()`, which succeeds because `state.mode == DispatcherMode::Suspended`, restoring `state.mode = DispatcherMode::Running`.
  10. **Impact:** A dispatcher that experienced queue saturation or an invalid transition resumes admitting input, directly violating the contract in [`docs/components/input-dispatcher.md:57-59`](file:///Users/jl/tether/docs/components/input-dispatcher.md#L57-L59) (*"Saturation, an impossible transition, backend fault, backend panic, an internally inconsistent release report, or uncertain neutrality ends in `Faulted`; constructing a reviewed replacement is required"*).
- **Proposed Correction:**
  In `complete_release`, ensure that if the dispatcher was already in `DispatcherMode::Faulted` prior to the release operation, its post-release mode remains `DispatcherMode::Faulted` (unless `pending.reason == EmergencyReason::Shutdown`, which transitions to `DispatcherMode::Stopped`).

  ```rust
  // Example correction in complete_release:
  let prior_mode = state.mode;
  state.mode = if release_panicked || prior_mode == DispatcherMode::Faulted {
      DispatcherMode::Faulted
  } else {
      pending
          .reason
          .mode_after_release(outcome.report.is_confirmed_neutral())
  };
  ```

- **Blocks IC-004:** **No.** `IC-004` implements normalization and accumulators, which do not depend on dispatcher recovery modes. This correction can be applied before or during `SS-001`/`SS-002`.

---

### B. Architectural Concerns & Edge Observations

#### Concern C-01: `HeldLedger` Does Not Validate Atomic Click During Held Button State

- **Severity:** Low
- **Location:** [`crates/input-core/src/dispatcher.rs:970-1000`](file:///Users/jl/tether/crates/input-core/src/dispatcher.rs#L970-L1000)
- **Evidence & Failure Scenario:**
  In `HeldLedger::validate`:

  ```rust
  match action {
      InputAction::MouseButton(ButtonAction::Down(button)) => ...
      InputAction::MouseButton(ButtonAction::Up(button)) => ...
      InputAction::PhysicalKey(KeyAction::Down(key)) => ...
      InputAction::PhysicalKey(KeyAction::Up(key)) => ...
      _ => Ok(()),
  }
  ```

  If mouse button `Left` is currently held down (`buttons.contains(&MouseButton::Left)`), an incoming action of `InputAction::MouseButton(ButtonAction::Click { button: MouseButton::Left, count: ClickCount::Single })` falls into `_ => Ok(())`. If executed, native injection on Windows/macOS would issue a down/up sequence while the button was already considered held down, potentially corrupting drag state.
- **Proposed Correction:**
  In `HeldLedger::validate`, reject `ButtonAction::Click { button, .. }` if `self.buttons.contains(button)` with `RejectionReason::DuplicateDown` or a dedicated `RejectionReason::ButtonAlreadyHeld`.
- **Blocks IC-004:** **No.**

---

## 5. Scope Gate & Implementation Plan Alignment

- **IC-003 Acceptance Criteria Satisfaction:**
  The deliverable for `IC-003` was:
  > *"Implement the bounded serialized input dispatcher with separate emergency control, deterministic ordering, cancellation, queue saturation shutdown, backend fault handling, and idempotent release-all. Add race/fault/model tests."*
  All elements of this deliverable are fully present, passing, and verified by 18 exhaustive unit tests.
- **Implementation Plan Status:**
  `IC-003` is correctly marked as `DONE` in [`docs/implementation-plan.md:68`](file:///Users/jl/tether/docs/implementation-plan.md#L68).
  `IC-004` (*Pointer/scroll normalization, sensitivity, optional acceleration, clamping, fractional accumulators*) has its dependencies satisfied and is correctly marked as `READY`.
- **Pre-existing Blockers (`F-002`, `PR-002`, `BV-001`):**
  These remain accurately labeled as `BLOCKED` with no false claims of unexecuted checks.

---

## 6. Actionable Recommendations Before/During IC-004

1. **Patch Fault Mode Latch in `dispatcher.rs` (Finding D-01):** Prevent non-shutdown releases from clearing `DispatcherMode::Faulted` back to `Suspended`.
2. **Validate Click on Held Button (Concern C-01):** Disallow `ButtonAction::Click` when the corresponding button is in the pressed state.
3. **Proceed with IC-004:** Begin implementation of pointer/scroll normalization, sensitivity curves, acceleration, fractional accumulators, and platform translation hooks in `crates/input-core`.

---

## 7. Conclusion

The implementation of `IC-003` represents an exceptionally robust, well-architected execution of the Tether input core specification. The concurrency and ordering invariants are clean, deterministic, and rigorously tested.

With the sole caveat that Finding D-01 must be tracked and patched prior to session-level integration (`SS-001`), the implementation is approved for **CONDITIONAL GO** to begin **IC-004**.
