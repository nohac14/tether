# PR-004 validation checkpoint — 2026-11-16

## Scope

Implemented stateless bounded client/server wire validation in `tether-protocol`.
Authenticated/current-owner facts are immutable caller-supplied context, not a
new authentication or session implementation. No listener, OS input, ownership
mutation or credential persistence is reachable from the validator.

Descriptor-derived preflight rejects ambiguous oneofs, duplicate singular fields,
unknown union fields, oversized repeated collections, invalid UTF-8, wide
scalars, non-boolean varints and unsupported enums before generated decoding.
Whole-batch conversion reuses input-core types and privately projects held state;
it checks lower negotiated budgets, capabilities, timestamps, exact sequences,
epochs, heartbeat identifiers, edit-repeat expansion and terminal ownership/
release barriers. Failures expose only closed safe categories. The `.proto`,
frozen compatibility baseline and TypeScript wire shape are unchanged.

The regression suite explicitly demonstrates that prost alone accepts a duplicate
payload that the admission guard rejects. Test-only protobuf constructors create
malformed fixtures; they are not another runtime codec/schema. Seeded fuzzing is
test-only and exercises 20,000 bounded arbitrary/mutated inputs through all four
validator entry points. No claim of exhaustive proof, coverage-guided fuzzing,
browser validation or native safety validation follows from this test.

## Actual verification

Host: Apple Silicon macOS. Rust/Cargo 1.99.0, Node 24.19.0, pnpm 11.25.0,
checksum-verified Buf 1.72.0 and cargo-deny 0.20.2.

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed through the repository gate |
| Strict workspace/all-target/all-feature Clippy | Passed after fixing test-only Copy clones and splitting generator metadata construction; no lint waiver added |
| Workspace/all-target/all-feature locked offline tests | Passed: 243 Rust tests, including 21 protocol validation and two new generator-policy tests |
| Workspace/all-target/all-feature locked offline build | Passed |
| Full worktree `cargo xtask check` | Passed: Rust checks/build/tests, Buf lint/breaking baseline, exact offline generation, TypeScript, ESLint, Prettier, Markdown, links, 38 Vitest tests, four Node scanner tests and foundation frontend builds |
| Isolated staged-source `cargo xtask check` | Passed: 147 Rust/29 Vitest/four Node tests and all aggregate gates, without older unstaged platform/network changes |
| `cargo check -p tether-protocol --all-targets --target x86_64-pc-windows-msvc --locked --offline` | Passed; compilation only, not Windows execution/linking |
| Workspace/all-feature no-dependency documentation | Passed |
| Local repository and staged-content secret scanners | Passed: 226 worktree text files, zero binary skips; 24 staged text files, zero findings and no payload output |
| Full-graph cached-advisory `cargo-deny --offline check` | Failed exit 1: existing R-019 RUSTSEC-2026-0370/0371/0372; bans/licenses/sources pass, duplicate-version warnings remain visible |
| Isolated strict all-feature/all-target Clippy, locked offline tests/build | Passed; 147 Rust tests |

The first aggregate attempt failed Clippy on a newly enlarged generator function;
the function was refactored and the complete gate rerun successfully. Initial
cargo-deny failed to lock its standard advisory cache inside the sandbox; the
approved normal rerun reached the actual advisory failure above. No TLS/DNS,
security policy, dependency pin or suppression was changed.

The isolated npm install first failed offline with `ERR_PNPM_NO_OFFLINE_TARBALL`
for the already locked typescript-eslint 8.71.1 archive. Approved standard
official-registry installation with `--frozen-lockfile --ignore-scripts` then
reused the existing store successfully; no package/lockfile pin changed.
Cargo generated the candidate's lockfile offline from the staged manifests;
its only delta is the protocol's two local dependencies. Older worktree Cargo
entries remain untouched and unstaged. The isolated tree uses a separate inert
Git fixture for the repository's file-list/attribute tests, not a substitute
index or a permission bypass for the main repository.

No Windows runtime, physical-device, live input, LAN listener, signed package,
remote CI or Windows executable validation ran. R-018 and R-023 remain open.
PR-005 owns cross-language golden interoperability; SS/PA/NW own actual session,
authentication, rate-limiting, emergency cleanup and network admission integration.
