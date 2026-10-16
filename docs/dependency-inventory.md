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

Wave 8 adds the following platform-gated packages. Every resolved package is
named here; packages grouped in one row share the stated review role. License
expressions were read from locked Cargo metadata. Each expression offers an
already allowed MIT or Apache-2.0 choice (the Objective-C bindings also offer
Zlib); final policy evaluation still requires the unexecuted cargo-deny gate.

| Packages and locked versions | Role/review disposition |
|---|---|
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

`pnpm-lock.yaml` currently contains no resolved external package snapshots. The
root manifest declares pinned frontend-development tools, but F-002 remains
blocked until a successful pnpm installation writes those exact packages and
transitives to the lockfile. They are therefore not falsely listed as reviewed
or scanned here. F-004 does not convert declarations into a resolved inventory.

## Tool execution record

- `cargo metadata --format-version 1 --locked`: passed for the graph above on
  2026-10-16 after fetching all locked platform packages from crates.io.
- Local dependency source/license review: passed for the graph above.
- `cargo deny`: not installed in the current environment; its configuration is
  committed for F-005 and must pass before release. No advisory exception is
  recorded.
- `cargo audit`: not installed; RustSec enforcement is assigned to cargo-deny.
- `node tooling/check-secrets.mjs` and its unit tests: results are recorded in
  the F-004 implementation-plan evidence after execution.

This inventory makes no claim about unperformed network advisory lookup, future
native dependencies, unresolved pnpm packages, or final release artifacts.
