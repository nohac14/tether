# Codex response to the Gemini foundation milestone audit

- **Date:** 2026-10-12
- **Input review:** `2026-10-12-gemini-foundation-audit.md`
- **Batch:** F-01/F-02 dispositions, ST-003, NB-001, and F-005 readiness
- **Boundary:** Stop for independent review before pairing, authentication,
  network input admission, or additional implementation outside this batch

## Disposition

| Finding | Disposition | Result |
| --- | --- | --- |
| F-01 | Accepted and fixed | Every already-open validated regular file now receives permission changes through its descriptor. The process lock, bounded data reads, temporary files, and quarantined files validate path/inode/link state before and after descriptor permission tightening. A Unix path-swap regression proves the opened inode changes to `0600`, a replacement at the old path remains unchanged, and the mismatch is rejected. Directory modes remain path-based because no validated file descriptor exists at those call sites. Atomic same-directory destination replacement is also path-based; symlink and hard-link checks remain fail-closed. |
| F-02 | Accepted and documented | The local scanner's generic assignment rule ignores values shorter than 16 characters and scans only the current tracked/non-ignored worktree. It does not inspect Git history, deleted blobs, or other refs. F-005 now explicitly requires pinned, redacting, history-aware CI scanning of reachable history. |
| F-03 | Tracked blocker, unchanged | Windows settings and identity persistence still fail closed with `PlatformPermissionEnforcementUnavailable`. No inherited-ACL fallback was added. R-018 remains open and release-blocking. |
| F-04 | Accepted; execution still unavailable | F-005 explicitly requires appropriately pinned cargo-deny policy, license, source, and advisory checks. `cargo deny --version` still fails because the subcommand is not installed, so no cargo-deny or network-advisory pass is claimed. |
| F-05/F-06 | Accepted as resolved | No dispatcher behavior was changed; the terminal-fault and held-button click regressions remain in the passing workspace suite. |

## ST-003 completion

ST-003 now provides:

- independent 256-bit installation identity and 128-bit hostname seed;
- production generation through `getrandom` 0.4.3's OS entropy API and
  deterministic injected entropy only in unit tests;
- a strict, distinct `tether.identity` schema-version-1 envelope with positive
  revision, exact lowercase hex encoding, and a 16 KiB encoded/read ceiling;
- stable `tether-<48-bit suffix>.local` derivation independent of IP address and
  deterministic bounded conflict candidates;
- atomic create-new/write/flush/sync/re-read/rename/directory-sync persistence;
- redacted diagnostics and the same Unix permission/path defenses as settings;
- quarantine and persistent `ResetRequired` state for corrupt, invalid legacy,
  oversized, or missing identity on an existing installation;
- unknown-newer-schema refusal without modifying the source;
- explicit reset/rotation with revision checks and a block until paired-device
  state is absent; and
- a no-document guarantee when first-run entropy generation fails.

Twenty-eight `tether-config` tests cover these outcomes plus every injected
identity commit phase, restart behavior, missing identity, paired-device reset
blocking, Unix modes, symlink/hard-link handling, and descriptor race behavior.
No Windows or Linux native storage validation ran.

## NB-001 completion

NB-001 adds:

- one explicit USB HID Keyboard/Keypad page `0x07` allowlist shared by domain
  validation and tests;
- exhaustive Windows, macOS, and Linux logical-command chord plans, with reverse
  release order and no fabricated Linux task-manager equivalent;
- redacted chord diagnostics;
- shared backend/status/action capability preflight that rejects unsupported
  input before backend execution; and
- an opt-in `backend-conformance` suite intended for production mapping code
  wired to inert event sinks.

The mock-backed suite exercises every allowlisted HID usage as balanced down/up
or an explicit unsupported mapping, every advertised button, all logical/media
commands, both scroll axes, text/edit, atomic click, unavailable-capability
gating, held-state release, and repeated idempotent release. Forty-nine
`tether-input-core` tests pass. No native mapper or native
input ran, so this establishes only the shared contract.

The HID boundary was rechecked against the current USB-IF HID Usage Tables 1.7
without silently expanding protocol version one. Windows and macOS chord plans
were checked against current Microsoft and Apple documentation. Linux mappings
remain conventions that each named native configuration must validate before
advertising.

## F-005 readiness

F-005 was not started because its F-002 dependency is incomplete. The committed
`pnpm-lock.yaml` has no resolved package snapshots, and `pnpm list --depth 0`
reports no installed root tools. A `node_modules` directory by itself is not a
resolved or verified frontend toolchain. Creating a Rust-only workflow would
violate the dependency gate and falsely imply a complete CI boundary.

When F-002 completes, F-005 must include both accepted audit conditions:

- pinned cargo-deny execution, including advisory, license, source, and ban
  policy; and
- a pinned history-aware secret scanner with sufficient checkout history and
  redacted output, in addition to the fast local worktree scanner.

## Verification results

Executed on macOS arm64 with Rust/Cargo 1.99.0:

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | Passed, zero warnings |
| `cargo test --workspace --all-targets --all-features --locked` | Passed, 94 tests (28 config, 49 input-core, 7 test-support, 10 xtask) |
| `cargo build --workspace --all-targets --all-features --locked` | Passed |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked` | Passed |
| `cargo metadata --format-version 1 --locked --no-deps` | Passed |
| `node --test tooling/check-secrets.test.mjs` | Passed, 3 tests |
| `node tooling/check-secrets.mjs` | Passed, 125 text files, zero findings |
| `node tooling/check-links.mjs` | Passed, 51 Markdown files |
| `cargo tree --duplicates --locked` | Passed; no duplicates printed |
| `cargo deny --version` | Not available; exit 101, no cargo-deny checks claimed |

The full `cargo xtask check`, TypeScript, ESLint, Prettier, Markdownlint, Vitest,
and frontend builds were not run because F-002 remains blocked. Android Chrome
LNA/WebSocket, Windows/Linux storage, every native input backend, signed package,
and physical-device validation were not run.

## Independent-review focus

The next reviewer should specifically examine:

1. first-run versus existing-installation identity creation and the persistent
   explicit-reset invariant after quarantine or deletion;
2. paired-device invalidation ordering around reset/rotation;
3. descriptor permission changes and remaining path-based directory/rename race
   assumptions;
4. HID allowlist boundaries, logical-chord semantics, and the decision to treat
   text-edit commands as requiring physical-key capability;
5. whether the conformance suite is sufficiently coupled to each future native
   mapper through its required inert event sink; and
6. the `getrandom` dependency/license inventory and preserved Windows R-018
   boundary.

No approved architecture was changed. The implementation deliberately stops at
this milestone; authentication, pairing, session management, LAN transport,
remote input admission, and native OS input remain unimplemented.

## Git record

No commit was created in this environment. A focused `git add` attempt failed
before changing the index with:

```text
fatal: Unable to create '/Users/jl/tether/.git/index.lock': Operation not permitted
```

The worktree is preserved, `.git` permissions were not changed, no bypass was
attempted, and nothing was pushed. The milestone must be reviewed from the
working tree or committed manually by the repository owner.
