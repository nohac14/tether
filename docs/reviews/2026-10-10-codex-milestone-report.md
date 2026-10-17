# IC-003 through F-004 milestone report

- **Review boundary:** Before session, authentication, networking, or native OS
  input implementation
- **Base commit:** `abb5e5bdb9cd6fc36203e9bcbb70ac270eef4c85`
- **Date:** 2026-10-10
- **Host actually exercised:** macOS arm64

This report is the requested handoff for independent Gemini review. Work stops
at this boundary. It does not claim validation on Windows, Linux, Intel macOS,
native input facilities, mobile browsers, physical devices, signed packages,
or clean operating-system installations.

## Completed tasks and acceptance evidence

### IC-003 — bounded fail-safe input dispatcher

The dedicated dispatcher thread owns the backend exclusively, admits through a
bounded logical queue, coalesces only safe adjacent motion or same-axis scroll,
and flushes at every state transition. A separately reserved, coalescing
emergency path cannot be starved by ordinary work. Admission, cancellation,
monotonic positions, held-state ordering, invalid transitions, saturation,
backend errors and panics, inconsistent release reports, poisoned state, and
idempotent cleanup have automated coverage.

Gemini findings D-01 and C-01 are resolved as documented in
`2026-10-10-gemini-ic003-review-response.md`. In particular, a persistent
terminal-fault latch survives intermediate `Releasing` states and every
non-shutdown release; only shutdown may end at `Stopped`. Atomic click on an
already-held button is rejected before backend execution without changing the
ledger. The regression suite demonstrates the previously failing
fault/release/resume sequence, repeated releases, a release race, shutdown from
fault, valid click, and ordinary drag behavior.

### IC-004 — deterministic input normalization

The input core now provides validated fixed-point pointer and scroll
normalization, independent sensitivity and direction state, optional bounded
linear pointer acceleration, overload clamping without delayed motion,
per-axis fractional accumulation and reset, and integral native pointer and
discrete-wheel translation hooks. Seeded properties exercise 20,000 pointer
sample pairs and 20,000 alternating-axis scroll samples for bounds, direction,
monotonic acceleration, and axis isolation.

This task defines platform-neutral behavior only. Pointer acceleration, DPI,
multiple-monitor, compositor, and application-specific wheel behavior remain
native hardware validation gates.

### ST-002 — versioned atomic settings storage

The `config` crate implements the settings portion of the accepted storage
contract: strict schema-v2 envelopes, deterministic v1 migration committed
before exposure, validated settings, positive revisions and conflict checks,
bounded strict JSON decoding, lifetime process locking, create-new temporary
writes, flush/file sync/re-read/rename, parent sync where supported,
indeterminate post-replace handling, exact temporary cleanup, corrupt-file
quarantine and confirmed recovery, and payload/path-redacted diagnostics.

On Unix, tests cover `0700`/`0600` tightening, symlink rejection, opened-file
identity checks, and hard-link anomalies. Non-Unix permission setup currently
fails closed. R-018 is an explicit Windows release blocker until a reviewed
DACL/reparse implementation and real Windows 11 validation exist; inherited
ACLs are not treated as compliant.

The lockfile resolves `serde` 1.0.229 and `serde_json` 1.0.151 plus their
reviewed transitive dependencies from the official crates.io registry.

### F-004 — dependency and secret policy

The repository now contains:

- `deny.toml` with crates.io-only, no-Git, no-wildcard, license, duplicate, and
  advisory policy;
- a documented dependency, lockfile, license, advisory, and incident policy;
- a reviewed inventory of every currently resolved Rust package;
- weekly Dependabot configuration for Cargo, npm, and GitHub Actions; and
- a bounded, shell-free repository secret scanner that redacts matched values,
  plus three unit tests.

The configuration parses, the current Cargo graph uses only checksummed
crates.io packages, and the repository scan is clean. `cargo-deny` is not
installed, so no live RustSec advisory lookup is claimed. F-005 must execute
the committed policy in CI before release.

## Verification results

| Command or check | Actual result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed with no warnings |
| `cargo test --workspace --all-targets` | Passed: 72 tests total (14 config, 41 input-core, 7 test-support, 10 xtask) |
| `cargo build --workspace --all-targets` | Passed |
| `cargo doc --workspace --no-deps` | Passed |
| `cargo metadata --format-version 1 --locked --no-deps` | Passed |
| `node --test tooling/check-secrets.test.mjs` | Passed: 3 tests |
| `node tooling/check-secrets.mjs` | Passed: 119 text files, no findings |
| `node tooling/check-links.mjs` | Passed: 48 Markdown files |
| TOML/YAML/JSON configuration parsing | Passed; Dependabot contains all 3 intended ecosystems |
| `git diff --check` | Passed |

`node --test tooling/check-links.test.mjs` did not run its tests because
`vitest` is not installed. This is the existing F-002 blocker: the current
authorization covered the Rust dependencies required by ST-002, not the pinned
npm development graph. The dependency-free link checker itself passed.

## Changed files by responsibility

- **Input core:** `crates/input-core/src/backend.rs`, `dispatcher.rs`, `lib.rs`,
  `mock.rs`, and `normalization.rs`.
- **Settings storage:** `crates/config/Cargo.toml`, `src/lib.rs`,
  `src/settings.rs`, `src/store.rs`, and `Cargo.lock`.
- **Supply chain:** `deny.toml`, `.github/dependabot.yml`, `package.json`,
  `tooling/check-secrets.mjs`, `tooling/check-secrets.test.mjs`,
  `docs/dependency-policy.md`, and `docs/dependency-inventory.md`.
- **Contracts and status:** `README.md`, `docs/components/README.md`,
  `docs/components/input-dispatcher.md`,
  `docs/components/input-normalization.md`, `docs/components/storage.md`,
  `docs/implementation-plan.md`, and `docs/open-risks.md`.
- **Review record:** the supplied Gemini IC-003 review and both Codex response
  documents, plus this milestone report.

## Git state

No milestone commit was created because the managed workspace permits reading
but not writing `.git`. The base history remains unchanged and the worktree
contains all reviewed changes. No permission workaround, history rewrite, or
push was attempted. The owner should create focused local commits after this
review; exact commands are included in the accompanying handoff.

## Outstanding blockers and deferred findings

- F-002: pinned npm dependencies and the completed frontend-quality gate are
  not installed or locked.
- PR-002: Buf 1.72.0 is unavailable; its format/lint/breaking gates and the
  atomic-click wire capability resolution remain incomplete.
- BV-001: current physical Android Chrome Local Network Access and same-origin
  LAN WebSocket behavior, including active-port construction, is untested.
- R-018: Windows settings ACL/reparse/hard-link enforcement is not implemented;
  settings storage fails closed there.
- `cargo-deny`/RustSec lookup and a history-aware secret scan remain CI work.
- All native OS input, cross-platform builds, signed packages, physical-device
  behavior, pointer acceleration, DPI, and multi-monitor behavior remain
  unperformed.

There is no deviation from approved D-01 through D-05 or from the LAN-only,
browser-controller architecture. No session management, authentication,
networking, or native OS input implementation began before this review.
ST-003 is the next dependency-ready task, but it has not begun pending this
milestone review.
