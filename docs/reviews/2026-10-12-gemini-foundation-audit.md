# Independent Implementation Audit: Tether Foundation Milestone

- **Reviewer:** Gemini (Independent Auditor)
- **Date:** 2026-10-12
- **Audit Scope:** Completed Foundation Milestone (`IC-003` corrections, `IC-004` input normalization, `ST-002` settings storage, `F-004` security tooling, integration readiness)
- **Authority:** `SPEC.md`, `AGENTS.md`, `docs/architecture-decisions.md`, `docs/implementation-plan.md`, `docs/open-risks.md`, `docs/reviews/2026-10-10-codex-milestone-report.md`, and previous audit reports.

---

## 1. Executive Summary

An independent, rigorous implementation audit was performed on the Tether foundation milestone codebase. The audit inspected actual Rust source files, unit/property tests, configuration manifests, build scripts, security tooling, and documentation across `crates/input-core`, `crates/config`, `crates/test-support`, `xtask`, `deny.toml`, and `tooling/check-secrets.mjs`.

### 1.1 Milestone Verdict

**Verdict: CONDITIONAL GO for ST-003 and NB-001.**

The milestone deliverables for `IC-003` corrections, `IC-004` input normalization, `ST-002` settings storage, and `F-004` security tooling satisfy their architectural and security contracts with high fidelity:

1. **IC-003 Corrections:** Both prior findings (`D-01` terminal-fault latch reset and `C-01` held-button atomic-click mutation) are completely resolved in code. Persistent latching and transition invariants are verified by multi-scenario regression tests.
2. **IC-004 Normalization:** Implements pure fixed-point arithmetic Widened to `i64` with mathematical headroom exceeding 34,000× over worst-case bounds. Sensitivity, acceleration, per-axis fractional accumulation, symmetric cancellation, and platform-unit translation hooks are mathematically sound and verified by 40,000 seeded property-test iterations.
3. **ST-002 Settings Storage:** The atomic write protocol (create-new temp, sync_all, re-read and validate, atomic rename, directory sync) is fully implemented with strict error handling, process lifetime locking, quarantine on corruption, and strict Unix `0700`/`0600` permissions with symlink and hard-link defenses.
4. **Windows Fail-Closed Behavior (R-018):** On non-Unix platforms (`#[cfg(not(unix))]`), `ST-002` fails closed immediately with `StorageError::PlatformPermissionEnforcementUnavailable`, faithfully preventing unverified inherited ACLs from being treated as compliant.
5. **F-004 Security Tooling:** Dependency pinning and inventory documentation are accurate. `tooling/check-secrets.mjs` runs quickly, redacts secret values from reports, and passes all tests. The unexecuted status of `cargo-deny` in the local environment is honestly reported in documentation.

### 1.2 Conditions for Proceeding to ST-003

1. **Preserve R-018 Fail-Closed Stance:** `ST-003` (identity storage) must continue to use `crates/config`'s storage engine and must not weaken or bypass the fail-closed behavior on Windows prior to a dedicated, security-reviewed DACL/reparse implementation.
2. **Cryptographic RNG for Identity:** In production, installation identity generation must use a cryptographic random source (`getrandom`/OS entropy), while `SeededRandom` from `test-support` remains strictly restricted to unit tests.
3. **Independent Identity Envelope & Invariants:** Identity storage must use its own distinct envelope (`tether.identity`), enforce its 16 KiB ceiling, and implement fail-closed reset rather than silent recreation on corruption.

---

## 2. Independent Verification Results

All available non-destructive verification commands were independently executed in the audit environment. The results are recorded below:

| Command | Status | Output Summary / Findings |
| --- | --- | --- |
| `cargo fmt --all --check` | **PASSED** | Code formatting is strictly compliant across all crates. |
| `cargo clippy --workspace --all-targets -- -D warnings` | **PASSED** | Zero warnings across the entire workspace (all 15 targets). |
| `cargo test --workspace --all-targets` | **PASSED** | All 72 tests passed (41 `input-core`, 14 `config`, 7 `test-support`, 10 `xtask`). |
| `cargo build --workspace --all-targets` | **PASSED** | Clean build for dev profile across the workspace. |
| `cargo doc --workspace --no-deps` | **PASSED** | Generated documentation for all crates without warnings. |
| `cargo metadata --format-version 1 --locked --no-deps` | **PASSED** | Workspace members resolve cleanly against committed `Cargo.lock`. |
| `node --test tooling/check-secrets.test.mjs` | **PASSED** | All 3 unit tests passed (benign text, credential shapes, literal assignments). |
| `node tooling/check-secrets.mjs` | **PASSED** | 119 text files scanned across repository; 0 secret findings. |
| `node tooling/check-links.mjs` | **PASSED** | 48 Markdown documentation files verified; zero broken links. |
| `cargo deny --version` | **NOT RUN** | Binary not installed in local environment (`error: no such command: deny`). |

---

## 3. Detailed Component Evaluations

### 3.1 IC-003 Corrections Verification

#### D-01: Terminal-Fault Latch Immutability

- **Inspection Targets:** `crates/input-core/src/dispatcher.rs` lines 135–144, 429, 469, 570, 583, 658, 876, 902–911, 930.
- **Analysis:**
  - `SharedState` maintains an explicit `terminal_fault: bool` latch.
  - In `begin_emergency_locked` (line 658), `state.terminal_fault |= reason.establishes_terminal_fault()`.
  - In `complete_release` (lines 902–911):

    ```rust
    state.terminal_fault |= release_panicked || !outcome.report.is_confirmed_neutral();
    state.mode = if pending.reason == EmergencyReason::Shutdown {
        DispatcherMode::Stopped
    } else if state.terminal_fault {
        DispatcherMode::Faulted
    } else {
        pending.reason.mode_after_release(outcome.report.is_confirmed_neutral())
    };
    ```

  - Crucially, once `state.terminal_fault` becomes `true`, it is impossible for subsequent release operations (e.g. `ExplicitStop`, `ConnectionLost`, `Lifecycle`) to set `state.mode` back to `Suspended`.
  - Furthermore, `resume()` explicitly checks `if state.mode != DispatcherMode::Suspended || state.terminal_fault`, guaranteeing that admission can never resume once faulted.
  - The worker remains responsive to `request_release(EmergencyReason::Shutdown)` so that `dispatcher.shutdown()` completes without deadlock and transitions to `DispatcherMode::Stopped`.
- **Test Evidence:**
  - `terminal_fault_survives_ordinary_and_repeated_releases` (lines 1490–1546) drives the dispatcher into fault via an invalid transition, then issues successive `ExplicitStop`, `ConnectionLost`, and `Lifecycle` release requests, asserting after each that `mode == Faulted` and `terminal_fault == true`.
  - `terminal_fault_latch_survives_release_race` (lines 1549–1585) uses a blocking mock backend to race an `ExplicitStop` request while a `QueueSaturated` emergency release is executing, proving the latch survives concurrent release requests.
  - `shutdown_from_faulted_still_stops_and_joins_worker` (lines 1587–1606) confirms clean shutdown and join from the faulted state.

#### C-01: Held-Button Atomic Click Correction

- **Inspection Targets:** `crates/input-core/src/dispatcher.rs` lines 1010–1016, 818–824.
- **Analysis:**
  - In `HeldLedger::validate`:

    ```rust
    InputAction::MouseButton(ButtonAction::Click { button, .. }) => {
        if self.buttons.contains(button) {
            Err(RejectionReason::ButtonAlreadyHeld)
        } else {
            Ok(())
        }
    }
    ```

  - When `validate` returns `Err(RejectionReason::ButtonAlreadyHeld)`, `process_action` immediately invokes `schedule_internal_release(shared, EmergencyReason::InvalidTransition)`.
  - Because `InvalidTransition` establishes terminal fault, `terminal_fault` is latched, the action is marked `Rejected { reason: RejectionReason::ButtonAlreadyHeld }`, and the backend is neutralized.
  - `ledger.apply` is not called, leaving the held ledger unmodified.
- **Test Evidence:**
  - `click_on_held_button_is_rejected_without_mutating_ledger` (lines 1608–1656) submits `Down(Left)` followed by `Click(Left, 1)`. It verifies rejection with `ButtonAlreadyHeld`, proves `ledger.release_plan()` is unaltered, confirms only one call reached the backend, and verifies emergency cleanup neutralization.
  - `click_on_unheld_button_and_ordinary_drag_remain_valid` (lines 1658–1684) proves valid clicks on unheld buttons and standard drag operations (`Down` -> `PointerMove` -> `Up`) continue to execute unimpeded.

---

### 3.2 IC-004 Input Normalization & Native Translation

#### Arithmetic Model & Numerical Safety

- **Inspection Targets:** `crates/input-core/src/normalization.rs` lines 1–251.
- **Headroom Proof:**
  - Input fixed-point deltas are hard-bounded by `MAX_ABSOLUTE_FIXED_DELTA` = 8,388,608 ($\approx 8.39 \times 10^6$).
  - Sensitivity is bounded: $125 \le \text{milli} \le 8,000$ (0.125× to 8.0×).
  - Acceleration multiplier is bounded: $1,000 \le \text{milli} \le 4,000$ (1.0× to 4.0×).
  - Maximum combined gain: $8,000 \times 4,000 = 32,000,000$ ($3.2 \times 10^7$).
  - Maximum intermediate product in `FractionalGain::apply`:
    $$\text{input} \times \text{gain} \le 8,388,608 \times 32,000,000 = 268,435,456,000,000 \approx 2.68 \times 10^{14}$$
  - Since $i64::\text{MAX} \approx 9.22 \times 10^{18}$, the calculation operates with a safety margin of $\approx 34,359\times$ below integer overflow.
  - Floating-point arithmetic, NaN, and platform-specific rounding are entirely absent.

#### Pointer Acceleration

- **Curve Definition:** Linear growth from 1.0× at `threshold` to `maximum_milli` at `MAX_ABSOLUTE_FIXED_DELTA`.
- **Direction Preservation:** In `PointerNormalizer::normalize` (line 275), `speed` is computed as $\max(|x|, |y|)$. A single scalar gain is applied to both $x$ and $y$, strictly preserving the trajectory vector angle without distortion.
- **Overload Handling:** If an output delta exceeds the configured `NormalizationLimit`, it is clamped and `self.remainder` is immediately cleared to zero (line 243). This ensures discarded overload never bleeds into subsequent samples.

#### Fractional State & Axis Isolation

- **Symmetric Truncation:** Rust integer division `/` truncates toward zero, and `%` yields a remainder with the same sign as the numerator. As a result, equal and opposite sub-unit deltas cancel symmetrically with zero directional drift.
- **Axis Isolation:** In `ScrollNormalizer`, `horizontal` and `vertical` maintain separate `FractionalGain` accumulators and independent sensitivity/direction configurations. No cross-axis contamination can occur.
- **Platform Translation Hooks:** `PointerUnitTranslator` and `DiscreteScrollTranslator` wrap `NativeAxisAccumulator` to convert fixed-point units into integer steps/pixels. A zero result indicates sub-quantum accumulation rather than an error.

#### Property Test Quality

- Seeded pseudo-random tests (`SeededRandom`) run 20,000 iterations for pointer deltas (`0x5445_5448_4552`) and 20,000 iterations for alternating scroll axes (`0x5343_524f_4c4c`). Tests verify sign preservation, bound clamping, axis isolation, and independent remainders.

---

### 3.3 ST-002 Settings Storage & Persistence Safety

#### Atomic Write Protocol

- **Inspection Targets:** `crates/config/src/store.rs` lines 405–523.
- **Execution Order:**
  1. Typed payload and revision validation in memory.
  2. Bounded serialization (`serde_json::to_vec`), rejecting payloads exceeding `SETTINGS_FILE_LIMIT` (256 KiB).
  3. Destination file pre-check: verifies `validate_regular_file` (not a symlink, not a hard-link).
  4. Temporary file creation: uses `create_new(true)` with random/counter names (`.settings.tmp-<hash>-<seq>`), opened with mode `0600` on Unix.
  5. In-flight protection: `TemporaryCleanup` guard is armed on creation and unlinks the temp file on drop if any subsequent step fails.
  6. Write and durability: writes payload, calls `flush()`, then `sync_all()` on the temp file.
  7. Read-back verification: re-reads the written temporary file from disk and parses it through `read_and_decode`, asserting `candidate == *snapshot`.
  8. Atomic replacement: `fs::rename(&temporary_path, &destination)` replaces the target atomically within the same directory.
  9. Cleanup disarm: `cleanup.disarm()` is called only after successful rename.
  10. Parent directory sync: calls `sync_directory(&self.root)` (`File::open(dir)?.sync_all()`). If directory sync fails, sets `self.reload_required = true` and returns `StorageError::IndeterminateCommit`.
- **Fault Injection:** Test `pre_replace_disk_failures_preserve_the_previous_commit` injects simulated disk full failures across all five pre-replacement stages (`CreateTemporary`, `WriteTemporary`, `SyncTemporary`, `ValidateTemporary`, `Replace`) and confirms the previous commit remains intact. Test `directory_sync_failure_is_indeterminate_until_reload` verifies post-replace sync failures force a reload before allowing further mutations.

#### Permissions, Process Locking & Path Attacks

- **Unix Permissions:** Root and quarantine directories are created/tightened to `0700`; data and lock files are created/tightened to `0600`.
- **Process Locking:** `.instance.lock` is opened with `read(true).write(true).create(true)` and locked via standard library non-blocking `try_lock()`. The `File` descriptor is stored in `SettingsStore._process_lock`, ensuring the lock is held for the instance lifetime and released on drop. Test `second_store_cannot_acquire_the_lifetime_lock` verifies concurrent lock attempts fail with `StorageError::LockHeld`.
- **Symlink & Hardlink Defenses:**
  - `validate_regular_file` checks `symlink_metadata` to reject symlinks and directories.
  - `validate_opened_file` inspects the open `File` metadata and compares it against `symlink_metadata` using `same_file` (`dev` and `ino` match on Unix), preventing TOCTOU file replacement attacks.
  - `validate_link_count` enforces `nlink == 1` on Unix, rejecting hard-link anomalies (`StorageProblem::HardLinkDetected`).
- **Abandoned File Cleanup:** On open, `cleanup_abandoned_temporaries` scans the root directory and removes only regular files matching the exact regex-like format `.settings.tmp-<16 hex>-<16 hex>`, leaving non-matching files untouched (verified by `abandoned_exact_temporary_is_removed_but_similar_name_is_not`).

#### Schema Validation, Migration & Quarantine

- **Strict Parsing:**
  - `EnvelopeHeader`, `SettingsEnvelopeV1`, `SettingsEnvelopeV2`, and payload structs all specify `#[serde(deny_unknown_fields)]`.
  - `parse_json` invokes `deserializer.end()`, guaranteeing rejection of trailing data.
  - Bounded size: reads enforce `SETTINGS_FILE_LIMIT` (256 KiB) before full allocation.
- **Migrations:**
  - Schema version 1 fixtures migrate deterministically: `start_at_login` defaults to `false`, `log_level` defaults to `Info`. The migrated payload is committed to disk at `revision + 1` before being exposed to the application.
  - Schema versions greater than 2 are rejected with `StorageError::UnsupportedSchema { found, current }` and left completely untouched on disk (never quarantined, never overwritten).
- **Quarantine:**
  - Corrupt JSON, invalid envelopes, trailing data, unknown fields, or domain validation errors (e.g. port 0, interface ID > 256 bytes, duplicate interface IDs) are identified as corrupt documents.
  - `quarantine_settings` renames `settings.json` into `quarantine/corrupt-<hash>.json`, tightens permissions, syncs both quarantine and root directories, and sets store state to `StoreState::RecoveryRequired`.
  - `snapshot()` returns `None`, and mutations are blocked with `StorageError::RecoveryConfirmationRequired` until desktop user confirmation via `confirm_recovery()`.

#### Windows Fail-Closed Behavior (R-018)

- On `#[cfg(not(unix))]` (Windows):
  - `tighten_directory_permissions` and `tighten_file_permissions` unconditionally return `Err(StorageError::PlatformPermissionEnforcementUnavailable)`.
  - Consequently, any call to `SettingsStore::open` on Windows fails closed immediately during `prepare_root`.
  - This faithfully implements the architectural decision under `R-018`: Windows persistence is disabled until a dedicated, security-audited Windows DACL and reparse inspection implementation is integrated.

---

### 3.4 F-004 Security Tooling & Supply Chain

#### Dependency Pinning & Policy

- `deny.toml` is configured with:
  - Validated target triples: `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`.
  - Allowed licenses: `Apache-2.0`, `MIT`, `Unicode-3.0`.
  - Bans: `wildcards = "deny"`, `multiple-versions = "warn"`.
  - Sources: `unknown-registry = "deny"`, `unknown-git = "deny"`, `allow-registry = ["https://github.com/rust-lang/crates.io-index"]`.
- The resolved `Cargo.lock` contains only 11 external crates: `serde` 1.0.229, `serde_json` 1.0.151, and transitives (`itoa`, `memchr`, `proc-macro2`, `quote`, `serde_core`, `serde_derive`, `syn`, `unicode-ident`, `zmij`). All satisfy the license allowlist.

#### Secret Scanner (`tooling/check-secrets.mjs`)

- **Implementation:**
  - Uses `git ls-files -z --cached --others --exclude-standard` to enumerate all tracked and non-ignored files without invoking shell expansion.
  - Skips known binary extensions and files containing NUL bytes.
  - Flags text files exceeding 1 MiB (`oversized-text-file`).
  - Redaction: Scanner output formats findings as `<file>:<line>: potential secret (<rule>)`; matched secret contents are never included in output or findings data.
- **Detection Patterns:**
  - Covers private keys (`BEGIN ... PRIVATE KEY`), AWS keys (`AKIA...`), GitHub tokens (`ghp_...`, `github_pat_...`), Slack tokens (`xoxb...`), Google API keys (`AIza...`), Stripe live keys (`sk_live_...`), credential URLs (`proto://user:pass@host`), and literal assignments (`password = "..."`).
- **Execution:**
  - Unit tests run via Node test runner and pass (`3/3`).
  - Repository scan executed: 119 files scanned, 0 binary skipped, 0 findings.

---

### 3.5 Integration & Existing Blockers

The milestone code integrates cleanly into the Tether architecture without introducing regressions:

- No existing public contracts were broken.
- New exports from `tether-input-core` (`PointerNormalizer`, `ScrollNormalizer`, `PointerUnitTranslator`, `DiscreteScrollTranslator`, etc.) and `tether-config` (`SettingsStore`, `DesktopSettings`, `ApprovedInterfaceId`, etc.) are cleanly bounded.
- Workspace builds and documentation generation succeed without warnings.

The standing blockers remain accurately tracked in `docs/implementation-plan.md` and `docs/open-risks.md`:

1. **F-002 (Frontend Toolchain):** Pinned in `package.json`, but `pnpm-lock.yaml` has no resolved packages due to offline sandbox constraints.
2. **PR-002 (Protobuf Schema / Buf CLI):** Blocked on Buf CLI 1.72.0 availability and resolving the audit-confirmed atomic-click capability schema alignment.
3. **BV-001 (Browser Local Network Access Validation):** Blocked on physical Android hardware with Chrome for direct LAN testing.
4. **R-018 (Windows Storage DACL):** Correctly blocks Windows runtime persistence until audited DACL/reparse handling is implemented.
5. **cargo-deny:** CLI tool is absent from the local developer environment; accurately disclosed in documentation.

---

## 4. Prioritized Audit Findings

| ID | Category | Severity | File & Line | Summary & Failure Scenario | Recommended Correction | Blocks ST-003? |
| --- | --- | --- | --- | --- | --- | --- |
| **F-01** | Architectural Concern | **Low** | `crates/config/src/store.rs:747, 790, 1038–1042` | `tighten_file_permissions` uses path-based `fs::set_permissions` instead of `File::set_permissions` when the descriptor is already open. | In `acquire_process_lock` and `create_temporary`, pass `&file` and invoke `file.set_permissions(...)` (`fchmod`) directly. | **No** (Safe to defer) |
| **F-02** | Tooling Limitation | **Low** | `tooling/check-secrets.mjs:51–54` | Secret assignment pattern requires values $\ge 16$ characters and only scans working tree, not git history. | Document length threshold in `docs/dependency-policy.md`. Rely on CI history scanner (`gitleaks`) in F-005. | **No** (Safe to defer) |
| **F-03** | Tracked Blocker | **Low** (Info) | `crates/config/src/store.rs:1032–1047` | Windows persistence unconditionally fails closed with `PlatformPermissionEnforcementUnavailable`. | Keep tracked under R-018 until dedicated Windows ACL/reparse task implements audited DACL. | **No** (ST-003 runs on Unix/mock) |
| **F-04** | Tooling Blocker | **Low** (Info) | `docs/dependency-inventory.md:44–46` | `cargo-deny` CLI is unexecuted in local environment due to missing binary. | Install `cargo-deny` in CI workflow (F-005) and execute advisory and license gates there. | **No** (Dependencies reviewed manually) |
| **F-05** | Verified Resolution | **N/A** | `crates/input-core/src/dispatcher.rs:902–911` | D-01 terminal-fault latch reset defect is completely resolved and verified. | None required; regression tests pass. | **No** (Resolved) |
| **F-06** | Verified Resolution | **N/A** | `crates/input-core/src/dispatcher.rs:1010–1016` | C-01 held-button atomic-click mutation defect is completely resolved and verified. | None required; regression tests pass. | **No** (Resolved) |

---

## 5. Decision & Next Steps

### Recommendation: CONDITIONAL GO

The Tether Foundation Milestone represents exceptionally high-quality systems engineering. The dispatcher lifecycle latch, normalization mathematics, atomic file persistence, security boundaries, and test suites are sound and adhere strictly to `SPEC.md` and `AGENTS.md`.

Codex may proceed immediately to:

1. **ST-003 (Wave 4):** Installation identity and deterministic preferred hostname derivation, maintaining fail-closed Windows behavior and cryptographic RNG in production.
2. **NB-001 (Wave 8):** Canonical HID/logical-command mapping tables and shared backend conformance suite (dependencies `IC-001` and `PR-001` are complete).
