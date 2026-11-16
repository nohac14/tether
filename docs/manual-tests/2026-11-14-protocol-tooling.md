# Windows-first protocol/tooling checkpoint — 2026-11-14

## Scope and authority

ADR 0004 implements the owner's Windows 11 x86-64 first priority, Mac second,
Linux deferred. Microsoft SDK license/pinned tool acquisition, four exact
minimatch overrides and one generated capability-message Clippy expectation
were explicitly owner-approved. R-018 native storage validation remains required.
No native input, prompt, capture, firewall change, production LAN listener,
Git permission change or push occurred in this batch.

Host: existing Apple Silicon macOS 15.7.5, Rust/Cargo 1.99.0, Node 24.19.0,
pnpm 11.25.0. Node's official archive digest was checked before use. Tool caches,
SDK manifests, downloads and build artifacts remain outside versioned source.

## Completed implementation and checks

- **F-007:** exactly the four approved parent overrides resolve minimatch
  11.0.1, plus nonrecursive-glob-match 1.1.0. Direct pins/prior overrides remain
  unchanged. Source/license review, five compatibility regressions, frozen
  scripts-disabled install, clean npm audit and the pre-codegen aggregate pass.
  That aggregate ran 214 Rust, 29 Vitest and four Node scanner tests. R-022 closed.
- **PR-002:** explicit atomic-click capability field 21, reserved 14–20 retained;
  official checksum/version-verified Buf 1.72.0; frozen FILE baseline; bounded
  local format/lint/build/breaking gate. Five regressions prove deleted field
  rejection, additive compatibility, baseline stability and fatal tool errors.
- **PR-003 implementation:** pinned prost family 0.14.6, syn 3.0.7 and
  protobuf-es 2.17.0; generated Rust/TS present; offline scratch generation/diff;
  exhaustive redacted Rust diagnostics; explicit output/bounds/symlink checks;
  full-width counters, UTF-8, atomic capability, malformed wire and unknown
  enum round trips. Does not implement PR-004 domain validation or PR-005 goldens.
  The original generated output failed strict Clippy. Rustdoc formatting and
  must-use annotations were fixed in the generator; only the owner's single
  expectation for the mandated capability booleans was added. No broad waiver.

Final local verification:

| Check | Actual result |
| --- | --- |
| `cargo xtask check` | Passed: formatting, strict workspace/all-target Clippy, 220 Rust tests, workspace/all-target builds, types, Buf format/lint/build/FILE, offline generated diff, ESLint, Prettier, Markdown/links, 38 Vitest tests, four Node scanner tests and current skeleton frontend builds |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | Passed with the one approved generated-wire expectation |
| `cargo test` / `cargo build --workspace --all-targets --all-features --locked` | Passed, 220 Rust tests; non-posting Quartz construction had WindowServer access |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked` | Passed |
| `cargo test --doc --workspace --all-features --locked` | Passed, one compile-fail discovery doc test |
| `cargo clippy --target x86_64-pc-windows-msvc --workspace --all-targets --all-features --locked -- -D warnings` | Passed cross-target static compilation, not Windows linking/execution |
| Frozen scripts-disabled pnpm install / full npm audit | Passed / zero known findings |
| `pnpm check:secrets` / `git diff --check` | Passed; 210 worktree text files, zero binary skips / clean whitespace |
| Markdown/local links | Passed across 89 Markdown files |

Worktree outputs are not a Git commit: `.git` is read-only in this managed
environment and HEAD remains `94d8e49`. No bypass or destructive Git operation
was attempted. PR-003's implementation/verification is complete, but the task
is not marked DONE before its generated-source commit criterion is satisfied.

## Windows build-tool evidence and acquisition blocker

- **cargo-xwin 0.23.1:** built using standard locked official Cargo into a
  private task directory; version check passed. No project dependency added.
- **LLVM 23.1.5 Mac ARM64:** official release archive downloaded and hash matched
  `aba592b4a15e8f90c634c5367b51ec8418b8bcd2829865db331a17dc9418dcff`.
  GitHub CLI 2.102.0 verified the publisher attestation for llvm/llvm-project,
  tag llvmorg-23.1.5 and the same subject digest. Initial Sigstore initialization
  failed in the sandbox; standard trust-cache access resolved it without a
  changed trust root/TLS setting. Archive extraction followed verification.
  clang and lld report 23.1.5; llvm-rc help executes. No Windows code ran.
- **Microsoft manifests:** the two official channels were read via HTTPS, then
  each advertised payload URL downloaded once. Both mismatched; no SDK
  installation/cross-link followed. xwin 0.10.0 source explicitly omits the
  package-manifest digest check; that omission is not an accepted substitute.

| Official channel | Advertised manifest SHA-256 | Observed SHA-256 | Advertised / observed bytes |
| --- | --- | --- | ---: |
| `https://aka.ms/vs/17/release/channel` | `da49c304df0fff4e75c20849171840a06840e845ce814104d709dfb5cd461668` | `d1d8109899558618035724949673f69f8c1b5704f542a2e76f31d87e425d5710` | 30,447,538 / 17,959,942 |
| `https://aka.ms/vs/18/stable/channel` | `b105af567bb71794076ad73e7505a3a3bd9b012c5d9e1c32ed609413da085ad0` | `c891dbc44df0b4b372ad802eb0bf590d2320c4cc09d0ea35575ad6fd7e755311` | 31,963,745 / 18,805,635 |

Payloads were the corresponding `VisualStudio.vsman` on
`download.visualstudio.microsoft.com`, with the advertised digest in their URL.
R-023/WB-001 remain blocked until authenticated matching vendor provenance or
a reviewed native Windows build environment is available. No bypass, guessed
CRT/SDK pin, NSIS installation, PE smoke artifact or finished executable is claimed.

## Security/dependency checks

Regenerated lockfiles use standard registry integrity checks. New Rust packages
and npm runtime/development boundaries are listed in the dependency inventory.
The exact foldhash 0.1.5 Zlib development license was reviewed before accepting
its license exception; advisory policy/ignores and the global allowlist stay intact.

- npm full audit: zero known findings with the 230-entry graph.
- cargo-deny 0.20.2 restored from official locked Cargo.
- Full graph: **failed**, exit 1, solely RUSTSEC-2026-0370/0371/0372; bans,
  licenses and sources pass. R-019 remains open. Initial unreviewed Zlib
  rejection was resolved by exact source/license disposition, not suppression.
- Individual `--target x86_64-pc-windows-msvc` and
  `--target aarch64-apple-darwin`: advisories/bans/licenses/sources pass.
  Visible getrandom/syn/hashbrown duplicates were reviewed, not hidden.
  These scoped results do not supersede the full-graph failure or prove native
  Windows runtime/input/security behavior.

## Not executed / limitations

No Windows runtime, native Windows storage attacks, live SendInput, actual app
gesture/keyboard/phone session, Mac input posting, installer, signing, new
physical phone check, remote CI or Linux migration. Current frontend builds
are foundation skeleton commands, not a completed Vite/React/Tauri application.
R-018/R-019/R-023 and the existing native/browser risks remain visible.

## Commit handoff

Review the worktree first; substantial Mac/network changes predate this batch
and must not be swept into a tooling commit. From the owner's terminal:

```sh
cd /Users/jl/tether
git status --short
git diff -- pnpm-workspace.yaml tooling/glob-toolchain.test.mjs
git add -- pnpm-workspace.yaml tooling/glob-toolchain.test.mjs docs/reviews/2026-11-14-npm-tooling-findings.md
git add -p -- pnpm-lock.yaml docs/dependency-policy.md docs/dependency-inventory.md docs/open-risks.md docs/implementation-plan.md
git diff --cached --check
git diff --cached
git commit -m "fix(tooling): apply reviewed scoped minimatch remediation"
```

Select only F-007 hunks in shared files for that commit; leave unrelated
earlier changes and PR-003 dependency/generation hunks unstaged. For the next
protocol commit:

```sh
git add -- .gitattributes proto crates/protocol xtask apps/mobile-web/package.json apps/mobile-web/src/protocol
git add -- tooling/check-protocol.mjs tooling/generate-protocol.mjs tooling/protocol-toolchain.test.mjs tooling/protocol-generation.test.mjs tooling/protocol-tools.json tests/fixtures/protocol
git add -- docs/components/protocol-generation.md
git add -p -- Cargo.lock package.json pnpm-lock.yaml deny.toml docs/components/README.md docs/components/protocol.md docs/components/protocol-reviews.md docs/dependency-policy.md docs/dependency-inventory.md docs/implementation-plan.md
git diff --cached --check
git diff --cached
git commit -m "feat(protocol): freeze v1 schema and add reproducible redacted wire generation"
```

Select the protocol generation/license hunks only in shared files; review
prerequisite pending commits before committing a partial lockfile. Do not use
`git add .` or add tool caches/artifacts. Keep the Windows-first ADR/checkpoint
documentation and prior Mac/network implementation in separately reviewed
commits. No push is authorized. After the required generated-source commit,
PR-004 becomes dependency-ready; NW-004 and F-005 are independently ready.
