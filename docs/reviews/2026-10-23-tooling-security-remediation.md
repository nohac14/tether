# Scoped tooling security remediation — 2026-10-23

## Authority and scope

The owner explicitly approved Vite 8.1.0 to 8.1.6, markdown-it 14.3.2 to
14.3.3 within its existing dependency range, required transitives, and restoring
Node 24.19.0 from the official distribution. All other direct pins, existing
security overrides, lint rules, and safety gates remain unchanged. No Wayland
migration, advisory ignore, native input execution, LAN listener, or architecture
change is included.

## Changes and provenance

- `package.json`: only the exact Vite pin changes to 8.1.6.
- `pnpm-lock.yaml`: only Vite and markdown-it change package versions/checksums;
  the Vitest/mocker peer snapshot references follow Vite 8.1.6. There are still
  224 entries. A structural comparison against `HEAD` verified every other
  direct pin and package integrity hash is unchanged. No override was added.
- `tooling/markdown-toolchain.test.mjs`: assert the patched parser and exercise
  multiline references, bounded malformed input, and HTML escaping, retaining
  every existing rule/configuration/parser compatibility test.
- `tooling/vite-toolchain.test.mjs`: assert Vite's patch and build a small
  isolated ES-module fixture without loading project configuration/env files,
  binding a listener, or writing build output into the repository.
- Inventory, risk, plan, and checkpoint documentation distinguish the original
  failed refresh from this subsequent scoped remediation.

Acquisition used standard pnpm 11.25.0 against `https://registry.npmjs.org/`:

```sh
pnpm update vite@8.1.6 markdown-it --depth Infinity --ignore-scripts --registry=https://registry.npmjs.org/
pnpm install --frozen-lockfile --ignore-scripts --registry=https://registry.npmjs.org/
```

The indirect refresh remains within markdownlint-cli's existing markdown-it
range. No arbitrary installation script or dependency lifecycle hook executed.
Official registry metadata matches the two resolved integrity hashes. Both
updated packages retain MIT licensing and no install/preinstall/postinstall
hooks. Vite's complete bundled license file is unchanged from 8.1.0; the
inventory retains those bundled notices rather than relying on the package's
MIT label alone. The installed license report still covers 194 instances with
unchanged license counts. Thirty non-host optional entries remain locked but
not executed.

Official advisory evidence includes the maintainer's
[markdown-it reference-parser advisory](https://github.com/markdown-it/markdown-it/security/advisories/GHSA-gfr2-rv44-r23c)
and [Vite's bundled editor advisory](https://github.com/vitejs/launch-editor/security/advisories/GHSA-587h-7wmj-h5pm).
Installed source inspection confirms direct-source reference scanning with a
999-code-point label limit and bundled launch-editor 2.14.2 argument/path
hardening. Compatibility tests do not claim to prove exploit resistance or
replace upstream security testing; no editor was launched to test an exploit.

## Pinned runtime

The bundled runtime had drifted to Node 24.22.1. The official
[Node 24.19.0 Darwin-arm64 archive](https://nodejs.org/download/release/v24.19.0/)
was downloaded with normal HTTPS verification and matched the official
[SHA-256 manifest](https://nodejs.org/download/release/v24.19.0/SHASUMS256.txt)
before extraction:

```text
3f1cf157479c1480352083105e13faf9d008ede98e7e157746b6df940d197b94
```

An isolated temporary launcher invokes the existing pinned pnpm 11.25.0 with
that exact Node executable. Both launcher and Node directory lead `PATH` so
recursive child commands use the same runtime. Actual process versions were
checked, not inferred from `package.json`. No bundled application runtime,
global installation, shell profile, or repository machine-specific path was
modified. This is a local verification tool directory, not a persistent
installation guarantee; future runs must verify the exact versions again.
Archives, caches, raw path-bearing license reports, and binaries stay outside
Git. No TLS/DNS/checksum/advisory check was disabled.

## Actual verification

Host: macOS 15.7.5 (24G624), arm64; Rust/Cargo 1.99.0,
Node 24.19.0, pnpm 11.25.0.

| Gate | Actual result |
| --- | --- |
| Frozen pnpm install, scripts disabled | Passed; 224-entry lockfile passes pnpm supply-chain policies |
| Full npm audit, including development/optional graph | Passed; zero findings in every severity class |
| Manifest/lockfile scope, source/license/lifecycle review | Passed to the scope described above |
| Tooling tests | Passed: 13 Vitest tests and four Node scanner tests |
| `cargo fmt --all --check` | Passed |
| Strict workspace/all-target/all-feature locked offline Clippy | Passed |
| Workspace/all-target/all-feature locked offline tests | Passed: 138 tests, none failed or ignored |
| Workspace/all-target/all-feature locked offline build | Passed |
| Warnings-denied workspace Rust docs | Passed |
| Pinned cargo-deny 0.20.2 bans/licenses/sources | Passed; reviewed `syn` 2/3 duplicate warning remains visible |
| Pinned cargo-deny 0.20.2 advisories | **Failed**, unchanged three R-019 unmaintained Wayland transitives |
| Aggregate repository check on pinned Node | Passed: Rust format/strict Clippy/tests/build; frontend types/ESLint/Prettier, Markdown, links, unit tests, and skeleton builds |
| Markdown and local links | Passed across 64 Markdown files |
| Local secret scan | Passed: 146 text files, zero binary files skipped; local scanner limitations remain documented |
| `git diff --check` | Passed |
| Cross-platform runtime, live native input, phone/browser, signed package, remote CI | Not run |

The Rust test run used approved WindowServer access only for native event
construction/inspection. No event was posted and no Accessibility prompt was
requested. The new Vite fixture exercises the installed Darwin-arm64 build
binding; it is not a mobile application build or a physical browser test.
Application frontend build targets are still skeleton checks.

## Remaining gates and Git boundary

R-020 is resolved for this dated verification. R-019 remains a whole-workspace
advisory blocker: `ashpd` 0.13.13 / zbus 5.19.0 brings `zbus_names` 4.3.5,
`zvariant` 5.15.1, and `zvariant_derive` 5.15.1, now reported unmaintained.
Their migration into zbus 6 needs a separate compatibility/source/license
review; nothing is suppressed or changed here. This report does not declare
the complete project security gate green. MAC-002 remains in progress;
Accessibility consent, real input/lifecycle/application validation, remaining
media support, and complete desktop/mobile integration are not done. R-018 and
BV-001 remain unresolved. F-005 CI has not been implemented or executed.

The repository's `.git` remains read-only. No permission bypass, commit, or
push was attempted for this remediation. Preserve the pre-existing macOS
checkpoint and personal-deployment documentation changes. Focused manual
commit commands are in the updated macOS checkpoint record; review the whole
diff before executing them. No credentials, native binaries, generated build
artifacts, caches, or machine-local runtime paths belong in those commits.
