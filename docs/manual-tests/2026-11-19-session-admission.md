# Session admission checkpoint — 2026-11-19

## Completed slice

Finished SS-002's transactional connection-local admission cursor. It stages
whole validated envelopes without mutation, commits only against matching cursor/
revision/current facts, tracks queued held-state projections, preserves monotonic
counters, and fences ownership/release barriers. It cannot inject input, verify
credentials, grant ownership or resume a dispatcher. The full actor task remains
IN PROGRESS. See the [contract](../components/session-admission.md).

The new dependencies are internal input-core/protocol paths and dev-only existing
prost 0.14.6/test-support. No new registry version or production crypto dependency
is introduced. Private instance numbers are checked process-local transaction
identifiers, not a replacement for OS-backed installation/pairing entropy.

Eleven component tests passed, including a 10,000-step seeded projection model,
parallel private-ID allocation, rollback, stale facts/preparations, modifier/key/
button state, barrier handling, exhaustion and redaction. Initial strict Clippy
identified the pinned Rust compiler's renamed atomic update API; the code uses
`try_update`, with no warning suppression. Component formatting, strict all-target/
all-feature Clippy, tests and build passed before final repository verification.

## Finalization gates

Actual verification on the Apple Silicon Mac with Rust 1.99.0, Node 24.19.0,
pnpm 11.25.0 and Buf 1.72.0:

- Full worktree: formatting, strict workspace/all-target/all-feature Clippy,
  locked offline tests (259 Rust) and builds passed. `cargo xtask check` passed,
  including 115 Vitest/four Node tests, types, ESLint, Prettier, Markdown, links,
  protocol policy, offline generated-code reproducibility and foundation builds.
- Isolated staged snapshot: the same all-feature Rust gates and aggregate passed,
  with 163 Rust, 106 Vitest and four Node tests. Its Cargo-generated lock delta
  adds only the session crate's internal/existing dependencies. Older unstaged
  platform/network work remains preserved outside this focused commit, accounting
  for the different test counts.
- `cargo check -p tether-session --all-targets --all-features --target
  x86_64-pc-windows-msvc --locked --offline` and no-dependency session rustdoc passed.
  This is compilation/documentation evidence, not Windows linking or execution.
- The first aggregate used a bundled pnpm launcher's Node 24.22.1 despite PATH.
  After identifying that launcher, both aggregates were rerun successfully with
  the exact pinned Node/pnpm via a task-local launcher. No repository pin, global
  configuration or security check was changed to accommodate that mismatch.
- The worktree scanner and a bounded supplemental scan of staged blobs and all
  unpublished Git-history blobs passed with no known-pattern findings. This is
  not a guarantee of secret absence or completed F-005 history-aware CI.
- Pinned cargo-deny 0.20.2, using the existing advisory cache with `--offline`,
  exited 1: RUSTSEC-2026-0370/0371/0372 remain on zvariant/zbus_names/
  zvariant_derive. Bans, licenses and sources passed; duplicate warnings remain
  visible. No waiver, suppression or fresh-online-advisory claim is made.

The explicit synthetic fixture writer remains intentionally excluded from normal
verification and was executed separately on November 16; no safety test is skipped.
Actual Windows runtime, native input, physical phone, signed-package and remote
CI validation have not run. R-018 storage security, R-019 dependency advisories
and R-023 Windows SDK provenance remain open. No runnable Windows application
or executable is claimed by this checkpoint.
