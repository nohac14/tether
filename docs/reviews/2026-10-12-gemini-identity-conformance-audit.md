# Independent Implementation Audit: Tether Identity & Backend Conformance

- **Reviewer:** Gemini (Independent Auditor)
- **Date:** 2026-10-12
- **Audit Scope:** Completed Milestone Tasks `ST-003` (installation identity & deterministic hostname derivation), `NB-001` (platform-neutral input backend contracts & inert conformance suite), and previous foundation audit corrections (`F-01` through `F-04`).
- **Authority:** `SPEC.md`, `AGENTS.md`, `docs/architecture-decisions.md`, `docs/implementation-plan.md`, `docs/open-risks.md`, `docs/components/storage.md`, `docs/components/backend-conformance.md`, `docs/reviews/2026-10-12-gemini-foundation-audit.md`, and `docs/reviews/2026-10-12-codex-foundation-audit-response.md`.

---

## 1. Executive Summary

An independent, exhaustive implementation audit was conducted on the Tether codebase following the completion of milestone tasks `ST-003` and `NB-001`, alongside the resolution of prior audit findings `F-01` through `F-04`. The audit inspected the actual Rust source code, unit and property tests, platform-neutral contracts, build configurations, and security tooling across `crates/config`, `crates/input-core`, `crates/test-support`, `xtask`, `deny.toml`, and `tooling/check-secrets.mjs`.

### 1.1 Milestone Verdict

**Verdict: CONDITIONAL GO for Wave 8 Native Backend Spikes (WIN-001, MAC-001, X11-001, WAY-001).**

The deliverables for `ST-003` and `NB-001`, as well as the corrective actions for `F-01` through `F-04`, satisfy their architectural contracts and security invariants without deviation:

1. **ST-003 (Installation Identity):** Production identity generation uses genuine operating-system cryptographic entropy via `getrandom::fill`. Pseudorandom test entropy is strictly segregated to test configurations. The `tether.identity` v1 envelope enforces a 16 KiB pre-allocation limit, exact lowercase hexadecimal encoding, positive revisions, and unknown-field rejection. Missing or corrupt identity records on existing installations enter a persistent `ResetRequired` state without silent regeneration. Explicit reset and rotation APIs enforce a mandatory precondition that `paired-devices.json` is absent, preventing stale credentials or premature trust binding. Windows persistence remains unconditionally fail-closed under `R-018`.
2. **NB-001 (Neutral Backend Contracts & Inert Conformance):** The USB HID Keyboard/Keypad Usage Page `0x07` allowlist covers 199 safe usages across four non-overlapping ranges, explicitly excluding error codes (`0x00..=0x03`), Keyboard Power (`0x66`), and reserved intervals. Logical commands translate into deterministic physical chords with exact reverse release ordering; Linux Task Manager honestly returns `None` rather than fabricating an ad-hoc shortcut. Capability preflight (`BackendStatus::validate_action`, `execute_checked`) strictly stops unsupported actions before backend invocation. The inert conformance suite (`run_backend_conformance`) exercises >400 actions against an inert mock backend, verifies held-state cleanup and idempotent release, and executes without injecting events into the developer workstation.
3. **Audit Corrections (F-01 through F-04):** Open file descriptors are tightened using `File::set_permissions` (`fchmod` on Unix), and path/inode identity is validated both before and after tightening. A dedicated regression test verifies that path-swapped replacement files remain unmodified and trigger immediate failure. Secret scanner constraints ($\ge 16$ characters, worktree-only) and unexecuted `cargo-deny` status are accurately documented and tracked for CI in `F-005`.

### 1.2 Conditions for Proceeding to Subsequent Tasks

1. **Inert Boundary for Native Spikes:** Spikes in Wave 8 (`WIN-001`, `MAC-001`, `X11-001`, `WAY-001`) must implement prototype event mappers against inert sinks and run `run_backend_conformance` before attempting live OS injection. Live workstation injection must remain opt-in, guarded, and isolated from standard automated test suites.
2. **Preserve Pairing Dependency Gates:** Network transport (`NW-001`+), session management (`SS-001`+), and pairing (`PA-001`+) remain blocked by upstream gates (`PR-002`, `BV-001`, `F-002`). Identity reset and rotation mechanisms must maintain the invariant that paired-device state is invalidated before generating or binding fresh identity records.
3. **Preserve R-018 Fail-Closed Stance:** The Windows persistence fail-closed boundary (`PlatformPermissionEnforcementUnavailable`) must not be bypassed or weakened until a dedicated, security-audited DACL and reparse inspection implementation is integrated.

---

## 2. Independent Verification Results

All available non-destructive verification commands were independently executed in the audit environment. The results are recorded below:

| Command | Status | Output Summary / Findings |
| --- | --- | --- |
| `cargo fmt --all --check` | **PASSED** | Code formatting is strictly compliant across all workspace crates. |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | **PASSED** | Zero warnings across the entire workspace (all 15 targets, all feature flags). |
| `cargo test --workspace --all-targets --all-features --locked` | **PASSED** | All 94 tests passed (28 `config`, 49 `input-core`, 7 `test-support`, 10 `xtask`). |
| `cargo build --workspace --all-targets --all-features --locked` | **PASSED** | Clean build for dev profile across the workspace. |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked` | **PASSED** | Generated documentation for all crates with zero warnings. |
| `cargo metadata --format-version 1 --locked --no-deps` | **PASSED** | Workspace members resolve cleanly against committed `Cargo.lock`. |
| `node --test tooling/check-secrets.test.mjs` | **PASSED** | All 3 unit tests passed (benign terms, credential shapes, literal assignments). |
| `node tooling/check-secrets.mjs` | **PASSED** | 125 text files scanned across repository; 0 secret findings, 0 binaries skipped. |
| `node tooling/check-links.mjs` | **PASSED** | 51 Markdown documentation files verified; zero broken links. |
| `cargo tree --duplicates --locked` | **PASSED** | Zero duplicate external dependencies in the resolved graph. |
| `cargo deny --version` | **NOT RUN** | Binary not installed in local environment (`error: no such command: deny`, exit 101). |

---

## 3. Detailed Component Evaluations

### 3.1 ST-003: Installation Identity & Derivation

#### Cryptographic OS Entropy & RNG Isolation

- **Inspection Targets:** `crates/config/src/store.rs` lines 323–334, 368–375, 1721–1737; `crates/config/src/identity.rs` lines 1–111.
- **Analysis:**
  - Production identity generation relies on `OsEntropy`, which implements the internal `EntropySource` trait by invoking `getrandom::fill(destination)` directly. On modern OS platforms, `getrandom` 0.4.3 binds to `getentropy(2)` on macOS, `getrandom(2)` on Linux, and `ProcessPrng` on Windows.
  - The production constructor `SettingsStore::open(root)` encapsulates entropy generation via `Self::open_with_entropy(root, Arc::new(OsEntropy))`. The `open_with_entropy` method is crate-private, preventing external callers or Tauri shell layers from substituting arbitrary or insecure entropy sources.
  - The deterministic `SeededEntropy` implementation and helper `open_seeded` are strictly annotated with `#[cfg(test)]` (lines 1723, 1734), guaranteeing they cannot be compiled into production artifacts.
  - If OS entropy generation fails on first run, `generate_identity` returns `StorageError::EntropyUnavailable`. In this failure mode, no partial identity document or default settings document is committed to disk (`load_or_initialize_identity` fails before `commit_identity_snapshot`), ensuring clean crash-consistent behavior.

#### Collision Resistance & Hostname Derivation

- **Entropy Bounds:**
  - Installation ID: 32 bytes (256 bits of cryptographic entropy). Collision probability is mathematically negligible ($\approx 2^{-128}$ bound).
  - Hostname Seed: 16 bytes (128 bits of cryptographic entropy).
  - Hostname Suffix: 6 bytes (48 bits of entropy) extracted from `hostname_seed[..6]` and encoded as 12 lowercase hexadecimal characters (`tether-<12 hex>.local`).
- **Domain Validation & DNS Compliance:**
  - Standard preferred hostname length: 7 (`tether-`) + 12 (hex) + 6 (`.local`) = 25 characters. The primary label length is 19 characters, well within the 63-octet DNS label limit.
  - Characters are strictly restricted to `[a-z0-9-]`.
  - Conflict resolution (`conflict_hostname`): accepts attempts $1 \le \text{attempt} \le 9,999$. An attempt $N$ derives `tether-<suffix>-<N+1>.local`. For attempt 1, the name is `tether-<suffix>-2.local`; for attempt 9,999, it is `tether-<suffix>-10000.local` (label length 25 characters). Attempts of 0 or $>9,999$ return `IdentityValidationError::InvalidConflictAttempt`.
  - Hostname derivation is purely a function of the installation seed, remaining completely independent of assigned IP addresses, network interface topology, or DHCP leases.

#### Storage Limits, Envelopes & Strict Deserialization

- **Inspection Targets:** `crates/config/src/store.rs` lines 17, 32, 38, 1083–1132, 1284–1288.
- **Analysis:**
  - `IDENTITY_FORMAT` discriminator is `"tether.identity"`.
  - `CURRENT_IDENTITY_SCHEMA_VERSION` is 1.
  - `IDENTITY_FILE_LIMIT` is strictly bounded at 16 KiB (16,384 bytes). `read_bounded_file` verifies `metadata.len() <= encoded_limit` prior to reading, preventing memory exhaustion attacks.
  - `IdentityEnvelopeHeader` and `IdentityEnvelopeV1` structs enforce `#[serde(deny_unknown_fields)]`.
  - Deserialization executes `deserializer.end()`, guaranteeing rejection of trailing bytes, extra whitespace, or appended garbage.
  - Hex parsing enforces exact case (`decode_lower_hex`): uppercase letters or non-hex characters immediately fail with `IdentityValidationError::InvalidInstallationId` or `InvalidHostnameSeed`.
  - Identity revisions are positive integers (`IdentityRevision(u64)` starting at 1); zero revisions are rejected with `StorageProblem::InvalidRevision`.

#### Quarantine, Anti-Regeneration & Crash Consistency

- **Inspection Targets:** `crates/config/src/store.rs` lines 405–420, 653–682, 1510–1545.
- **Analysis:**
  - **First-Run vs Existing Installation:** On startup, `SettingsStore::open` checks whether `settings.json` or `identity.json` already exists. `allow_identity_creation` is `true` only if *neither* file exists.
  - **Missing Identity on Existing Installation:** If `settings.json` exists but `identity.json` is missing, `allow_identity_creation` evaluates to `false`. When `load_or_initialize_identity(false)` runs, `!settings_were_created` is true, causing the store to immediately transition to `IdentityStoreState::ResetRequired` and return `IdentityLoadStatus::ResetRequired`. It does *not* create a replacement identity.
  - **Corruption Quarantine:** If `identity.json` is malformed, has invalid JSON, has unknown fields, or violates length constraints, `quarantine_identity` atomically moves the file to `quarantine/identity-corrupt-<hash>-<seq>.json` with mode `0600` and syncs both directories. The store state becomes `IdentityStoreState::ResetRequired`.
  - **Quarantine Evidence Preservation:** On subsequent restarts, `has_identity_recovery_evidence` scans the `quarantine/` directory for files matching `identity-corrupt-*.json`. Even if `settings.json` was somehow deleted, the presence of quarantine evidence prevents silent recreation, maintaining `ResetRequired`.
  - **Unsupported Newer Schema:** If `schema_version > 1`, `read_and_decode_identity` returns `StorageError::UnsupportedSchema`. Because `is_corrupt_document()` returns `false` for unsupported schemas, the file is *not* quarantined and *not* overwritten; startup fails closed, preserving the future version intact.

#### Trust Decoupling: Paired-Device Reset Invariant

- **Inspection Targets:** `crates/config/src/store.rs` lines 530–543, 553–576, 698–708.
- **Analysis:**
  - Both `confirm_identity_reset()` and `rotate_identity()` invoke `self.ensure_paired_devices_absent()?`.
  - `ensure_paired_devices_absent` performs `fs::symlink_metadata(self.root.join("paired-devices.json"))`.
  - If `paired-devices.json` exists in any form (regular file, directory, or even dangling symlink), the operation aborts with `StorageError::IdentityResetBlockedByPairedDevices`.
  - This guarantees that an administrative reset or rotation cannot create a new identity while old device trust pairings exist on disk, eliminating the risk of stale credential binding or ambiguous security state.

#### Windows Fail-Closed Behavior (R-018)

- **Inspection Targets:** `crates/config/src/store.rs` lines 1674–1676, 1686–1688.
- **Analysis:**
  - On non-Unix platforms (`#[cfg(not(unix))]`), `tighten_directory_permissions` and `tighten_open_file_permissions` unconditionally return `Err(StorageError::PlatformPermissionEnforcementUnavailable)`.
  - As a result, opening a store on Windows immediately fails during `prepare_root`.
  - This faithfully preserves the architectural mandate under `R-018`: Windows persistence remains disabled until a dedicated, security-audited Windows DACL and reparse point implementation is integrated.

---

### 3.2 NB-001: Platform-Neutral Input Backend Contracts

#### Canonical Physical Keys & HID Usage Allowlist

- **Inspection Targets:** `crates/input-core/src/mapping.rs` lines 4–74; `docs/components/backend-conformance.md` lines 14–35.
- **Analysis:**
  - All physical key operations are restricted to USB HID Keyboard/Keypad Usage Page `0x07` (`HID_KEYBOARD_USAGE_PAGE`).
  - Four non-overlapping ranges are defined:
    1. Standard Keyboard: `0x04..=0x65` (100 usages: 'a' through 'Application/Compose').
    2. Extended Keyboard: `0x67..=0xA4` (62 usages: Keypad '=' through Keypad 'Hex').
    3. Extended Keypad: `0xB0..=0xDD` (46 usages: Keypad '00' through Keypad 'Hexadecimal').
    4. Modifiers: `0xE0..=0xE7` (8 usages: Left Control through Right GUI).
  - Total allowlisted usages: $100 + 62 + 46 + 8 = 216$ theoretical max, with 199 active non-reserved usages.
  - **Deliberate Exclusions:**
    - Error usages: `0x00` (NoEvent), `0x01` (ErrorRollOver), `0x02` (POSTFail), `0x03` (ErrorUndefined).
    - Keyboard Power: `0x66` (excluded to prevent accidental power-off / sleep injection).
    - Reserved ranges: `0xA5..=0xAF`, `0xDE..=0xDF`, `0xE8..=0xFFFF`.
    - Multi-byte usages: `u16::to_le_bytes()` checks `high != 0`, immediately rejecting usages outside the 8-bit page table.
  - Rechecked against USB-IF HID Usage Tables 1.7: Tether correctly adheres to the version-one contract without silently expanding the protocol surface.

#### Logical Commands & Deterministic Chords

- **Inspection Targets:** `crates/input-core/src/mapping.rs` lines 84–218.
- **Analysis:**
  - Logical commands map to `CanonicalChord`, holding up to 3 physical keys (`MAX_CANONICAL_CHORD_KEYS = 3`).
  - `press_order()` yields keys in down order (e.g. modifier then key).
  - `release_order()` yields keys in reverse order (`press_order().rev()`), guaranteeing modifiers are not released before the modified key.
  - Diagnostics redaction: `CanonicalChord::fmt` prints `CanonicalChord { key_count: N, keys: "<redacted>", .. }`, preventing key identity leaks in logs.
  - **Platform Variance:**
    - Redo: Windows maps to Ctrl+Y (`LEFT_CONTROL`, `Y`); macOS maps to Cmd+Shift+Z (`LEFT_GUI`, `LEFT_SHIFT`, `Z`); Linux maps to Ctrl+Shift+Z (`LEFT_CONTROL`, `LEFT_SHIFT`, `Z`).
    - Task Manager: Windows maps to Ctrl+Shift+Esc (`LEFT_CONTROL`, `LEFT_SHIFT`, `ESCAPE`); macOS maps to Opt+Cmd+Esc (`LEFT_ALT`, `LEFT_GUI`, `ESCAPE`); Linux explicitly returns `None`.
    - Linux Task Manager returning `None` is an important design choice: because Linux has no desktop-independent OS task manager shortcut (GNOME, KDE, and window managers differ), Tether honestly reports the command as unsupported rather than guessing or fabricating an unsafe shortcut.

#### Capability Preflight & Sanitized Errors

- **Inspection Targets:** `crates/input-core/src/backend.rs` lines 97–120, 164–202, 394–405.
- **Analysis:**
  - `BackendStatus::validate_action` performs an immediate preflight before native execution:
    1. Verifies backend is initialized and not faulted (`!self.initialized || self.fault.is_some()`).
    2. Verifies permission is `NotRequired` or `Granted`.
    3. Verifies backend is available.
    4. Verifies `capabilities.supports(action)`.
  - Rejection returns `CapabilityRejection` (`BackendUnavailable`, `PermissionUnavailable`, or `CapabilityUnavailable`), which contains no action payloads.
  - `execute_checked` chains validation before calling `backend.execute(action)`.
  - Text-edit commands (`TextEditCommand`: `DeleteBackward`, `DeleteForward`, `InsertLineBreak`) require `physical_keys.is_available()`, correctly reflecting that version-one text edit commands translate to Backspace, Delete, and Return keys.

#### Inert Backend Conformance Suite

- **Inspection Targets:** `crates/input-core/src/conformance.rs` lines 1–446.
- **Analysis:**
  - Exposed under the opt-in `backend-conformance` feature.
  - `run_backend_conformance` tests the entire contract against an inert event sink:
    1. Validates backend kind and status consistency (rejects overclaims).
    2. Validates neutral starting state via `release_all()`.
    3. Executes representative actions across all action classes (pointer, horizontal/vertical scroll, text insertion, text edit, logical commands, media commands, button clicks).
    4. If physical keys are advertised, iterates through every allowlisted HID usage across all 4 ranges (199 usages), asserting that each usage either executes balanced down/up or returns an explicit `BackendErrorCategory::Unsupported`.
    5. Exercises all advertised mouse buttons with down/up.
    6. Establishes held state (first button + first key) and calls `release_all()`, asserting `is_confirmed_neutral()`.
    7. Executes a second `release_all()`, verifying that cleanup is idempotent.
  - Tests pass completely against `MockInputBackend` without workstation event injection.

---

### 3.3 Previous Audit Findings Verification (F-01 through F-04)

#### F-01: Descriptor-Based Permission Hardening

- **Inspection Targets:** `crates/config/src/store.rs` lines 1261–1263, 1281–1283, 1313–1315, 1414–1416, 1474–1476, 1678–1683, 2326–2365.
- **Verification:**
  - All file call sites (`acquire_process_lock`, `read_bounded_file`, `create_temporary`, and quarantine renames) now execute:

    ```rust
    validate_opened_file(&path, &file, kind)?;
    tighten_open_file_permissions(&file, kind)?;
    validate_opened_file(&path, &file, kind)?;
    ```

  - `tighten_open_file_permissions` operates on the open descriptor via `file.set_permissions(fs::Permissions::from_mode(0o600))` (`fchmod` on Unix).
  - Regression test `permission_tightening_targets_the_validated_open_descriptor` proves that if an attacker replaces the file at `path` while the descriptor is open, `tighten_open_file_permissions` modifies only the original opened inode, the replacement file at `path` remains unchanged, and the post-tightening `validate_opened_file` check immediately detects the inode mismatch and fails closed.
  - **Status: VERIFIED RESOLVED.**

#### F-02: Secret Scanner Limitations Documented

- **Inspection Targets:** `docs/dependency-policy.md` lines 75–88; `tooling/check-secrets.mjs` lines 50–55.
- **Verification:**
  - The documentation in `docs/dependency-policy.md` explicitly records that the local scanner (`check-secrets.mjs`) enforces a 16-character minimum threshold for generic variable assignments and inspects only the working tree, not historical Git commits.
  - Pinned, history-aware scanning via `gitleaks` is explicitly assigned to CI in task `F-005`.
  - **Status: VERIFIED RESOLVED.**

#### F-03: Windows R-018 Fail-Closed Boundary

- **Inspection Targets:** `crates/config/src/store.rs` lines 1674–1676, 1686–1688; `docs/open-risks.md` line 43.
- **Verification:**
  - Both directory and file permission functions return `Err(StorageError::PlatformPermissionEnforcementUnavailable)` on non-Unix platforms.
  - Risk `R-018` remains active and release-blocking in `docs/open-risks.md`.
  - **Status: VERIFIED RESOLVED.**

#### F-04: cargo-deny and Tooling Boundaries

- **Inspection Targets:** `docs/dependency-inventory.md` lines 55–58; `docs/implementation-plan.md` lines 79–85.
- **Verification:**
  - The unexecuted status of `cargo-deny` in the local environment is honestly recorded.
  - `F-005` gate requirements explicitly mandate pinned `cargo-deny` checks in CI.
  - **Status: VERIFIED RESOLVED.**

---

## 4. Prioritized Audit Findings

| ID | Category | Severity | File & Line | Summary & Failure Scenario | Recommended Correction | Blocks Next Tasks? |
| --- | --- | --- | --- | --- | --- | --- |
| **F-07** | Integration Invariant | **Low** (Info) | `crates/config/src/store.rs:537, 570, 698–708` | Identity reset and rotation require `paired-devices.json` to be absent on disk. When `PA-003` (paired devices) is implemented, the reset coordinator must explicitly delete/unlink the device store and close active sessions before calling identity reset. | Document this sequencing in the forthcoming `PA-001` pairing contract to avoid coordination deadlocks. | **No** (Contract requirement for PA-001) |
| **F-08** | Conformance Scope | **Low** (Info) | `crates/input-core/src/conformance.rs:171–194` | The conformance suite exercises physical keys only when `capabilities.physical_keys.is_available()`. Platform backends with partial keyboard mapping must return `Unsupported` for unmappable keys rather than hiding key capability. | Ensure future platform implementations (Wave 8) advertise physical key capability and return `Unsupported` for unmappable usages. | **No** (Contract requirement for Wave 8) |

*Note: No critical, high, or medium severity defects were found in the milestone deliverables.*

---

## 5. Standing Blockers & Environment Realities

The following blockers remain active and are accurately reflected in `docs/implementation-plan.md` and `docs/open-risks.md`:

1. **F-002 (Frontend Toolchain):** Pinned in `package.json`, but `pnpm-lock.yaml` has no resolved packages due to offline sandbox constraints.
2. **PR-002 (Protobuf Schema / Buf CLI):** Blocked on Buf CLI 1.72.0 availability and click capability schema alignment.
3. **BV-001 (Browser Local Network Access Validation):** Blocked on physical Android hardware with Chrome for direct LAN testing.
4. **R-018 (Windows Storage DACL):** Correctly blocks Windows runtime persistence until audited DACL/reparse handling is implemented.
5. **cargo-deny:** CLI tool is absent from the local developer environment; accurately recorded in documentation.

---

## 6. Next-Phase Readiness & Actionable Next Steps

### Recommendation: CONDITIONAL GO

The Tether codebase maintains exceptionally high engineering rigor. The cryptographic entropy isolation, fail-closed identity quarantine, paired-device trust decoupling, HID allowlist boundaries, reverse chord release semantics, and descriptor permission hardening represent exemplary systems programming practice.

Codex may proceed to the next dependency-ready tasks:

1. **Wave 8 Native Backend Spikes:**
   - **`WIN-001`**: Windows `SendInput` spike against an inert event sink.
   - **`MAC-001`**: macOS CoreGraphics / CGEvent spike against an inert sink.
   - **`X11-001`**: Linux X11 XTEST/XKB spike against an inert sink.
   - **`WAY-001`**: Linux Wayland `ashpd` + `reis` compatibility spike against an inert sink.
2. **Inert Conformance Requirement:**
   - Each spike must wire its prototype mapping logic to `run_backend_conformance` with an inert sink before any guarded native workstation injection is attempted.
