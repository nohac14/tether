# Independent Implementation Review: Tether Wave 8 Native Backend Spikes

Ran command: Git status check (`git status`)
Ran command: Git log check (`git log -n 10 --oneline`)
Ran command: Git diff summary (`git diff 6233f76..HEAD --stat`)
Viewed 2026-10-16-codex-identity-conformance-response.md:1-92
Viewed 2026-10-16-wave8-spike.md:1-73
Viewed native-backend-spikes.md:1-119
Viewed pairing-lifecycle.md:1-60
Ran command: Git diff docs (`git diff 6233f76..HEAD docs/components/backend-conformance.md`)
Ran command: Git diff conformance.rs (`git diff 6233f76..HEAD crates/input-core/src/conformance.rs`)
Viewed Cargo.toml:1-19
Viewed lib.rs:1-120
Viewed lib.rs:121-260
Viewed lib.rs:261-370
Viewed lib.rs:371-470
Viewed Cargo.toml:1-19
Viewed lib.rs:1-150
Viewed lib.rs:151-300
Viewed Cargo.toml:1-19
Viewed lib.rs:1-150
Viewed lib.rs:151-300
Viewed Cargo.toml:1-20
Viewed lib.rs:1-150
Viewed lib.rs:151-320
Viewed lib.rs:321-450
Viewed lib.rs:451-580
Viewed lib.rs:581-738
Ran command: Cargo fmt check (`export PATH="$HOME/.cargo/bin:$PATH" && cargo fmt --all --check`)
Ran command: Clippy check (`export PATH="$HOME/.cargo/bin:$PATH" && cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`)
Ran command: Cargo test execution (`export PATH="$HOME/.cargo/bin:$PATH" && cargo test --workspace --all-targets --all-features --locked`)
Ran command: Cargo build check (`export PATH="$HOME/.cargo/bin:$PATH" && cargo build --workspace --all-targets --all-features --locked`)
Ran command: Cargo doc check (`export PATH="$HOME/.cargo/bin:$PATH" && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked`)
Ran command: Check rustup targets (`export PATH="$HOME/.cargo/bin:$PATH" && rustup target list --installed`)
Ran command: Check Windows target (`export PATH="$HOME/.cargo/bin:$PATH" && cargo check --target x86_64-pc-windows-msvc -p tether-input-windows && cargo clippy --target x86_64-pc-windows-msvc -p tether-input-windows -- -D warnings`)
Ran command: Check X11 target (`export PATH="$HOME/.cargo/bin:$PATH" && cargo check --target x86_64-unknown-linux-gnu -p tether-input-x11 && cargo clippy --target x86_64-unknown-linux-gnu -p tether-input-x11 -- -D warnings`)
Ran command: Check Wayland target (`export PATH="$HOME/.cargo/bin:$PATH" && cargo check --target x86_64-unknown-linux-gnu -p tether-input-wayland && cargo clippy --target x86_64-unknown-linux-gnu -p tether-input-wayland -- -D warnings`)
Ran command: Tooling checks (`node --test tooling/check-secrets.test.mjs && node tooling/check-secrets.mjs && node tooling/check-links.mjs && git diff --check`)
Ran command: Duplicate dependency check (`export PATH="$HOME/.cargo/bin:$PATH" && cargo tree --duplicates --target all --locked`)
Ran command: Search unsafe (`git grep "unsafe" crates/input-macos crates/input-wayland crates/input-windows crates/input-x11`)
Viewed lib.rs:310-340
Viewed implementation-plan.md:140-165

- **Reviewer:** Gemini (Independent Auditor)
- **Date:** 2026-10-16
- **Review Scope:** Wave 8 inert native backend spikes (`WIN-001`, `MAC-001`, `X11-001`, `WAY-001`), shared inert conformance harness (`InertConformanceBackend`), pairing lifecycle contract (`F-07`), partial HID capability handling (`F-08`), and associated documentation/dependency updates.
- **Authority:** `SPEC.md`, `AGENTS.md`, `docs/architecture-decisions.md`, `docs/implementation-plan.md`, `docs/open-risks.md`, `docs/components/native-backend-spikes.md`, `docs/components/pairing-lifecycle.md`, `docs/reviews/2026-10-16-codex-identity-conformance-response.md`, and `docs/manual-tests/2026-10-16-wave8-spike.md`.

---

## 1. Executive Summary & Verdict

### **Verdict: GO for Milestone Completion of Wave 8 Inert Spikes; CONDITIONAL GO for Next Dependency-Ready Tasks**

The uncommitted Wave 8 changes implement platform-neutral native event-plan mappers, probe abstractions, and inert conformance wrappers with outstanding technical discipline. Every platform crate translates Tether domain actions into redacted native event descriptions, exercises the shared `NB-001` conformance suite through an inert event sink, and strictly enforces fail-closed semantics:

1. **Mapping Correctness & F-08 Conformance:**
   - **Windows (`tether-input-windows`):** Translates relative motion, five buttons (including XBUTTON1/XBUTTON2), horizontal/vertical wheels, scan codes with extended-key flags, media virtual keys, and UTF-16 surrogate pairs in proper order.
   - **macOS (`tether-input-macos`):** Translates continuous scroll, five buttons with click states (1 and 2), virtual keycodes, auxiliary media keys, and UTF-16 Unicode text via `objc2-core-graphics`.
   - **X11 (`tether-input-x11`):** Adheres strictly to the X11 button model (1=Left, 2=Middle, 3=Right, 4/5=Vertical scroll, 6/7=Horizontal scroll, 8=Back, 9=Forward), preflights whole-string text mapping against active layout keycodes without clipboard mutations, and handles fractional scroll residues independently per axis.
   - **Wayland (`tether-input-wayland`):** Translates evdev keys/buttons, frames every atomic batch with `WaylandEvent::Frame`, scales normalized fixed-point scroll deltas into standard EI v120 discrete units with independent per-axis residues, chunks UTF-8 text strictly at character boundaries to $\le 254$ bytes (one chunk per frame), and models `ScrollStop` and `StopEmulating` teardown plans.
   - **Partial HID Handling (F-08):** In all four mappers, keyboard interface availability is decoupled from individual HID usage mapping. Unmapped allowlisted usages return `BackendErrorCategory::Unsupported` before sink execution, preserving physical key capability without overclaiming support for unmapped keys.

2. **Partial Failures & Cleanup Safety:**
   - The shared `InertConformanceBackend` manages backend-side held buttons and keys. On `release_all`, buttons release in reverse order, followed by non-modifier keys in reverse order, with modifier keys released last.
   - If an event sink reports a partial execution failure ($0 < \text{accepted} \le \text{total}$), `InertConformanceBackend` latches `uncertain_native_state = true` and returns `PartialStateChange`. Subsequent `release_all` calls include `ReleaseFailure::BackendLocalState` and cannot report a confirmed neutral state. This integrates seamlessly with the `IC-003` dispatcher's terminal fault latch.

3. **Dependency Selection & Local Code Hygiene:**
   - Selected dependencies (`windows` 0.62.2, `objc2-core-graphics` 0.3.2, `x11rb` 0.14.0, `ashpd` 0.13.13, `reis` 0.7.1) are pinned with narrowed features and resolve against crates.io.
   - There are **zero local `unsafe` blocks** across all four input crates.
   - Cross-compilation targets (`x86_64-pc-windows-msvc` and `x86_64-unknown-linux-gnu`) compile cleanly under strict Clippy (`-D warnings`).

4. **Honesty of Completion Claims:**
   - Task completion claims in `docs/implementation-plan.md` faithfully match the evidence: only `MAC-001` is marked `DONE` because it was compiled and executed on an actual macOS host (running `CGPreflightPostEventAccess`). `WIN-001`, `X11-001`, and `WAY-001` are explicitly kept as `BLOCKED` pending execution on actual native host environments. Cross-compilation is correctly not conflated with native runtime evidence.

---

## 2. Independent Verification Results

All non-destructive verification commands were independently executed in the audit environment:

| Command | Status | Output Summary / Findings |
| --- | --- | --- |
| `cargo fmt --all --check` | **PASSED** | Code formatting is strictly compliant across all workspace crates. |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | **PASSED** | Zero warnings across the entire workspace (all targets and features). |
| `cargo test --workspace --all-targets --all-features --locked` | **PASSED** | All 118 workspace tests passed (28 `config`, 49 `input-core`, 6 `input-macos`, 7 `input-wayland`, 5 `input-windows`, 6 `input-x11`, 7 `test-support`, 10 `xtask`). |
| `cargo build --workspace --all-targets --all-features --locked` | **PASSED** | Clean build for dev profile across the workspace. |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked` | **PASSED** | Generated documentation for all crates with zero warnings. |
| `cargo check --target x86_64-pc-windows-msvc -p tether-input-windows` | **PASSED** | Windows binding surface and mapper compile cleanly. |
| `cargo clippy --target x86_64-pc-windows-msvc -p tether-input-windows -- -D warnings` | **PASSED** | Zero Clippy warnings for Windows x86-64 target. |
| `cargo check --target x86_64-unknown-linux-gnu -p tether-input-x11` | **PASSED** | Linux X11 binding surface and mapper compile cleanly. |
| `cargo clippy --target x86_64-unknown-linux-gnu -p tether-input-x11 -- -D warnings` | **PASSED** | Zero Clippy warnings for Linux X11 x86-64 target. |
| `cargo check --target x86_64-unknown-linux-gnu -p tether-input-wayland` | **PASSED** | Linux Wayland binding surface (`ashpd` + `reis`) compiles cleanly with required `screencast` feature. |
| `cargo clippy --target x86_64-unknown-linux-gnu -p tether-input-wayland -- -D warnings` | **PASSED** | Zero Clippy warnings for Linux Wayland x86-64 target. |
| `cargo tree --duplicates --target all --locked` | **PASSED** | Only `syn` 2.0.119 and 3.0.7 exist as duplicates across macro dependencies (documented). |
| `node --test tooling/check-secrets.test.mjs` | **PASSED** | All 3 unit tests passed. |
| `node tooling/check-secrets.mjs` | **PASSED** | 130 text files scanned; 0 secrets found, 0 binaries skipped. |
| `node tooling/check-links.mjs` | **PASSED** | 56 Markdown documentation files verified; zero broken links. |
| `git diff --check` | **PASSED** | Zero whitespace or formatting errors. |
| `git grep unsafe crates/input-*` | **PASSED** | Zero local unsafe blocks across all input crates. |
| `cargo deny --version` | **NOT RUN** | Binary not installed (`error: no such command: deny`, exit 101). |

---

## 3. Deep Technical Evaluation

### 3.1 Mapping Correctness & F-08 Conformance

#### Windows (`crates/input-windows/src/lib.rs`)

- **Relative Pointer:** Emits `WindowsEvent::RelativeMotion { dx, dy }`.
- **Mouse Buttons:** Translates `Left` (0x0002/0x0004), `Right` (0x0008/0x0010), `Middle` (0x0020/0x0040), `Back` (0x0080/0x0100 with `data: 1`), and `Forward` (0x0080/0x0100 with `data: 2`). Click actions generate sequential down/up pairs.
- **Scroll Wheels:** Emits `WindowsEvent::Wheel { horizontal, delta }`.
- **Physical Keys & Scan Codes:** `hid_to_scan_code` maps standard keys and navigation clusters, correctly applying the `extended` flag to arrow keys (`0x4D`, `0x4B`, `0x50`, `0x48`), Right Ctrl (`0x1D`), Right Alt (`0x38`), Left/Right GUI (`0x5B`, `0x5C`), and Keypad Enter (`0x1C`).
- **Unicode & Surrogate Pairs:** `map_unicode` converts strings to UTF-16 code units via `.encode_utf16()`. Unit test `unicode_supplementary_scalar_emits_ordered_surrogate_pairs` proves that supplementary characters (e.g. emoji 😀, `U+1F600`) produce ordered surrogate pairs `[0xD83D, 0xDE00]`, emitting down and up events for each surrogate unit.
- **Media & Logical Commands:** Maps standard Windows virtual key codes (Play/Pause: `0xB3`, Next: `0xB0`, Prev: `0xB1`, VolUp: `0xAF`, VolDown: `0xAE`, Mute: `0xAD`). Logical chords map via `canonical_logical_chord(Windows)`.
- **F-08 Compliance:** `WindowsMapper::from_probe` advertises `physical_keys: Available`. When an unmapped allowlisted key (e.g. `0xB0`) is submitted, `map_key` returns `Err(unsupported(PhysicalKey))`. Test `unmappable_hid_is_explicit_without_hiding_keyboard_capability` proves this behavior.

#### macOS (`crates/input-macos/src/lib.rs`)

- **Relative Pointer:** Emits `MacEvent::RelativeMotion { dx, dy }`.
- **Mouse Buttons:** Maps buttons 0 (Left), 1 (Right), 2 (Middle), 3 (Back), and 4 (Forward). Double-click actions set `click_state` sequentially to 1 and 2.
- **Scroll:** Emits `MacEvent::Scroll { horizontal, delta, continuous: true }`.
- **Physical Keys & Keycodes:** `hid_to_key_code` maps ANSI standard keys to Carbon/Quartz virtual keycodes (`kVK_ANSI_A = 0x00`, `kVK_Control = 0x3B`, `kVK_Command = 0x37`, `kVK_Option = 0x3A`, etc.).
- **Text & Media:** UTF-16 code units are emitted in `MacEvent::Unicode`. Media keys map to system-defined auxiliary keys (Play: 16, Next: 17, Prev: 18, SoundUp: 0, SoundDown: 1, Mute: 7).
- **Accessibility Preflight:** On macOS, `platform_probe()` invokes `CGPreflightPostEventAccess()` without prompting. If false, it assigns `PermissionState::PromptRequired` and leaves capabilities empty.
- **F-08 Compliance:** Keyboard capability remains available while unmapped allowlisted keys return `Unsupported`.

#### X11 (`crates/input-x11/src/lib.rs`)

- **Button Numbering:** Uses the canonical X11 button assignment: Left = 1, Middle = 2, Right = 3, Back = 8, Forward = 9. Scroll buttons use 4 (Up), 5 (Down), 6 (Left), and 7 (Right).
- **Fractional Scroll Accumulation:** `map_scroll` accumulates fixed-point amounts against `vertical_remainder` and `horizontal_remainder`. Each multiple of `FIXED_POINT_SCALE` emits one down/up button event. Overload ($> 64$ steps) returns `InvalidPlatformState`.
- **Text Preflight:** `map_text` preflights every character against the active layout keycode map (`text_keycodes`). If any character cannot be mapped, or if shifted characters require a shift key that is unavailable, the entire action fails with `Unsupported` before producing any events.
- **Residue Reset:** `clear_fractional_scroll()` resets remainders to 0. `reset_local_state()` wires this to the conformance harness.

#### Wayland (`crates/input-wayland/src/lib.rs`)

- **Evdev Mappings:** Button codes map to standard Linux input definitions (`BTN_LEFT = 0x110`, `BTN_RIGHT = 0x111`, `BTN_MIDDLE = 0x112`, `BTN_SIDE = 0x116`, `BTN_EXTRA = 0x115`). Keycodes map to standard Linux `KEY_*` constants via `hid_to_evdev`.
- **EI Framing:** In compliance with the libei specification, every discrete interaction batch is terminated with `WaylandEvent::Frame`.
- **EI Scroll Units:** `map_scroll` translates Tether's fixed-point units into standard EI v120 discrete units:
  $$\text{v120} = \frac{\text{fixed\_amount} \times 120 + \text{remainder}}{\text{FIXED\_POINT\_SCALE}}$$
  The calculation uses `i64` arithmetic and retains the modulo remainder per axis. Releasing or neutralizing input resets the residues.
- **EI Text Chunking:** `map_text_chunks` bounds each UTF-8 chunk to `MAX_EI_TEXT_BYTES` (254 bytes), strictly splitting on character boundaries (`text.is_char_boundary(end)`). Each chunk is emitted as `WaylandEvent::TextUtf8` followed immediately by `WaylandEvent::Frame`, ensuring no malformed UTF-8 or multi-request frames reach the compositor.
- **Teardown Plans:** `scroll_stop_plan` and `stop_emulating_plan` provide explicit sequences for clean session termination.
- **Device Lifecycle:** `apply_device_lifecycle` transitions the probe to `Paused` or `Removed`, immediately dropping capabilities and failing closed.

---

### 3.2 Partial Failures & Held-Input Cleanup

The implementation of `InertConformanceBackend` in `crates/input-core/src/conformance.rs` provides a robust, reusable validation harness:

1. **Held-State Tracking:** Maintains explicit vectors of `held_buttons` and `held_keys`.
2. **Reverse Release Order:** During `release_all()`, buttons are released in reverse chronological order. Keys are released in reverse order, with non-modifier keys released first and modifier keys released last.
3. **Uncertain Native State Latch:**

   ```rust
   let category = if error.accepted() == 0 {
       BackendErrorCategory::NativeCallFailed
   } else if error.accepted() <= events.len() {
       self.uncertain_native_state = true;
       BackendErrorCategory::PartialStateChange
   } else {
       self.uncertain_native_state = true;
       BackendErrorCategory::Fatal
   };
   ```

   If a sink fails after partially accepting an event batch, `uncertain_native_state` is set to `true`. In `release_all()`, an uncertain native state causes `ReleaseFailure::BackendLocalState` to be reported, preventing `neutral` from being asserted.
4. **Dispatcher Integration:** Because `ReleaseReport::is_confirmed_neutral()` returns `false` when `uncertain_native_state` is true, the dispatcher latches `terminal_fault = true` and transitions to `DispatcherMode::Faulted`, guaranteeing that control admission cannot resume after an unverified partial native injection.

---

### 3.3 Dependency Choices & Platform Safety

- **Windows:** Pinned `windows` 0.62.2 with `default-features = false` and features `["std", "Win32_UI_Input_KeyboardAndMouse"]`. Compiles with zero warnings.
- **macOS:** Pinned `objc2-core-graphics` 0.3.2 with `default-features = false` and minimal Quartz features. Replaces ad-hoc FFI with maintained Rust bindings.
- **X11:** Pinned `x11rb` 0.14.0 with `default-features = false` and features `["xkb", "xtest"]`. Pure-Rust X11 client protocol implementation avoids C library linking and distribution drift.
- **Wayland:** Pinned `ashpd` 0.13.13 (`remote_desktop`, `screencast`, `tokio`) and `reis` 0.7.1 (`tokio`). Enabling `ashpd/screencast` was strictly necessary because `ashpd`'s RemoteDesktop API compiles types from screencast internally.
- **Dependency Graph:** `Cargo.lock` has no duplicate crates except for `syn` 2.0.119 and 3.0.7, which are transitively required by proc-macro dependencies (`windows` macros vs `zbus`/`ashpd`). This is documented in `docs/dependency-inventory.md`.

---

### 3.4 Verification of Prior Finding Dispositions

#### F-07 (Identity Reset / Paired-Device Coordination)

- `docs/components/pairing-lifecycle.md` defines the exact coordinator sequence:
  1. Close pairing and control admission.
  2. Terminate active/standby sessions and perform bounded emergency release.
  3. Invalidate credentials and delete `paired-devices.json`.
  4. Confirm `paired-devices.json` is absent on disk under the storage lock.
  5. Invoke `confirm_identity_reset()` or `rotate_identity()`.
  6. Recreate an empty device store only after successful identity commit.
- **Verdict: Properly resolved as an architectural contract prerequisite for `PA-001`.**

#### F-08 (Partial HID Allowlist Mappings)

- Evaluated across all four platform crates: general physical key capability is advertised while unmapped allowlisted usages return `Unsupported` before reaching the event sink.
- Conformance tests verify that all allowlisted usages either execute a balanced down/up or return `Unsupported`.
- **Verdict: Fully resolved and verified by automated tests.**

---

## 4. Prioritized Findings & Observations

| ID | Category | Severity | File & Line | Summary & Detail | Recommended Action | Blocks Next Tasks? |
| --- | --- | --- | --- | --- | --- | --- |
| **F-09** | Architecture Checkpoint | **Medium** | [`crates/input-wayland/Cargo.toml:16`](file:///Users/jl/tether/crates/input-wayland/Cargo.toml#L16), [`docs/open-risks.md:34`](file:///Users/jl/tether/docs/open-risks.md#L34) | `reis` 0.7.1 is pinned for the Wayland spike, but upstream documentation notes the library is incomplete and subject to API changes. | Treat `reis` strictly as a prototype spike dependency. Enforce the `WAY-002` architecture decision checkpoint (evaluate `reis` vs isolated `libei` FFI) on real GNOME/KDE hardware before committing to production. | **No** (Blocks WAY-002/003, does not block MAC/WIN/X11) |
| **F-10** | Testing Boundary | **Low** (Info) | [`docs/components/backend-conformance.md:88–92`](file:///Users/jl/tether/docs/components/backend-conformance.md#L88-L92) | Future production backends (`MAC-002`, `WIN-002`, etc.) will wire native event posting. Accidental native injection during ordinary `cargo test` could manipulate developer workstations. | Maintain the rule that default `cargo test` runs only against inert sinks. Native workstation injection must remain strictly opt-in behind explicit manual guards (`xtask test-manual`). | **No** (Contract requirement for Wave 8 Phase 2) |
| **F-11** | Platform Gaps | **Low** (Info) | [`docs/manual-tests/2026-10-16-wave8-spike.md:57–70`](file:///Users/jl/tether/docs/manual-tests/2026-10-16-wave8-spike.md#L57-L70) | Native host tests for Windows `SendInput`, X11 server connection, and Wayland portal/EI are not run due to host environment constraints. | Accurately retain `WIN-001`, `X11-001`, and `WAY-001` as `BLOCKED` in `docs/implementation-plan.md` until physical/VM test environments are provisioned. | **No** (Accurately documented) |

*Zero critical or high severity defects were identified.*

---

## 5. Next-Phase Readiness & Actionable Next Steps

### Recommendation: GO for Milestone Acceptance; CONDITIONAL GO for Next Tasks

The completed Wave 8 work satisfies all architectural requirements and quality standards established in `SPEC.md` and `AGENTS.md`.

#### Readiness for Next Tasks

1. **`MAC-002` (Production macOS Backend):**
   - **Status:** **UNBLOCKED / READY**.
   - `MAC-001` is complete and verified natively on macOS arm64. Implementation of `MAC-002` (serialized execution context, Accessibility preflight, Local Network metadata, and event construction) may begin.
2. **`WIN-001`, `X11-001`, `WAY-001` (Native Platform Spikes):**
   - **Status:** **BLOCKED** pending physical/VM test environments with real Windows 11, X11, and GNOME/KDE Wayland compositors. The inert event-plan mappers are complete, but native observation criteria must be validated before marking the tasks `DONE`.
3. **`PA-001` (Pairing Lifecycle Contract):**
   - **Status:** **BLOCKED** by upstream dependency `PR-002` (Protobuf schema / Buf CLI). When `PR-002` is unblocked, `PA-001` can proceed using the sequencing established in `docs/components/pairing-lifecycle.md`.
4. **`F-005` (CI Infrastructure):**
   - **Status:** **BLOCKED** by `F-002` (frontend toolchain / pnpm package resolution). When pnpm packages are resolved, `F-005` must integrate pinned `cargo-deny` policy checks and history-aware `gitleaks` scanning.
