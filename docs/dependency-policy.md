# Dependency and supply-chain policy

- **Task:** F-004
- **Status:** Active
- **Authority:** `SPEC.md` sections 20, 32, 36, and 37

Dependencies are production code. Add one only when its maintained capability
is preferable to a smaller in-repository implementation and its boundary,
features, source, license, security history, platform behavior, and transitive
cost have been reviewed.

## Source and version policy

- Rust registry dependencies must come from the official crates.io registry.
  Unknown registries and every Git dependency are denied by default. An
  exception requires an ADR, immutable revision, provenance review, and a
  documented removal/update owner.
- Wildcard requirements are denied. Direct requirements use the narrowest
  compatible stable range that permits security updates. Application builds
  and CI use the committed `Cargo.lock` with `--locked`.
- The pnpm workspace uses the exact package-manager version in `package.json`.
  Direct JavaScript development/runtime versions remain exact, and the complete
  `pnpm-lock.yaml` is preserved. Use the official npm registry, the pinned Node
  runtime, and `pnpm install --frozen-lockfile --ignore-scripts` for the current
  tool graph. No dependency lifecycle script is needed by this reviewed graph.
  A future script/build approval requires source review, not a broad bypass.
- Scoped npm overrides are explicit, exact, owner-approved security fixes:
  `markdownlint-cli>smol-toml` 1.9.0, `markdownlint-cli>js-yaml` 5.4.1, and
  `micromark-extension-math>katex` 0.18.2. Their compatibility tests belong to
  the mandatory unit gate. Reevaluate the overrides when upstream ranges catch
  up; do not expand them to unrelated consumers or silently alter direct pins.
- The 2026-11-14 scoped remediation also fixes minimatch at 11.0.1 only beneath
  `eslint`, `@eslint/config-array`, `@typescript-eslint/typescript-estree` and
  `markdownlint-cli`. Preserve all four consumer-resolution/behavior regressions
  and the bounded preprocessing regression; no global override is approved.
- Default and optional features are reviewed. Disable unused features where
  that materially reduces native code, build scripts, parsers, or attack
  surface without reimplementing maintained behavior.
- New build scripts, procedural macros, native/FFI libraries, cryptography,
  authentication libraries, parsers, network stacks, installers, and updater
  components receive explicit source review proportional to their privilege.
- Runtime operation must not fetch packages or depend on a registry, CDN, or
  update service. Dependency acquisition is a build/development activity only.

## License policy

`deny.toml` initially allows only licenses needed by the reviewed graph:
MIT, Apache-2.0, and Unicode-3.0. Workspace-private crates are excluded from
third-party Rust license evaluation. A newly encountered license is denied until its
obligations and compatibility with desktop/web distribution are reviewed and
the inventory is updated. License exceptions must name an exact crate/version
and rationale. BV-001 accepts only `matchit` =0.8.4's additional BSD-3-Clause
obligation alongside MIT: both bundled license texts were reviewed; copyright,
conditions, disclaimers, and the no-endorsement restriction must be retained.
The probe is not a shipped application dependency; any future application use
still requires its normal dependency/notice review. This exact exception does
not broaden the Rust license allowlist or suppress advisory checks.

PR-003 accepts `foldhash` =0.1.5's Zlib license only as an unmodified
development transitive of prost-build/petgraph/hashbrown. Its full source
notice was reviewed: retain that notice in source redistribution, do not
misrepresent origin, and plainly mark any altered source. The exact
cargo-deny license exception does not allow Zlib globally or suppress an
advisory. It is not part of the protocol application runtime dependency tree.
The protobuf-es browser runtime requires both Apache-2.0 and BSD-3-Clause
notices, including Google's varint notice; PK-005 must include them, rather
than treating the conjunction as a choice of license.

The npm development graph is reviewed separately in `dependency-inventory.md`;
cargo-deny does not inspect it. The current additional BSD-2-Clause,
BSD-3-Clause, ISC, Python-2.0, and BlueOak-1.0.0 packages are accepted only with
their required notices retained. `run-con` uses its MIT alternative.
The optional WASM graph's `tslib` 2.8.1 uses 0BSD; its unmodified runtime
helper is accepted under that permissive license, with attribution retained
in the inventory and any later notice bundle.
MPL-2.0 Lightning CSS and its platform bindings are accepted as unmodified,
development-only tools, not application dependencies to embed or ship. If
those tools or covered code are redistributed, PK-005 must retain notices and
satisfy the covered-source availability obligations before release. New
runtime use, changed license terms, or modifications require another review;
the Rust license allowlist is not broadened by this npm disposition.

## Advisory and update policy

- `cargo deny check advisories` is the RustSec/yank gate. Advisories are not
  ignored without a dated, scoped risk acceptance and remediation deadline.
- `pnpm audit --registry=https://registry.npmjs.org/` is the npm advisory gate,
  including development and optional packages. No severity threshold or ignore
  is used. A clean lookup is dated evidence, not a proof of vulnerability absence.
- Use the reviewed `cargo-deny` 0.20.2 tool pin for local and future CI checks.
  Install with `cargo install cargo-deny --version 0.20.2 --locked` from the
  official crates.io registry. The tool's MSRV is 1.88.0 and is compatible with
  the repository's Rust 1.99.0. Do not commit the executable or its caches.
- `cargo deny check bans licenses sources` enforces duplicate, wildcard,
  license, registry, and Git-source policy. Duplicate versions warn initially
  and require review rather than automatic rejection because platform stacks
  can legitimately constrain them.
- Internal path dependencies also specify an explicit workspace version;
  leaving the requirement implicit is a wildcard under the same bans policy.
- Dependabot checks Cargo, npm, and GitHub Actions weekly. Patch/minor updates
  are grouped; major changes remain individually reviewable. Every update must
  pass the same tests and platform gates as a manual change.
- F-005 must pin third-party CI actions to reviewed immutable commit SHAs. A
  moving tag shown in documentation is not sufficient provenance for release
  CI.
- Release artifacts eventually include the reviewed inventory/SBOM required by
  PK-005. The current inventory is development evidence, not the final SBOM.

## Lockfile changes

Every dependency change must include:

1. the manifest change and regenerated lockfile in one focused review;
2. `cargo metadata --locked` or `pnpm install --frozen-lockfile` as applicable;
3. source/license/advisory checks with no unexplained exception;
4. review of new transitive packages, build scripts, proc macros, and native code;
5. relevant formatting, lint, tests, builds, and generated-source checks; and
6. an updated `docs/dependency-inventory.md` until automated SBOM generation
   replaces the hand-reviewed bootstrap inventory.

Never hand-edit a checksum or source entry. Do not disable TLS, DNS, certificate,
checksum, signature, advisory, or lockfile verification to make acquisition
succeed.

## Secret scanning

`node tooling/check-secrets.mjs` scans tracked and non-ignored untracked text
files through a bounded, shell-free Git file list. It reports only path, line,
and rule; matched values never appear in output. Known binary assets are skipped,
NUL-containing files are treated as binary, and an unexpected text file over
1 MiB fails rather than bypassing inspection.

The local scanner catches common private keys, provider tokens, credential URLs,
and literal credential assignments. Its generic literal-assignment rule has an
intentional minimum value length of 16 characters; provider-specific rules use
their documented token shapes. It enumerates only the current tracked and
non-ignored worktree. Paths explicitly reported deleted by Git have no worktree
contents and are excluded; unexpected read failures still fail the scan. It
does not inspect commits, deleted blobs, other refs, or
Git history. It therefore complements rather than replaces the pinned,
history-aware scanner required in F-005/IT-004. CI must scan all reachable Git
history appropriate to the checkout, while retaining redacted output and a
reviewed immutable tool version. Do not add broad suppressions. A necessary test
fixture must construct a clearly fake value at runtime or use a narrowly
reviewed path/rule exception that cannot hide production credentials.

If a real secret is found, stop using it, revoke/rotate it through its owner,
remove it from all reachable history where authorized, and document only the
non-sensitive incident disposition.
