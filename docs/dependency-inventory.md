# Initial dependency and license inventory

- **Task:** F-004
- **Inventory date:** 2026-10-12
- **Source of truth:** committed `Cargo.lock` and `pnpm-lock.yaml`
- **Scope:** bootstrap development inventory; PK-005 owns the release SBOM

## Rust registry graph

All resolved external Rust packages come from
`registry+https://github.com/rust-lang/crates.io-index` and have Cargo checksums.
The graph was enumerated with `cargo metadata --format-version 1 --locked`.

| Package | Version | License expression | Role/review disposition |
|---|---:|---|---|
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

No resolved Rust package uses a Git source, unknown registry, obligatorily
copyleft license, or unchecksummed registry entry. The lockfile currently
contains one version of each of the 15 external crates.

The newly introduced `getrandom` build script only inspects Cargo's sanitizer
configuration and emits a compile-time cfg. Its default/custom entropy backend
features are not enabled. The existing `libc` transitive includes its reviewed
target-configuration/rustc-introspection build script; `cfg-if` and target-gated
`r-efi` add no build script to this graph. No dependency build step downloads or
runs an external installation script.

## JavaScript graph

`pnpm-lock.yaml` currently contains no resolved external package snapshots. The
root manifest declares pinned frontend-development tools, but F-002 remains
blocked until a successful pnpm installation writes those exact packages and
transitives to the lockfile. They are therefore not falsely listed as reviewed
or scanned here. F-004 does not convert declarations into a resolved inventory.

## Tool execution record

- `cargo metadata --format-version 1 --locked`: passed for the graph above.
- Local dependency source/license review: passed for the graph above.
- `cargo deny`: not installed in the current environment; its configuration is
  committed for F-005 and must pass before release. No advisory exception is
  recorded.
- `cargo audit`: not installed; RustSec enforcement is assigned to cargo-deny.
- `node tooling/check-secrets.mjs` and its unit tests: results are recorded in
  the F-004 implementation-plan evidence after execution.

This inventory makes no claim about unperformed network advisory lookup, future
native dependencies, unresolved pnpm packages, or final release artifacts.
