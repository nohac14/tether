Ran command: Git status check (`git status`)
Ran command: List workspace directory (`ls -la`)
Ran command: List docs directory (`ls -la docs`)
Ran command: List component docs directory (`ls -la docs/components`)
Ran command: List crates directory (`ls -la crates`)
Ran command: List proto apps tests tooling xtask (`ls -la proto apps tests tooling xtask`)
Viewed implementation-plan.md:1-199
Viewed architecture-decisions.md:1-221
Viewed open-risks.md:1-78
Viewed environment-inventory.md:1-84
Viewed SPEC.md:1-120
Ran command: Check all targets in cargo workspace (`cargo check --all-targets --workspace`)
Ran command: Check environment PATH (`echo $PATH`)
Ran command: Check cargo path in home (`ls -la /Users/jl/.cargo/bin`)
Ran command: Cargo check all targets (`export PATH="/Users/jl/.cargo/bin:$PATH" && cargo check --all-targets --workspace`)
Ran command: Check Rust toolchain versions (`export PATH="/Users/jl/.cargo/bin:$PATH" && cargo --version && rustc --version`)
Ran command: Check cargo formatting (`export PATH="/Users/jl/.cargo/bin:$PATH" && cargo fmt --all --check`)
Ran command: Run cargo clippy on workspace (`export PATH="/Users/jl/.cargo/bin:$PATH" && cargo clippy --all-targets --workspace -- -D warnings`)
Ran command: Run all cargo tests (`export PATH="/Users/jl/.cargo/bin:$PATH" && cargo test --all-targets --workspace`)
Ran command: Run cargo xtask help (`export PATH="/Users/jl/.cargo/bin:$PATH" && cargo xtask help`)
Ran command: Run aggregate cargo xtask check (`export PATH="/Users/jl/.cargo/bin:$PATH" && cargo xtask check`)
Ran command: Find pnpm binary (`which pnpm || find / -name pnpm 2>/dev/null`)
Killed task Find pnpm binary
Ran command: Locate node and pnpm (`which node; which npm; which pnpm; ls -la /opt/homebrew/bin/node /opt/homebrew/bin/pnpm ~/Library/pnpm 2>/dev/null`)
Ran command: Check node managers in home (`ls -la /Users/jl/Library/pnpm /Users/jl/.local/share/pnpm /Users/jl/.nvm/versions/node /Users/jl/.fnm /Users/jl/.volta 2>/dev/null`)
Ran command: Check tool dirs in user home (`ls -la /Users/jl/ | grep -E "node|pnpm|nvm|cargo|rust|proto|volta|asdf|mise"`)
Viewed main.rs:1-250
Ran command: List pnpm store and node modules (`ls -la .pnpm-store node_modules`)
Viewed package.json:1-32
Viewed pnpm-lock.yaml:1-14
Ran command: Run node check-links test (`node tooling/check-links.test.mjs`)
Ran command: Run check-links tool (`node tooling/check-links.mjs`)
Ran command: Run cargo xtask package-preflight (`export PATH="/Users/jl/.cargo/bin:$PATH" && cargo xtask package-preflight`)
Ran command: Run cargo xtask codegen (`export PATH="/Users/jl/.cargo/bin:$PATH" && cargo xtask codegen`)
Ran command: Run cargo xtask fixtures (`export PATH="/Users/jl/.cargo/bin:$PATH" && cargo xtask fixtures`)
Ran command: Run cargo xtask manual-test-guard (`export PATH="/Users/jl/.cargo/bin:$PATH" && cargo xtask manual-test-guard`)
Ran command: Run cargo xtask manual-test-guard native (`export PATH="/Users/jl/.cargo/bin:$PATH" && cargo xtask manual-test-guard native`)
Ran command: Run cargo xtask manual-test-guard native with ack (`export PATH="/Users/jl/.cargo/bin:$PATH" && TETHER_MANUAL_TEST_ACK=I_UNDERSTAND_TETHER_INJECTS_INPUT cargo xtask manual-test-guard native`)
Ran command: Run cargo xtask manual-test-guard with target (`export PATH="/Users/jl/.cargo/bin:$PATH" && TETHER_MANUAL_TEST_ACK=I_UNDERSTAND_TETHER_INJECTS_INPUT TETHER_MANUAL_TEST_TARGET=macos-native cargo xtask manual-test-guard native`)
Viewed Cargo.toml:1-32
Viewed rust-toolchain.toml:1-6
Ran command: List .cargo directory (`ls -la .cargo`)
Viewed config.toml:1-3
Viewed Cargo.lock:1-67
Viewed Cargo.toml:1-13
Viewed Cargo.toml:1-10
Viewed Cargo.toml:1-11
Ran command: Check all crate manifests (`head -n 20 crates/*/Cargo.toml apps/desktop/src-tauri/Cargo.toml`)
Viewed lib.rs:1-300
Viewed lib.rs:301-415
Viewed main.rs:251-450
Viewed main.rs:451-538
Viewed protocol.md:1-250
Viewed protocol.md:251-353
Ran command: Find proto files in repo (`find proto -type f`)
Viewed buf.yaml:1-11
Viewed README.md:1-11
Viewed control.proto:1-250
Viewed control.proto:251-368
Ran command: Compile proto with protoc (`which protoc && protoc --descriptor_set_out=/dev/null --proto_path=proto proto/tether/control/v1/control.proto`)
Ran command: Check if buf is installed (`which buf || find /opt -name buf 2>/dev/null`)
Ran command: Find files in input-core (`find crates/input-core -type f`)
Viewed input-backend.md:1-224
Viewed lib.rs:1-21
Viewed action.rs:1-250
Viewed action.rs:251-400
Viewed backend.rs:1-250
Viewed backend.rs:251-272
Viewed mock.rs:1-250
Viewed mock.rs:251-397
Viewed storage.md:1-250
Viewed desktop-shell.md:1-234
Ran command: View crate lib.rs headers (`head -n 25 crates/*/src/lib.rs apps/desktop/src-tauri/src/main.rs`)
Ran command: List desktop src-tauri src (`ls -la apps/desktop/src-tauri/src/`)
Ran command: Find files in apps (`find apps -maxdepth 3`)
Ran command: Find files in tests (`find tests -type f`)
Viewed README.md:1-6
Ran command: Search test-support usages in Cargo.toml (`git grep "tether-test-support" || grep -rn "tether-test-support" Cargo.toml crates/*/Cargo.toml apps/*/src-tauri/Cargo.toml xtask/Cargo.toml`)
Viewed package.json:1-15
Viewed package.json:1-15
Viewed main.ts:1-4
Viewed main.ts:1-4
Ran command: Run frontend skeleton build command (`node --experimental-strip-types apps/desktop/src/main.ts && node --experimental-strip-types apps/mobile-web/src/main.ts`)
Ran command: Search paste in SPEC.md (`grep -n -C 3 "paste" SPEC.md`)
Ran command: Search Copy in SPEC.md (`grep -n -C 10 "Copy" SPEC.md`)
Ran command: Search fixed-point in SPEC.md (`grep -n -C 5 -i "fixed-point" SPEC.md`)
Ran command: Search natural scrolling in SPEC.md (`grep -n -C 5 "natural" SPEC.md`)
Viewed check-links.test.mjs:1-61
Ran command: Check corepack availability (`which corepack`)
Ran command: Check git repo in parent (`ls -la .. | grep -E "\.git" || true`)

# Tether Baseline Implementation Audit Report

**Audit Target:** Repository baseline prior to Task `IC-003`  
**Audit Date:** 2026-10-09  
**Auditor:** Antigravity (Independent Architecture & Implementation Audit)  
**Overall Decision:** **CONDITIONAL GO** for beginning `IC-003`

---

## 1. Executive Summary & Audit Decision

### Audit Decision: **CONDITIONAL GO**

Task `IC-003` (*Bounded serialized input dispatcher*) is technically ready to begin because its direct dependency `IC-002` is complete, and `crates/input-core` is isolated from the blocked npm dependencies (`F-002`) and Protobuf generator (`PR-002`).

However, the decision is **CONDITIONAL** upon Codex resolving four concrete constraints:
1. **Mock Observability Gap in `crates/input-core/src/mock.rs`:** `MockInputBackend` does not record `release_all` invocations in its call log, does not maintain a release call counter, and does not simulate latency on release. If no keys or buttons are held, calling `release_all()` leaves the mock in an identical observable state. IC-003 tests verifying saturation shutdown, emergency stop, or cancellation when queues contain only motion/scroll cannot assert that `release_all` was called without modifying `MockInputBackend`.
2. **Dispatcher Ledger Order vs Backend Mirror:** `MockInputBackend` uses `BTreeSet<PhysicalKey>`, which loses press order. The dispatcher ledger implemented in `IC-003` must explicitly maintain FIFO press order (and reverse press order on release) in its own ledger regardless of the mock's set representation.
3. **Continuous Verification Tooling:** `cargo xtask check` unconditionally executes `pnpm` checks and fails in the current uninstalled state. Verification during `IC-003` must use direct Cargo commands (`cargo fmt`, `cargo clippy`, `cargo test`, `cargo build`).
4. **Delta Accumulation Headroom:** When coalescing `PointerDelta` and `ScrollDelta` between barriers in `IC-003`, arithmetic must account for bounds checks against `MAX_ABSOLUTE_FIXED_DELTA` ($8,388,608$) to avoid premature rejection or integer overflow.

---

## 2. Independent Verification Log

The following checks were **independently executed** in the audit environment. They are explicitly distinguished from claims recorded in task handoffs.

| Check | Command Executed | Result | Notes |
|---|---|---|---|
| Rust Toolchain Version | `cargo --version && rustc --version` | **PASSED** | `cargo 1.99.0 (5f94df478 2026-08-27)`, `rustc 1.99.0 (b940084d7 2026-09-28) aarch64-apple-darwin` |
| Workspace Build Check | `cargo check --all-targets --workspace` | **PASSED** | All 14 crates and `xtask` checked in 0.58s with zero warnings or errors. |
| Workspace Formatting | `cargo fmt --all --check` | **PASSED** | Code formatting is strictly compliant across all Rust crates. |
| Strict Workspace Clippy | `cargo clippy --all-targets --workspace -- -D warnings` | **PASSED** | Zero warnings under `-D warnings`, `unsafe_code = "deny"`, `all = "deny"`, `pedantic = "deny"`. |
| Workspace Unit Tests | `cargo test --all-targets --workspace` | **PASSED** | **27 unit tests passed, 0 failed** (10 in `input-core`, 7 in `test-support`, 10 in `xtask`). |
| Packaging Preflight | `cargo xtask package-preflight` | **PASSED** | Verified all 8 required repository skeleton files exist. |
| Manual Test Guard | `cargo xtask manual-test-guard native` | **PASSED** | Verified guard strictly rejects missing `TETHER_MANUAL_TEST_ACK` and `TETHER_MANUAL_TEST_TARGET`. |
| Local Markdown Links | `node tooling/check-links.mjs` | **PASSED** | Verified all local links in 39 Markdown files exist and resolve without errors. |
| Protobuf Syntax Validation | `protoc --descriptor_set_out=/dev/null --proto_path=proto proto/tether/control/v1/control.proto` | **PASSED** | Schema compiles cleanly with `protoc 29.3`. |
| Frontend Skeleton Run | `node --experimental-strip-types apps/*/src/main.ts` | **PASSED** | Both desktop and mobile skeleton entrypoints run with Node.js. |
| Aggregate Check (`xtask`) | `cargo xtask check` | **FAILED** | Fails at `Command::new("pnpm")` with `os error 2` (pnpm not installed in PATH). |
| Link Checker Vitest Suite | `node tooling/check-links.test.mjs` | **FAILED** | Fails with `ERR_MODULE_NOT_FOUND` (`vitest` not installed due to blocked `F-002`). |
| Buf CLI Availability | `which buf` | **FAILED** | `buf` binary not present in environment (confirms blocker on `PR-002`). |

---

## 3. Evaluation of Completed Tasks

### F-001 — Pinned Workspaces, Skeletons, Toolchains, and Lockfiles
- **Status:** Fully Compliant.
- **Findings:**
  - `Cargo.toml` specifies `resolver = "3"`, `edition = "2024"`, `rust-version = "1.99"`.
  - Workspace manifests enforce `[workspace.lints.rust] unsafe_code = "deny"` and `[workspace.lints.clippy] all = "deny"`, `pedantic = "deny"`.
  - `Cargo.lock` (version 4) is cleanly locked with zero third-party dependencies.
  - Skeletons across `apps/desktop/src-tauri` and all 13 crates in `crates/` compile without warnings.

### F-003 — Repository Task Automation (`xtask`)
- **Status:** Compliant with Documented Blocker.
- **Findings:**
  - `xtask` correctly locates workspace root and implements `check`, `codegen`, `fixtures`, `package-preflight`, `manual-test-guard`, and `help`.
  - Tested guard isolation: Aggregate check contains zero native injection commands; native test scopes require explicit environment token `I_UNDERSTAND_TETHER_INJECTS_INPUT` and non-empty target name.
  - Preflight checks 8 essential files: [`AGENTS.md`](file:///Users/jl/tether/AGENTS.md), [`Cargo.lock`](file:///Users/jl/tether/Cargo.lock), [`Cargo.toml`](file:///Users/jl/tether/Cargo.toml), [`SPEC.md`](file:///Users/jl/tether/SPEC.md), [`apps/desktop/src-tauri/Cargo.toml`](file:///Users/jl/tether/apps/desktop/src-tauri/Cargo.toml), [`package.json`](file:///Users/jl/tether/package.json), [`pnpm-lock.yaml`](file:///Users/jl/tether/pnpm-lock.yaml), and [`rust-toolchain.toml`](file:///Users/jl/tether/rust-toolchain.toml).

### F-006 — Test Support Crate (`tether-test-support`)
- **Status:** Fully Compliant.
- **Findings:**
  - Dev-only crate providing [`FakeClock`](file:///Users/jl/tether/crates/test-support/src/lib.rs#L30-L59), [`SeededRandom`](file:///Users/jl/tether/crates/test-support/src/lib.rs#L61-L91) (`SplitMix64`), [`PrivateTempDir`](file:///Users/jl/tether/crates/test-support/src/lib.rs#L93-L165) (owner-only permissions `0700` on Unix), [`SanitizedTraceCapture`](file:///Users/jl/tether/crates/test-support/src/lib.rs#L217-L272) (bounded queue with dropped counter and poisoned lock recovery), and [`NoopOsHandle`](file:///Users/jl/tether/crates/test-support/src/lib.rs#L283-L321).
  - Used strictly as a `[dev-dependencies]` entry in [`crates/input-core/Cargo.toml`](file:///Users/jl/tether/crates/input-core/Cargo.toml#L11-L12). Zero production dependencies.

### PR-001 — Control Protocol Contract
- **Status:** Fully Compliant.
- **Findings:**
  - Contract in [`docs/components/protocol.md`](file:///Users/jl/tether/docs/components/protocol.md) fixes transport binding, envelope directions, session epoch/sequence rules, canonical USB HID physical keys (`0x07`), closed mouse buttons, closed logical/media commands, redacted text boundaries, JavaScript heartbeats, and hard ceilings.
  - Local markdown link checks validated across 39 documentation files.

### IC-001 — Input Backend Contract
- **Status:** Fully Compliant.
- **Findings:**
  - Defined in [`docs/components/input-backend.md`](file:///Users/jl/tether/docs/components/input-backend.md).
  - Strict serialization: Backends are `Send` but not `Sync`, blocking native calls restricted to dispatcher thread.
  - Idempotent release ordering specified (buttons $\to$ non-modifiers in reverse press order $\to$ modifiers in reverse press order $\to$ local state cancel).

### IC-002 — Domain Types, `InputBackend`, and `MockInputBackend`
- **Status:** Compliant with Minor Omissions.
- **Findings:**
  - Implemented in [`crates/input-core`](file:///Users/jl/tether/crates/input-core).
  - Strongly typed, bounded fixed-point deltas ([`PointerDelta`](file:///Users/jl/tether/crates/input-core/src/action.rs#L52-L90), [`ScrollDelta`](file:///Users/jl/tether/crates/input-core/src/action.rs#L101-L139)) with $1/1024$ scale and ceiling $8,388,608$.
  - Allowlisted USB HID usage IDs in [`PhysicalKey`](file:///Users/jl/tether/crates/input-core/src/action.rs#L207-L249) (`0x04..=0x65 | 0x67..=0xA4 | 0xB0..=0xDD | 0xE0..=0xE7`), excluding reserved and power keys.
  - Redaction verified: `Debug` and `Display` implementations for [`SensitiveText`](file:///Users/jl/tether/crates/input-core/src/action.rs#L265-L311), [`InputAction`](file:///Users/jl/tether/crates/input-core/src/action.rs#L361-L399), [`PointerDelta`](file:///Users/jl/tether/crates/input-core/src/action.rs#L86-L90), [`ScrollDelta`](file:///Users/jl/tether/crates/input-core/src/action.rs#L135-L139), [`MouseButton`](file:///Users/jl/tether/crates/input-core/src/action.rs#L151-L155), and [`PhysicalKey`](file:///Users/jl/tether/crates/input-core/src/action.rs#L246-L250) redact payloads unconditionally.
  - Fail-closed defaults verified on [`BackendCapabilities`](file:///Users/jl/tether/crates/input-core/src/backend.rs#L63-L85) and [`BackendStatus`](file:///Users/jl/tether/crates/input-core/src/backend.rs#L99-L113).

### ST-001 — Storage Architecture Contract
- **Status:** Fully Compliant.
- **Findings:**
  - Defined in [`docs/components/storage.md`](file:///Users/jl/tether/docs/components/storage.md).
  - Enforces per-user platform app data roots, atomic temp-file $\to$ sync $\to$ parse $\to$ rename $\to$ directory sync sequence, owner-only permissions (`0700`/`0600` on Unix, explicit DACL on Windows), lifetime `.instance.lock`, fail-closed quarantine, and segregated reset scopes.

### DT-001 — Desktop Shell Contract
- **Status:** Fully Compliant.
- **Findings:**
  - Defined in [`docs/components/desktop-shell.md`](file:///Users/jl/tether/docs/components/desktop-shell.md).
  - Explicit boundaries: Desktop webview is administrative, mobile web is not. No LAN administration routes. Single-instance plugin registered first before storage or networking. Bounded revisioned IPC commands and events. Window close hides to tray; quit executes ordered shutdown.

---

## 4. Evaluation of Blocked & Partial Tasks

### F-002 — NPM Development Tooling (BLOCKED)
- **Status:** Legitimate External Blocker.
- **Review:** Configured dependencies in [`package.json`](file:///Users/jl/tether/package.json) include `@eslint/js`, `eslint`, `markdownlint-cli2`, `prettier`, `typescript`, `typescript-eslint`, `vite`, `vitest`. Because sandbox network isolation prevents reaching the npm registry, dependencies could not be fetched and [`pnpm-lock.yaml`](file:///Users/jl/tether/pnpm-lock.yaml) has no resolved packages.
- **Impact on IC-003:** **NONE.** `crates/input-core` has zero Node/npm dependencies.

### PR-002 — Protobuf Schema & Validation Baseline (BLOCKED)
- **Status:** Legitimate External Blocker.
- **Review:** [`proto/buf.yaml`](file:///Users/jl/tether/proto/buf.yaml) and [`proto/tether/control/v1/control.proto`](file:///Users/jl/tether/proto/tether/control/v1/control.proto) are written and compile under `protoc 29.3`. However, Buf CLI 1.72.0 is not installed in the environment. `buf lint` (under `STANDARD`) and `buf breaking` (under `FILE`) have not run.
- **Impact on IC-003:** **NONE.** `crates/input-core` uses internal semantic domain types and deliberately does not depend on Protobuf or wire schemas.

### BV-001 — Early Android Browser Network Validation (BLOCKED)
- **Status:** Legitimate Hardware Blocker.
- **Review:** Requires physical Android device running current Chrome on a controlled LAN to validate Local Network Access (LNA) prompts and same-origin WebSocket upgrades before finalizing `NW-001`.
- **Impact on IC-003:** **NONE.** BV-001 is a dependency gate for LAN networking (Wave 5), not input core (Wave 3).

---

## 5. Prioritized Findings

### A. Confirmed Defects

#### Finding D-01: `MockInputBackend` Lacks Observability and Latency Simulation for `release_all`
- **Severity:** Medium
- **Location:** [`crates/input-core/src/mock.rs:197-212`](file:///Users/jl/tether/crates/input-core/src/mock.rs#L197-L212)
- **Evidence & Failure Scenario:**
  ```rust
  fn release_all(&mut self) -> ReleaseReport {
      let held = self.held_buttons.len().saturating_add(self.held_keys.len());
      let default_report = ReleaseReport {
          attempted: u32::try_from(held).unwrap_or(u32::MAX),
          released: u32::try_from(held).unwrap_or(u32::MAX),
          failures: Vec::new(),
          neutral: true,
      };
      let report = self.release_reports.pop_front().unwrap_or(default_report);
      if report.neutral {
          self.held_buttons.clear();
          self.held_keys.clear();
      }
      report
  }
  ```
  `release_all` does not invoke `self.record(...)`, does not increment `self.next_sequence`, does not increment a release counter, and does not advance `self.simulated_elapsed` or invoke `self.time_advance`. If the dispatcher triggers emergency release due to queue saturation or network timeout when no mouse buttons or keys happen to be held down (e.g. during rapid motion or scroll), calling `backend.release_all()` leaves `MockInputBackend` in an identical state: `calls()` is unchanged, `held_counts()` remains `(0, 0)`, and `is_neutral()` remains `true`. A test cannot assert whether `release_all` was called.
- **Correction:** Add a `release_call_count: usize` field and a `release_calls(&self) -> usize` getter to `MockInputBackend`. When `simulated_latency` is configured, advance `simulated_elapsed` and notify `time_advance` inside `release_all`.
- **Blocks IC-003:** **No, but must be resolved during IC-003 implementation** when writing dispatcher fault and saturation tests.

---

### B. Architectural Concerns

#### Finding A-01: `CapabilitySnapshot` Wire Schema Omits Explicit `atomic_click_available` Field
- **Severity:** Low
- **Location:** [`proto/tether/control/v1/control.proto:269-285`](file:///Users/jl/tether/proto/tether/control/v1/control.proto#L269-L285) vs [`crates/input-core/src/backend.rs:54`](file:///Users/jl/tether/crates/input-core/src/backend.rs#L54)
- **Evidence & Failure Scenario:**
  [`docs/components/protocol.md:229`](file:///Users/jl/tether/docs/components/protocol.md#L229) and [`docs/components/input-backend.md:105`](file:///Users/jl/tether/docs/components/input-backend.md#L105) require `CapabilitySnapshot` to report "supported mouse buttons and atomic click". In Rust, [`BackendCapabilities`](file:///Users/jl/tether/crates/input-core/src/backend.rs#L51-L61) has `pub atomic_click: CapabilityState`. In `control.proto`, `CapabilitySnapshot` has `bool pointer_available`, `bool physical_keys_available`, `bool vertical_scroll_available`, `bool horizontal_scroll_available`, but lacks `bool atomic_click_available`. `ProtocolLimits` has `uint32 maximum_atomic_click_count = 6;`, but wire-to-domain conversion in PR-004 will have an impedance mismatch unless `0` count is defined as unavailable or field `14` (currently in reserved `14 to 20`) is assigned.
- **Correction:** In `PR-002`, either assign `bool atomic_click_available = 14;` in `control.proto` or document that `maximum_atomic_click_count == 0` represents `CapabilityState::Unavailable`.
- **Blocks IC-003:** **No** (can be deferred to `PR-002`/`PR-004`).

#### Finding A-02: `MockInputBackend` Held Keys Storage Discards Press Order
- **Severity:** Low
- **Location:** [`crates/input-core/src/mock.rs:36`](file:///Users/jl/tether/crates/input-core/src/mock.rs#L36)
- **Evidence & Failure Scenario:**
  `held_keys` is stored as a `BTreeSet<PhysicalKey>`, which orders keys by usage ID rather than press order. [`docs/components/input-backend.md:156-157`](file:///Users/jl/tether/docs/components/input-backend.md#L156-L157) mandates that non-modifier and modifier keys be released in reverse press order. While `MockInputBackend` does not emit individual release events, tests cannot inspect press order from the mock's held state.
- **Correction:** For `IC-003`, ensure the dispatcher's held-key ledger maintains strict insertion order (`Vec` or `IndexSet`) independent of the backend's mirror.
- **Blocks IC-003:** **No** (dispatcher design requirement for IC-003).

---

### C. Tooling & Build Hygiene

#### Finding T-01: Monolithic `xtask check` Prevents Sub-System Check Execution
- **Severity:** Low
- **Location:** [`xtask/src/main.rs:177-231`](file:///Users/jl/tether/xtask/src/main.rs#L177-L231)
- **Evidence & Failure Scenario:**
  `cargo xtask check` runs 4 Cargo commands followed immediately by 7 pnpm commands. Because pnpm cannot run while `F-002` is blocked, developers cannot use `cargo xtask check` as a local gate.
- **Correction:** Add flags (e.g. `cargo xtask check --rust` or `cargo xtask check --frontend`) or gracefully report blocked prerequisites without breaking the Rust verification path.
- **Blocks IC-003:** **No** (developers can execute direct `cargo` commands).

#### Finding T-02: Missing Git Repository Initialization in Workspace Root
- **Severity:** Low
- **Location:** `/Users/jl/tether`
- **Evidence & Failure Scenario:**
  Running `git status` reports `fatal: not a git repository (or any of the parent directories): .git`. While build tools and file-checking scripts operate without git, tools like `git diff` or pre-commit hooks cannot function until git is initialized or mounted.
- **Correction:** Verify whether git is initialized in a parent tree or initialize `.git` in `/Users/jl/tether`.
- **Blocks IC-003:** **No.**

---

### D. Missing Tests & Edge Cases to Address in IC-003

1. **Delta Saturation & Coalescing Overflow:**
   In [`crates/input-core/src/action.rs:44-50`](file:///Users/jl/tether/crates/input-core/src/action.rs#L44-L50), `validate_delta` caps values at `MAX_ABSOLUTE_FIXED_DELTA` ($8,388,608$). When IC-003 coalesces consecutive motion samples between barriers, adding two large valid deltas (e.g. $5,000,000 + 5,000,000 = 10,000,000$) exceeds `MAX_ABSOLUTE_FIXED_DELTA`. IC-003 must test whether the accumulator clamps or flushes before exceeding bounds.
2. **Deterministic Monotonic Sequence Tracking:**
   `MockCall` sequences advance only on `execute()`. IC-003 must verify that the dispatcher's internal dispatch position advances monotonically across all accepted actions, including barriers and release requests.
3. **Poison Error Recovery in Dispatcher:**
   `tether-test-support` demonstrates poison recovery using `.unwrap_or_else(PoisonError::into_inner)`. The dispatcher in IC-003 must ensure mutex locks cannot permanently hang or panic on unexpected worker thread panic.

---

## 6. Categorized Action Plan

### Findings that MUST Be Handled During IC-003
1. **Enhance `MockInputBackend`:** Add `release_calls` counter to [`crates/input-core/src/mock.rs`](file:///Users/jl/tether/crates/input-core/src/mock.rs) so IC-003 tests can assert release execution when held counts are zero.
2. **Preserve FIFO Press Order in Dispatcher Ledger:** The dispatcher ledger must maintain exact press order for keys and buttons, releasing non-modifiers then modifiers in reverse order.
3. **Use Direct Cargo Commands for Gate Verification:** Do not rely on `cargo xtask check` until F-002 is unblocked; verify IC-003 using:
   - `cargo fmt --all --check`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo test --workspace --all-targets`
   - `cargo build --workspace --all-targets`

### Findings that Can Safely Be Deferred
1. **`control.proto` Atomic Click Field Alignment:** Defer to `PR-002` / `PR-004`.
2. **Buf CLI Format & Breaking Gate:** Defer to unblocking `PR-002`.
3. **NPM Dependency Installation & TypeScript/ESLint/Vitest:** Defer to unblocking `F-002`.
4. **Physical Android Probe (`BV-001`):** Defer until physical hardware is available prior to `NW-001`.

---

## 7. Conclusion

The Tether repository baseline is in **sound architectural and structural health**. All completed documentation contracts (`PR-001`, `IC-001`, `ST-001`, `DT-001`) are comprehensive, mutually consistent, and rigorously aligned with [`SPEC.md`](file:///Users/jl/tether/SPEC.md) and approved decisions (`D-01` through `D-05`). Rust workspace configuration and tests in `tether-test-support` and `tether-input-core` pass with zero warnings under strict Clippy policies.

Codex may proceed with **Task IC-003** under the condition that the test observability and queue accumulation constraints noted above are incorporated during dispatcher implementation.
