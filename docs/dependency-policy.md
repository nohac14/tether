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
  `pnpm-lock.yaml` is committed. F-002 remains incomplete until its declared
  tool dependencies are actually resolved into that lockfile.
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
third-party license evaluation. A newly encountered license is denied until its
obligations and compatibility with desktop/web distribution are reviewed and
the inventory is updated. License exceptions must name an exact crate/version
and rationale; there are currently none.

## Advisory and update policy

- `cargo deny check advisories` is the RustSec/yank gate. Advisories are not
  ignored without a dated, scoped risk acceptance and remediation deadline.
- `cargo deny check bans licenses sources` enforces duplicate, wildcard,
  license, registry, and Git-source policy. Duplicate versions warn initially
  and require review rather than automatic rejection because platform stacks
  can legitimately constrain them.
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
non-ignored worktree and does not inspect commits, deleted blobs, other refs, or
Git history. It therefore complements rather than replaces the pinned,
history-aware scanner required in F-005/IT-004. CI must scan all reachable Git
history appropriate to the checkout, while retaining redacted output and a
reviewed immutable tool version. Do not add broad suppressions. A necessary test
fixture must construct a clearly fake value at runtime or use a narrowly
reviewed path/rule exception that cannot hide production credentials.

If a real secret is found, stop using it, revoke/rotate it through its owner,
remove it from all reachable history where authorized, and document only the
non-sensitive incident disposition.
