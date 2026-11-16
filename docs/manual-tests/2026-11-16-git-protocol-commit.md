# Local Git and protocol commit checkpoint — 2026-11-16

## Outcome

The owner selected manual approvals and authorized local staging/commits.
The normal escalation/approval flow successfully ran `git add`, narrowly scoped
`git apply --cached`, and `git commit`. No `chmod`, ownership change, temporary
index redirection, repository reset, history rewrite or push was used.

Existing owner commit `0a2b2f1` was preserved. New commits:

- `8ff0bf5` — existing tooling/network evidence and contracts referenced by that
  commit; documentation only. The isolated link checker initially found their
  target files absent from Git. Including the existing records, rather than
  suppressing the checker, repairs those links without committing native code.
- `a5a8da0` — PR-002 refinement and PR-003 reproducible redacted wire generation.
  Generated Rust/TypeScript sources, baseline, generator/tools/tests, manifests,
  exact development license disposition and protocol-only Cargo lock additions
  are committed. PR-003's previously missing commit criterion is now satisfied.

PR-004 is dependency-ready, not implemented. No authentication, session admission,
LAN input or Windows runtime was enabled by these commits.

## Isolation and preservation

The owner had already committed the full npm lockfile, including protocol and
earlier Vite security entries. The protocol commit aligns the corresponding
manifests, retaining the approved Vite 8.1.6 pin; it does not introduce another
dependency upgrade.

The worktree Cargo lockfile also contained older Mac/network/probe additions.
Those were not swept into the protocol commit. An owned temporary staged-files
checkout used normal offline Cargo resolution to prune unrelated entries and
retain exactly the 17 reviewed protocol additions. Every selected registry
version/checksum was compared to the reviewed worktree lockfile and matched.
Only that generated lockfile delta was staged. The main worktree file and its
older dependencies were left intact. No checksum was hand-edited.

The temporary verification checkout had its own inert Git test fixture so
Git-attribute and file-list tests could execute normally. It was not an alternate
index for the main repository and was never used to evade a denied write.
Build output, downloaded packages and this test metadata are outside committed
source. Older Mac/network/config code and other unrelated work remain unstaged.

The first staged whitespace check exposed two previously untracked issues:
Markdown metadata hard breaks and the TypeScript plugin's extra EOF blank line.
The Markdown uses ordinary paragraph separation; the generator now canonicalizes
EOF and tests exactly one final newline. No generated source was hand-edited and
no whitespace, lint, safety or secret rule was weakened.

## Actual verification

Host/toolchain: existing Apple Silicon Mac, Rust/Cargo 1.99.0, Node 24.19.0,
pnpm 11.25.0 and checksum-verified Buf 1.72.0. All commands ran on this Mac.

| Check | Result |
| --- | --- |
| Full working-tree `cargo xtask check` | Passed: 220 Rust tests, 38 Vitest tests, four Node scanner tests, format/strict Clippy/types/ESLint/Prettier/Markdown/links, Buf and offline generation checks, current Rust/frontend foundation builds |
| Isolated commit candidate `cargo xtask check` | Passed: 124 Rust tests, 29 Vitest tests, four Node scanner tests and the same applicable aggregate gates; 78 Markdown files |
| Isolated strict all-feature/all-target Rust Clippy, tests and builds with `--locked --offline` | Passed; 124 Rust tests |
| Isolated frozen scripts-disabled npm installation | Passed after official-registry metadata acquisition; 230 locked entries, no pin/lockfile changes or lifecycle scripts |
| Initial offline npm installation | Failed with `ERR_PNPM_NO_OFFLINE_META` for protobuf supply-chain metadata. Not counted as passing; approved standard official-registry installation resolved the missing metadata without disabling policy checks |
| Full worktree secret scanner | Passed: 210 text files, zero binary skips |
| Staged-content secret scan before final checkpoint staging | Passed: 28 changed text files, zero findings; values never printed |
| Isolated worktree secret scanner | Passed: 167 text files, zero binary skips |
| Staged whitespace / full worktree whitespace | Passed after generator/documentation fixes |
| Isolated full-graph cargo-deny 0.20.2 | Failed, exit 1: the same RUSTSEC-2026-0370/0371/0372 Wayland maintenance advisories. Bans/licenses/sources pass; duplicate syn/hashbrown warnings remain visible |

Different test counts are intentional evidence scopes, not skipped tests: the
larger worktree includes older uncommitted Mac/network components and tests.
The isolated source snapshot proves the new protocol commit does not require
those files to compile or pass its aggregate. Frontend builds remain foundation
skeleton commands, not a finished React/Tauri application.

The original November 14 hardware/tooling evidence remains historical. No fresh
Windows compilation/runtime, signed package, live native input, physical-device,
LAN discovery or installer validation is claimed here. R-018 and R-023 remain
open; Microsoft SDK provenance was not bypassed or retested in this Git batch.
R-019's failing full-graph result remains a verification/release blocker.

## Next tasks

PR-004 wire-to-domain validation is next. NW-004 embedded assets and F-005 CI are
independently ready in the plan. Git writes still require their normal approval
flow; this is not unrestricted machine access or permission to push to GitHub.
