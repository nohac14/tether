# Initial dependency and license inventory

- **Task:** F-004
- **Inventory date:** 2026-10-16
- **Source of truth:** committed `Cargo.lock` and `pnpm-lock.yaml`
- **Scope:** bootstrap development inventory; PK-005 owns the release SBOM

## Rust registry graph

All resolved external Rust packages come from
`registry+https://github.com/rust-lang/crates.io-index` and have Cargo checksums.
The graph was enumerated with `cargo metadata --format-version 1 --locked`.

| Package | Version | License expression | Role/review disposition |
| --- | ---: | --- | --- |
| `getrandom` | 0.4.3 | MIT OR Apache-2.0 | Direct OS-backed cryptographic entropy for installation identity generation; approved. No custom backend is enabled. |
| `cfg-if` | 1.0.5 | MIT OR Apache-2.0 | Target-selection helper used by `getrandom`; approved. |
| `libc` | 0.2.190 | MIT OR Apache-2.0 | Target-gated system API bindings used by `getrandom`; approved. |
| `r-efi` | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later | Target-gated UEFI entropy support; accepted under the MIT/Apache alternatives and not built for current desktop targets. |
| `serde` | 1.0.229 | MIT OR Apache-2.0 | Direct typed serialization API for versioned local settings; approved. |
| `serde_json` | 1.0.151 | MIT OR Apache-2.0 | Direct maintained strict JSON parser/serializer; approved with exact typed envelopes and byte ceilings. |
| `serde_core` | 1.0.229 | MIT OR Apache-2.0 | Serde traits transitive; approved. |
| `serde_derive` | 1.0.229 | MIT OR Apache-2.0 | Serde procedural macros; build-time code, approved for closed storage types. |
| `proc-macro2` | 1.0.107 | MIT OR Apache-2.0 | Procedural-macro token support; transitive, approved. |
| `quote` | 1.0.47 | MIT OR Apache-2.0 | Procedural-macro code generation; transitive, approved. |
| `syn` | 3.0.7 | MIT OR Apache-2.0 | Rust syntax parser used by derive macros at build time; transitive, approved. |
| `unicode-ident` | 1.0.26 | (MIT OR Apache-2.0) AND Unicode-3.0 | Unicode identifier tables for proc macros; approved, including Unicode-3.0 notice obligation. |
| `itoa` | 1.0.18 | MIT OR Apache-2.0 | JSON integer formatting; transitive, approved. |
| `memchr` | 2.8.3 | Unlicense OR MIT | JSON byte search; accepted under the MIT option. |
| `zmij` | 1.0.23 | MIT | JSON floating-point formatting transitive to `serde_json`; settings reject unexpected numeric shapes through typed decoding. Approved. |

Wave 8 adds the following platform-gated packages. Every resolved package is
named here; packages grouped in one row share the stated review role. License
expressions were read from locked Cargo metadata. Each expression offers an
already allowed MIT or Apache-2.0 choice (the Objective-C bindings also offer
Zlib); cargo-deny 0.20.2 subsequently passed the committed license policy on
2026-10-16.

| Packages and locked versions | Role/review disposition |
| --- | --- |
| `windows` 0.62.2 | Direct generated Windows API bindings for the `SendInput` compile contract; approved with a narrow KeyboardAndMouse feature. |
| `windows-core` 0.62.2; `windows-collections` 0.3.2; `windows-future` 0.3.2; `windows-implement` 0.60.2; `windows-interface` 0.59.3; `windows-link` 0.2.1; `windows-numerics` 0.3.1; `windows-result` 0.4.1; `windows-strings` 0.5.1; `windows-threading` 0.2.1; `syn` 2.0.119 | Generated Windows binding/runtime and macro transitives; target-gated, no Tether FFI or installation script. |
| `objc2-core-graphics` 0.3.2; `objc2-core-foundation` 0.3.2; `bitflags` 2.13.2 | Direct generated Core Graphics binding plus Core Foundation/flags support. Selected to avoid local FFI and compiled on macOS arm64 with only reviewed CG features. |
| `x11rb` 0.14.0; `x11rb-protocol` 0.14.0 | Direct pure-Rust X11 client and generated XKB/XTEST protocol support; default/native unsafe features remain disabled. |
| `gethostname` 1.1.0; `errno` 0.3.14; `rustix` 1.1.5; `linux-raw-sys` 0.12.1; `windows-sys` 0.61.2 | Platform abstraction transitives used by the X11/portal graph; reviewed as target-gated OS bindings. |
| `ashpd` 0.13.13; `reis` 0.7.1 | Direct XDG RemoteDesktop portal and pure-Rust EI protocol candidates. `ashpd` uses remote-desktop/screencast/Tokio only; `reis` uses Tokio only. Both remain prototypes pending R-009 target validation. |
| `zbus` 5.19.0; `zbus_macros` 5.19.0; `zbus_names` 4.3.5; `zvariant` 5.15.1; `zvariant_derive` 5.15.1; `zvariant_utils` 4.2.0; `zcheapstr` 1.1.0; `endi` 1.1.1; `enumflags2` 0.7.12; `enumflags2_derive` 0.7.12; `serde_repr` 0.1.21; `ordered-stream` 0.2.0; `hex` 0.4.3 | D-Bus, portal wire types, flags, derives, and bounded encoding transitives of `ashpd`; no runtime network dependency. |
| `async-broadcast` 0.7.2; `event-listener` 5.4.2; `event-listener-strategy` 0.5.4; `futures-core` 0.3.34; `futures-io` 0.3.34; `futures-lite` 2.6.1; `futures-macro` 0.3.34; `futures-task` 0.3.34; `futures-util` 0.3.34; `pin-project-lite` 0.2.17; `parking` 2.2.1; `slab` 0.4.12 | Bounded asynchronous/event-stream support transitively required by the portal/EI stacks. |
| `tokio` 1.53.3; `mio` 1.2.4; `socket2` 0.6.5; `signal-hook-registry` 1.4.8; `bytes` 1.12.1 | Target-gated async Unix socket/runtime support for the `ashpd` and `reis` compatibility spike; not a LAN server selection. |
| `tracing` 0.1.44; `tracing-attributes` 0.1.31; `tracing-core` 0.1.36; `log` 0.4.34 | Library diagnostics facade transitives. Tether's no-input-payload logging policy still applies at adapters. |
| `async-recursion` 1.2.0; `async-trait` 0.1.92; `proc-macro-crate` 3.5.0; `toml_datetime` 1.1.2+spec-1.1.0; `toml_edit` 0.25.17+spec-1.1.0; `toml_parser` 1.1.5+spec-1.1.0; `winnow` 1.0.4; `indexmap` 2.14.2; `hashbrown` 0.17.1; `equivalent` 1.0.3 | Procedural macro/configuration/parser transitives used to generate the reviewed D-Bus/EI APIs. They do not parse Tether LAN input. |
| `autocfg` 1.5.1; `memoffset` 0.9.1; `once_cell` 1.21.4; `fastrand` 2.5.0; `tempfile` 3.27.0; `uuid` 1.28.0 | Build/configuration, temporary-resource, and portal identifier transitives. They are not used for Tether credentials or production installation identity. |
| `bumpalo` 3.20.3; `js-sys` 0.3.106; `rustversion` 1.0.23; `uds_windows` 1.2.1; `wasi` 0.11.1+wasi-snapshot-preview1; `wasm-bindgen` 0.2.129; `wasm-bindgen-macro` 0.2.129; `wasm-bindgen-macro-support` 0.2.129; `wasm-bindgen-shared` 0.2.129 | Target-specific portability transitives resolved by the full portal/runtime graph; fetched and inventoried but not built for the current macOS/Linux desktop paths unless selected by Cargo cfg. |

No resolved Rust package uses a Git source, unknown registry, obligatorily
copyleft license, or unchecksummed registry entry. The only duplicate package
name is `syn` at 2.0.119 and 3.0.7, constrained respectively by the Windows and
existing Serde macro stacks; it requires the documented duplicate warning, not
suppression.

The newly introduced `getrandom` build script only inspects Cargo's sanitizer
configuration and emits a compile-time cfg. Its default/custom entropy backend
features are not enabled. The existing `libc` transitive includes its reviewed
target-configuration/rustc-introspection build script; `cfg-if` and target-gated
`r-efi` add no build script to this graph. No dependency build step downloads or
runs an external installation script.

## JavaScript graph

F-002 resolved 224 package entries/snapshots through the official npm registry
using Node 24.19.0 and pnpm 11.25.0. All 224 entries have integrity hashes.
The macOS arm64 installation's license report enumerates 194 package/version
instances. Thirty platform/WASM entries are resolved but not installed on this
host; their execution is not validated here. The lockfile, not a machine-local
cache/path report, is the complete package/version graph.

For the 30 uninstalled entries, official npm version metadata was queried
without executing code; package identity and integrity matched the lockfile.
That completes license/lifecycle metadata coverage for all 224 entries, not
native runtime or binary-source validation. None declares an install,
preinstall, or postinstall hook. Optional metadata identifies 19 MIT, ten
MPL-2.0 Lightning CSS platform bindings, and one 0BSD package.

| Direct tool | Exact pin | Role |
| --- | --- | --- |
| `@eslint/js` | 10.0.1 | Official ESLint rules; owner-approved correction of nonexistent 10.11.0; peer range supports ESLint 10. |
| `eslint` | 10.11.0 | JavaScript and TypeScript static analysis. |
| `markdownlint-cli` | 0.49.1 | Owner-approved replacement of CLI2 0.23.3 to remove the unpatched `braces` graph; unchanged rule configuration and source coverage. |
| `prettier` | 3.9.7 | Mechanical source/config formatting. |
| `typescript` | 6.0.3 | Frontend type checks. |
| `typescript-eslint` | 8.71.1 | Strict typed ESLint rules; declared ESLint/TypeScript peer ranges checked. |
| `vite` | 8.1.0 | Frontend build/test tooling; no production web bundle exists yet. |
| `vitest` | 5.0.3 | Tooling unit tests; the independent Node test suite is also mandatory. |

The three exact subtree overrides are `markdownlint-cli>smol-toml` 1.9.0,
`markdownlint-cli>js-yaml` 5.4.1, and
`micromark-extension-math>katex` 0.18.2. They fix the reviewed parser advisories;
no other direct pin changed. Five regression tests verify unchanged rule
configuration, TOML loading/UTF-8/malformed parsing, YAML loading/rejection,
ignored-artifact versus source-document coverage, and inline/block math
rendering with untrusted links rejected. These are compatibility checks, not a
claim to prove parser security exhaustively. The CLI probes multiple formats;
an invalid TOML string that is valid YAML is not necessarily rejected by the
CLI, so strict malformed TOML coverage tests the actual overridden parser.
The original CLI2 0.23.3 pins markdownlint 0.41.1, and the replacement resolves
the same engine; the unit gate asserts that version as well as the unchanged
configuration. No rule-set downgrade is hidden by the CLI change.

### npm license disposition

Installed metadata counts are 157 MIT, 15 Apache-2.0, 7 BSD-2-Clause,
3 BSD-3-Clause, 7 ISC, 1 Python-2.0, 1 BlueOak-1.0.0, 2 MPL-2.0, and
1 multi-license package accepted under MIT. Review used the installed package
metadata/license texts; no license text was edited or stripped.

| Additional license / packages | Development and distribution disposition |
| --- | --- |
| BSD-2-Clause: `entities` 4.5.0, `eslint-scope` 9.1.2, `espree` 11.2.0, `esrecurse` 4.3.0, `estraverse` 5.3.0, `esutils` 2.0.3, `uri-js` 4.4.1 | Accept with copyright, conditions, and disclaimer retained in source and binary redistribution materials. |
| BSD-3-Clause: `esquery` 1.7.0, `smol-toml` 1.9.0, `source-map-js` 1.2.2 | Accept with the above notice duties and no contributor-name endorsement. |
| ISC: `flatted` 3.4.4, `glob-parent` 6.0.2, `ini` 7.0.0, `isexe` 2.0.0, `picocolors` 1.1.1, `semver` 7.8.5, `which` 2.0.2 | Accept with copyright and permission notice preserved. |
| Python-2.0: `argparse` 2.0.1 | Accept as an unmodified development tool transitive; retain its bundled Python/PSF and historical notices. A redistributed modified derivative must include the required change summary. |
| BlueOak-1.0.0: `minimatch` 10.2.6 | Accept with the license text or permitted license link accompanying redistribution. |
| `run-con` 1.3.3: BSD-2-Clause OR MIT OR Apache-2.0 | Select MIT; retain its notice. |
| 0BSD: optional `tslib` 2.8.1 | Accept the unmodified WASM-runtime helper under its permissive zero-clause BSD terms; preserve its source attribution in the inventory/notice process. Not installed or executed here. |
| MPL-2.0: `lightningcss` 1.33.0 and `lightningcss-darwin-arm64` 1.33.0 | Accept only as unmodified development tooling. Do not embed or ship this native compiler in the Tether application. Redistributing covered tools/code requires MPL notices and covered-source availability under sections 3.1–3.4; PK-005 must verify actual artifacts. |

The uninstalled optional graph comprises `@emnapi/core`, `@emnapi/runtime`, and
`@emnapi/wasi-threads`; `@napi-rs/wasm-runtime`; `@tybys/wasm-util`; `tslib`;
14 non-host `@rolldown/binding-*` packages; and ten non-host `lightningcss-*`
packages. Exact names/versions are in the lockfile. The 0BSD license text was
checked against [Microsoft's matching tslib tag](https://github.com/microsoft/tslib/blob/v2.8.1/LICENSE.txt).
The MPL obligations refer to covered compiler code, not a claim that compiler
output automatically inherits MPL; see the [upstream MPL FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/).

Vite/Vitest load Rolldown's packaged native binding; Lightning CSS also carries
platform-specific optional native bindings. No installed package declares an
install/preinstall/postinstall lifecycle hook. Installation explicitly used
`--ignore-scripts`, without executing an external installer. The Darwin-arm64
Rolldown binding was exercised by Vitest. Other native bindings and the WASM
fallback graph were not executed. They remain locked provenance, not platform
support evidence. Build tools are not LAN runtime dependencies. The final
application dependency/asset notice audit and SBOM remain PK-005 requirements.

The npm audit on 2026-10-16 passed with zero known advisories, including optional
and development packages. This is a dated registry/database result, not a
guarantee of vulnerability absence. The previous findings and owner-approved
remediation are preserved in `reviews/2026-10-16-npm-tooling-findings.md`.

## Tool execution record

- `cargo metadata --format-version 1 --locked`: passed for the graph above on
  2026-10-16 after fetching all locked platform packages from crates.io.
- Local dependency source/license review: completed to the stated Rust and npm
  development scope; optional native execution and release artifact review are
  explicitly not covered.
- `cargo-deny` 0.20.2: built from official crates.io with standard Cargo and its
  published lockfile on 2026-10-16. Locked advisories, bans, licenses, and sources
  checks passed after ten internal path dependencies gained explicit `=0.1.0`
  versions. The reviewed `syn` 2/3 duplicate remains a visible policy warning;
  no exception or advisory suppression was added. The temporary tool binary
  and RustSec cache are not repository artifacts. F-005 still owns CI execution.
- `cargo audit`: not installed; RustSec enforcement is assigned to cargo-deny.
- `pnpm install --frozen-lockfile --ignore-scripts` and `pnpm audit`: passed
  for the final reviewed 224-entry graph on 2026-10-16.
- `cargo xtask check`: passed, including all 118 Rust tests, 10 Vitest tests,
  four Node scanner tests, frontend types/lint/format, all 60 Markdown files,
  and existing workspace/skeleton builds. No browser/production package ran.
- `node tooling/check-secrets.mjs` and its unit tests: results are recorded in
  the F-004 implementation-plan evidence after execution.

This inventory makes no claim about future dependencies, unexecuted platform
packages, CI execution, or final release artifacts. A successful local
advisory lookup reflects the database at execution time and must run again in
CI and before release.
