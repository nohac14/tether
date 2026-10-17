# Response to Gemini's Wave 8 review and tooling follow-up

**Review read in full:** `2026-10-16-gemini-wave-8-review.md`
**Reviewed commits:** `12ee084` and `067ea11`
**Date:** 2026-10-16

## Review disposition

- F-09 accepted: `reis` remains a spike candidate, and WAY-002/003 stay blocked
  on the actual GNOME/KDE evidence and adapter checkpoint under R-009.
- F-10 accepted: ordinary tests stay inert and native injection requires explicit
  opt-in. The review names `xtask test-manual`, but that command does not exist;
  `xtask/src/main.rs` implements `cargo xtask manual-test-guard <scope>`, which
  checks acknowledgement/target and performs no input. Future native runners
  must enforce the guard rather than assuming the command launches tests.
- F-11 accepted: WIN-001, X11-001, and WAY-001 remain blocked on native/VM
  environment evidence. ARM engineering tests cannot establish x86-64 support.

The review's PA-001 readiness note abbreviates the dependency chain to PR-002.
The authoritative plan requires PR-004, which itself requires PR-003 and
PR-002. Those gates remain intact. MAC-002 is dependency-ready after the review,
but this tooling/environment follow-up does not begin its implementation.

No code defect or approved architecture change was requested by these findings.

## Authorized dependency/tooling results

The owner authorized the exact pinned npm graph and a compatible pinned
cargo-deny build. The original npm install was attempted with Node 24.19.0 and
pnpm 11.25.0 against `https://registry.npmjs.org/`, with lifecycle scripts
disabled during initial resolution. It failed with
`ERR_PNPM_NO_MATCHING_VERSION` for `@eslint/js@10.11.0`.

Official registry metadata confirmed every other direct package pin exists.
Published `@eslint/js@10.0.1` supports `eslint ^10.0.0` and Node 24; the
existing `typescript-eslint@8.71.1` supports ESLint 10 and TypeScript 6.0.3.
The owner approved the `@eslint/js` correction to 10.0.1. Installation then
exposed three Markdown-tool advisories; the owner approved replacing only CLI2
0.23.3 with CLI 0.49.1 and scoped smol-toml 1.9.0/KaTeX 0.18.2 overrides.
The replacement exposed a fourth advisory, resolved by separately approved
`markdownlint-cli>js-yaml: 5.4.1`. No other direct pin changed.

F-002 is now complete: 224 entries with integrity hashes are resolved in the
pnpm lockfile, frozen installation passes with scripts disabled, npm audit
reports zero known advisories, and all aggregate checks pass. The original
lint rules remain unchanged. Baseline Markdown formatting was corrected,
including existing SPEC/review files, without changing product requirements
or review conclusions. Parser compatibility, source lint coverage, cache
exclusion, and scanner tracked-deletion handling have regression coverage.
See `2026-10-16-npm-tooling-findings.md` for the complete sequence and actual
results. F-005 is dependency-ready, not implemented or remotely validated.

`cargo-deny` 0.20.2 (MSRV 1.88.0, compatible with the pinned Rust 1.99.0)
was downloaded and built with standard `cargo install --version 0.20.2 --locked`
from crates.io into a temporary tool root. The first bans run found ten
versionless internal path dependencies. Each now specifies exact workspace
version `=0.1.0`; no policy exception or registry dependency change was needed.

Actual results after that correction:

- `cargo deny --locked check advisories`: passed with the official RustSec
  advisory database.
- `cargo deny --locked check bans licenses sources`: passed; the existing
  reviewed `syn` 2/3 duplicate warning remains visible under the policy.
- Formatting, strict workspace/all-target/all-feature Clippy, all 118 workspace
  tests, workspace builds, and Rust documentation passed on macOS arm64.
- Windows x86-64 and Linux x86-64 platform-crate compilation passed after the
  internal manifest changes. The local link checker passed across 59 Markdown
  files; all three secret-scanner tests passed; 133 text files had no findings;
  `git diff --check` and locked Cargo metadata passed.

The final F-002 rerun additionally passed `cargo xtask check` end to end,
strict all-target/all-feature locked Rust Clippy, all 118 Rust tests/builds,
warnings-denied Rust documentation, and cross-target backend Clippy. All 10
Vitest tests, four Node scanner tests, 60 Markdown lint/link checks, and the
136-file redacting secret scan passed. These are local automated results,
not native posting, a production frontend bundle, signed-package, browser,
or real-device validation. cargo-deny was rerun successfully after its normal
cache lock required the previously authorized narrow sandbox permission.

The source build and advisory fetch needed sandbox approval to write Cargo's
cache. Standard TLS, registry checksums, pinned tool version, and the project
lockfile policy were preserved. The tool binary/cache is uncommitted. F-005's
CI and Git-history secret scan are not complete merely because a local
cargo-deny run now passes.

## Environment follow-up

`../manual-tests/test-environment-setup.md` gives hardware-aware VM, SSH,
interactive session, Windows, and Android steps. No VM, guest account, public
listener, native injector, or physical-device test was created or run by this
follow-up. The owner confirmed only this Apple Silicon Mac with 16 GiB RAM
and approximately 60 GiB free disk. The concrete next setup is one ARM64
GNOME VM (4 GiB RAM, 2 vCPUs, dynamic 32 GiB disk), not multiple simultaneous
guests. UTM installation, ISO/storage location, and VM creation still require
the owner's preparation/authorization. Windows/Linux x86-64 and physical
Android gates need borrowed hardware or a collaborator and remain open.

The owner later reported a Windows 11 AMD PC. The live environment inventory
and setup guide now reflect that candidate native test host; Windows access,
exact build/architecture/resources, and actual tests are still pending. No
Windows or Linux VM gate is closed by hardware availability alone.

## Git handoff

The owner-created Wave 8 commits were verified as `12ee084` and `067ea11`.
The owner subsequently authorized committing. A normal scoped `git add` was
attempted and failed with `fatal: Unable to create
'/Users/jl/tether/.git/index.lock': Operation not permitted`. The sandbox still
exposes `.git` read-only. No index update, permission change, commit, or push
was performed, and no sandbox bypass was attempted. `Cargo.lock`
is unchanged; the regenerated pnpm lockfile and manifests contain only the
approved dependency changes. The scanner regression test creates/stages only
a disposable temporary fixture repository, never this real index or history.
Review `git diff` first, then use these task-focused commands manually from the
repository root (no cache, secret, build output, or machine-specific key is staged):

```sh
git diff --check
git add -- crates/config/Cargo.toml crates/input-core/Cargo.toml crates/input-macos/Cargo.toml crates/input-wayland/Cargo.toml crates/input-windows/Cargo.toml crates/input-x11/Cargo.toml
git commit -m "build: version internal path dependencies for cargo-deny"
git add -- .markdownlint-cli2.jsonc .markdownlint.json eslint.config.mjs package.json pnpm-lock.yaml pnpm-workspace.yaml vitest.config.mjs tooling/check-links.mjs tooling/check-links.test.mjs tooling/check-secrets.mjs tooling/check-secrets.test.mjs tooling/markdown-toolchain.test.mjs apps/desktop/src/main.ts apps/mobile-web/index.html apps/mobile-web/src/main.ts docs/dependency-policy.md docs/dependency-inventory.md docs/reviews/2026-10-16-npm-tooling-findings.md
git add -- SPEC.md apps/desktop/tests/README.md apps/mobile-web/tests/README.md docs/adr/README.md docs/architecture-decisions.md docs/architecture-review.md docs/components/backend-conformance.md docs/components/desktop-shell.md docs/components/native-backend-spikes.md docs/components/protocol.md docs/components/storage.md docs/documentation-plan.md docs/gemini-independent-review.md docs/manual-tests/2026-10-16-wave8-spike.md docs/reviews/2026-10-09-gemini-baseline-audit.md docs/reviews/2026-10-10-codex-milestone-report.md docs/reviews/2026-10-10-gemini-ic003-review-response.md docs/reviews/2026-10-10-gemini-ic003-review.md docs/reviews/2026-10-12-codex-foundation-audit-response.md docs/reviews/2026-10-12-gemini-foundation-audit.md docs/reviews/2026-10-12-gemini-identity-conformance-audit.md tests/contract/README.md tests/e2e/README.md tests/fixtures/README.md tests/integration/README.md tests/native/README.md
git commit -m "build: complete F-002 with audited npm toolchain"
git add -- docs/implementation-plan.md docs/open-risks.md docs/manual-tests/environment-inventory.md docs/manual-tests/test-environment-setup.md docs/reviews/2026-10-16-codex-wave8-response.md docs/reviews/2026-10-16-gemini-wave-8-review.md
git commit -m "docs: reconcile Wave 8 review and environment setup"
```
