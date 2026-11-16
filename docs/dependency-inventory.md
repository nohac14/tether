# Initial dependency and license inventory

- **Task:** F-004
- **Inventory date:** 2026-10-25 (test-only BV-001 graph; earlier checkpoints retained)
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
copyleft license, or unchecksummed registry entry. The initial Wave 8 graph's
only duplicate was `syn` at 2.0.119 and 3.0.7 across the native-binding/portal and
Serde macro stacks. BV-001 also requires `getrandom` 0.3.4/0.4.3 for the
WebSocket and identity graphs respectively. Both duplicates remain documented,
visible warnings, not suppressions.

The identity dependency `getrandom` 0.4.3 build script only inspects Cargo's sanitizer
configuration and emits a compile-time cfg. Its default/custom entropy backend
features are not enabled. The existing `libc` transitive includes its reviewed
target-configuration/rustc-introspection build script; `cfg-if` and target-gated
`r-efi` add no build script to this graph. No dependency build step downloads or
runs an external installation script.

## macOS checkpoint dependency note

MAC-002 promotes the already locked `objc2-core-foundation` 0.3.2 transitive
to a direct, macOS-only dependency with `std`/`CFCGTypes` for owned Quartz
objects and points. No new registry package/version was added; the lockfile
adds only that edge. MIT/Apache-2.0/Zlib licensing and source are unchanged.
Production code has one narrowly documented unsafe slice-to-pointer Unicode
setter; tests have an additional bounded output-buffer getter. No local extern
declaration, unsafe Send implementation, or crate-wide lint waiver was added.

On 2026-10-23 pinned cargo-deny was rebuilt from the official cached package
with its published lockfile. Bans/licenses/sources pass; advisories **fail** on
the three newly unmaintained Wayland D-Bus transitives in R-019. The older
2026-10-16 passing evidence below is historical, not today's gate status.
No advisory was ignored and no unrelated Linux dependency pin changed.

The initial npm refresh failed on seven Vite/markdown-it findings, and its
aggregate used drifted bundled Node 24.22.1. Subsequent owner-approved R-020
remediation changes only Vite 8.1.0 to 8.1.6 and markdown-it 14.3.2 to 14.3.3;
all other direct pins, overrides, and package integrity hashes are unchanged.
The graph remains 224 entries. Frozen installation and audit now pass with
zero findings on checksum-verified official Node 24.19.0/pnpm 11.25.0. See
[the scoped verification record](reviews/2026-10-23-tooling-security-remediation.md).
This does not resolve R-019 or claim native macOS input validation.

## BV-001 test-only HTTP/WebSocket dependencies — 2026-10-25

The isolated probe is a private executable under `tests/e2e`, not a shipped
application dependency or substitute production server. Standard Cargo fetched
44 additional official registry packages and retained every previously locked
package/version/checksum. Its graph reuses Tokio 1.53.3 and the existing
futures/Serde/OS bindings; the probe adds only the required runtime/IO/time/signal
features and development socket-test support. All new packages are listed here.
This is a compatibility/security-boundary review, not a claim of exhaustive
third-party source verification.

| New packages and resolved versions | Review and license disposition |
| --- | --- |
| `axum` 0.8.9; `axum-core` 0.5.6; `matchit` 0.8.4 | Maintained intended HTTP stack with default form/JSON/query/tracing features disabled; route only fixed assets/test WS. Axum pins matchit exactly. Both MIT and BSD-3-Clause notices are required for matchit, not alternatives; the exact =0.8.4 exception retains copyright, conditions, disclaimer, and no endorsement. Other packages MIT. |
| `hyper` 1.12.0; `hyper-util` 0.1.21; `http` 1.5.0; `http-body` 1.1.0; `http-body-util` 0.1.5; `httparse` 1.10.1; `httpdate` 1.0.3; `mime` 0.3.17; `percent-encoding` 2.3.2 | HTTP/1 parsing, bounded server configuration, body/response types, and header utilities. MIT or an available MIT/Apache-2.0 choice. HTTP/2 and TLS are not enabled. Explicit headers/buffers/deadlines are tested; no arbitrary filesystem serving. |
| `tower` 0.5.3; `tower-layer` 0.3.3; `tower-service` 0.3.3; `tokio-macros` 2.7.2; `atomic-waker` 1.1.2; `futures-sink` 0.3.34; `sync_wrapper` 1.0.2; `smallvec` 1.16.4 | Service adaptation, local runtime macros, and bounded asynchronous infrastructure. MIT/Apache-2.0 choices, with sync_wrapper Apache-2.0. Existing Tokio version unchanged. |
| `tokio-tungstenite` 0.29.0; `tungstenite` 0.29.0 | Axum's compatible WS implementation and loopback test client, MIT/Apache alternatives where available. Read/frame/message/write limits, masking, exact Origin/subprotocol, and deadlines are enforced. No compression/TLS extension enabled. Not an authentication implementation. |
| `base64` 0.22.1; `sha1` 0.10.7; `block-buffer` 0.10.4; `crypto-common` 0.1.7; `digest` 0.10.7; `generic-array` 0.14.7; `typenum` 1.20.1; `cpufeatures` 0.2.17; `data-encoding` 2.11.1; `version_check` 0.9.5 | Standard WS handshake/base64/digest and compiler/CPU helpers. MIT/Apache choices, generic-array and data-encoding MIT. SHA-1 is only the required non-authenticating WS handshake, never credentials, identity, encryption, or signing. |
| `rand` 0.9.5; `rand_chacha` 0.9.0; `rand_core` 0.9.5; `getrandom` 0.3.4; `ppv-lite86` 0.2.21; `r-efi` 5.3.0; `wasip2` 1.0.4+wasi-0.2.12; `wit-bindgen` 0.57.1 | WS randomness and target-specific OS entropy/portability, accepted MIT/Apache choices. Installation identity still uses getrandom 0.4.3. The justified getrandom 0.3/0.4 duplicate remains a visible warning alongside syn 2/3; no skip. WASI/UEFI code is locked, not runtime-validated on this host. |
| `thiserror` 2.0.21; `thiserror-impl` 2.0.21; `zerocopy` 0.8.63; `zerocopy-derive` 0.8.63 | Error derive and random-word representation support. MIT/Apache alternatives selected; zerocopy also offers BSD-2-Clause. No Tether unsafe block added by the probe. |

Registry metadata, checksums, relevant features, parser/upgrade bounds, and
new build/proc-macro targets were inspected. New build scripts perform local
compiler/configuration checks or generate build-output files: generic-array,
getrandom 0.3, httparse, thiserror, and zerocopy. Target-only wit-bindgen copies
its packaged WASM support archive to Cargo's output directory; it was not
executed for the Mac path. Macro inputs are local compile-time source, not
LAN data. No external installer/download script, source override, custom TLS,
or compiler-security setting was introduced.

The initial license gate correctly failed on matchit's additional BSD term.
Both bundled license texts were reviewed before the exact package/version
exception and policy/inventory were synchronized. Bans/licenses/sources now
pass; advisories still fail only on R-019's unchanged Wayland transitives. No
advisory finding is ignored. This graph is not evidence that the future
production network server, packaging, or physical Safari behavior works.

## NW-002 internal dependency reuse

The subsequent NW-002 policy model reuses only internal `tether-config` and
dev-only `tether-test-support` =0.1.0 path dependencies. Cargo regenerated those
edges without a new registry package or changed external version/checksum.
That initial model did not select a native provider. The config dependency
makes the existing Windows fail-closed storage boundary
visible to cross-target checks; no security or license exception was added.

### NW-002 macOS provider dependencies — 2026-10-25

The Mac-only provider adds three registry packages: `nix` 0.31.3 (MIT), its
build-only `cfg_aliases` 0.2.1 (MIT), and `objc2-system-configuration` 0.3.2
(Zlib OR Apache-2.0 OR MIT; MIT alternative accepted). Every pre-existing
external version/checksum is preserved. Cargo generated their checksummed
lock entries through the standard official registry, with no source override.
Direct minimum-compatible versions are 0.31.3/0.3.2 respectively; locked builds
use exactly the resolved graph. nix's declared MSRV is 1.69; Apple's bindings
declare 1.71; both compile with the pinned Rust 1.99.0.

nix has defaults disabled and only `net`, `ioctl` and `fs`: safe getifaddrs,
socket/address/descriptor APIs and Darwin request encoding. Existing libc,
cfg-if, bitflags, memoffset and autocfg are reused. Its build script invokes
cfg_aliases to emit compile-time target aliases; neither package adds a network
download, native compiler, external installer or parser for LAN input.
SystemConfiguration defaults are disabled with only std, SCPreferences and
SCNetworkConfiguration plus required CF container types. Core Foundation 0.3.2
and getrandom 0.4.3 are already locked and reused directly for owned CF objects
and OS-random session IDs. No CoreWLAN/SSID, service mutation, route/DNS query,
Objective-C runtime macro, shell command or packet probe is used.

Source review covered nix's ownership, address/netmask conversion (including
the XNU short-netmask workaround), socket/descriptor APIs and build script;
the generated Apple API signatures/ownership, CF type checking and feature
edges; and the small local read-only ABI boundary. Candidate netdev 0.46.3 was
inspected but **not added**: its public IPv6 all-false flags cannot distinguish
a successful preferred-address query from a failed query. No fork or silent
false-safe translation is introduced. The narrower provider preserves unknown
status. Native configuration is trusted OS data, not caller/LAN metadata.

Bans/licenses/sources pass with the existing duplicate warnings; the refreshed
advisory check still fails solely on R-019's unchanged Wayland packages. No new
advisory was ignored. The actual provider, limits, safety boundaries and manual
coverage are recorded in the network contract and dated provider checkpoint.

## NW-003 native socket dependency reuse — 2026-10-25

The Mac-only network adapter promotes already locked `socket2` 0.6.5 and
`tokio` 1.53.3 to direct dependencies with defaults disabled. Tokio enables only
net/rt/time here; existing probe/portal features remain governed by their
original consumers. No external package/version/checksum/source is added or
updated by this step. Only those network dependency edges and dev-only
`tether-input-core` =0.1.0 are added to Cargo.lock. MIT/Apache alternatives and
the existing generated OS bindings/build scripts are unchanged.

Reviewed official cached socket2 ownership/options/clone/shutdown and Tokio
nonblocking from_std/poll_accept/runtime requirements. No local unsafe code,
native installation script or standalone server is added. Socket shutdown uses
an owned duplicate descriptor; Mac loopback tests verify closure, collision and
TIME_WAIT restart with Unix reuse-address but no reuse-port. Windows/Linux
socket behavior is not inferred. Source/license/bans pass; advisories continue
to fail solely on R-019's unchanged packages.

The mdns-sd 0.21.6 archive was downloaded only for DS-002 source review using
official Cargo tooling. It is **not** in the project manifest/lockfile and was
neither compiled nor runtime-started. Its constructor ordering fails the scoped
startup requirement. [The review](reviews/2026-10-25-mdns-candidate-review.md)
records its provenance and proposed decision; this inventory does not approve
that dependency or claim its advisory/native gates passed.

## DS-002 accepted Mac-native boundary — 2026-10-25

Accepted ADR 0003 permits system Bonjour registration after explicit interface
consent/current binding. The source-reviewed `async-dnssd` 0.5.1 archive was
downloaded only through official Cargo tooling, not added to Tether. Its API is
sufficient, but unbounded callback queues/draining do not meet this contract;
see the [binding review](reviews/2026-10-25-bonjour-binding-review.md).

The local, function-scoped unsafe boundary uses six SDK DNS-SD functions through
macOS `libSystem`, with explicit indexes and single-owner pinned contexts.
No extra daemon, external wrapper or registry package/version/checksum is
added. `nix` 0.31.3 gains only its `poll` feature in the Mac discovery crate.
Internal config/network edges use `=0.1.0`; dev-only test-support and the
already locked Tokio 1.53.3 support the opt-in qualification example.
That example's identity uses the production OS entropy path in a private scratch
store, never seeded test randomness or the app's real state.

SDK ABI compilation, inert tests and linking pass. Separately owner-approved
en0/TCP 54322 native registration, local scoped A/AAAA/SRV/TXT queries and
browse/graceful withdrawal pass. Packet containment, native privacy denial
and physical phone resolution have not run.
The [checkpoint](manual-tests/2026-10-25-bonjour-boundary.md) records final gates.
This is not a new-platform support or clean advisory claim; R-019 is unchanged.

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
| `vite` | 8.1.6 | Owner-approved scoped security update; frontend build/test tooling, not a production server. No application web bundle exists yet. |
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

The 2026-10-23 update adds three mandatory compatibility tests: patched
markdown-it reference rendering/malformed input/HTML escaping, the Vite patch
version, and a bounded isolated production-build fixture. That fixture writes
only to a temporary directory/in-memory build output and starts no web server.
These are tool checks, not application/browser or adversarial-complexity proof.

### npm license disposition

Installed metadata counts are 157 MIT, 15 Apache-2.0, 7 BSD-2-Clause,
3 BSD-3-Clause, 7 ISC, 1 Python-2.0, 1 BlueOak-1.0.0, 2 MPL-2.0, and
1 multi-license package accepted under MIT. Review used the installed package
metadata/license texts; no license text was edited or stripped.

The refreshed installed license report retains those same 194 instances and
counts. Vite 8.1.6 and markdown-it 14.3.3 remain MIT with no dependency
install/preinstall/postinstall hooks. Vite's full bundled license file is
byte-identical to 8.1.0, including Apache-2.0, BSD-2-Clause, ISC, MIT, and
CC0-1.0 notices (the latter for `string-hash` and sample-code waivers). Retain
the complete bundled notices if redistributing the tool; this inventory does
not treat its MIT package label as the license of every bundled component.
No new license or bundled-license change is introduced by this patch.

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

### 2026-11-16 isolated protocol commit

Protocol generation is committed in `a5a8da0`; local staging/commit access used
the normal approval flow, without changing `.git` permissions or pushing.
The staged-only Cargo graph was resolved offline with normal Cargo tooling:
exactly the 17 reviewed protocol additions, with every registry version/checksum
unchanged from the reviewed worktree lockfile. Older unstaged Mac/network/probe
dependencies were not swept into that commit.

The isolated snapshot independently passes its aggregate (124 Rust, 29 Vitest,
four Node tests), strict all-feature Clippy/tests/builds, frozen scripts-disabled
npm installation and secret/whitespace checks. The larger worktree aggregate
passes 220 Rust/38 Vitest/four Node tests. Initial offline npm installation lacked
supply-chain metadata; approved official-registry metadata/package acquisition
resolved that failure without a lockfile change or disabled policy check.
No dependency version was changed in this Git batch. Generated TypeScript EOF
is now normalized reproducibly to satisfy staged whitespace checks.

The isolated full-graph cargo-deny 0.20.2 result still fails R-019's three known
Wayland maintenance advisories; bans/licenses/sources pass. No new native
Windows, browser/device, installer or SDK provenance result is inferred. See
the [commit checkpoint](manual-tests/2026-11-16-git-protocol-commit.md).

### 2026-11-14 protocol tool and Windows build preparation

Buf 1.72.0's official Darwin-arm64 standalone executable was downloaded again
to a private task cache and verified against the published SHA-256. Published
per-host executable hashes are now recorded in `tooling/protocol-tools.json`;
only the available Mac binary ran. The checker verifies that hash and version,
uses private temporary cache/bounded subprocesses, and does not install tools,
rewrite the descriptor baseline, upload schemas or download dependencies.
The frozen JSON descriptor is an intentional compatibility test fixture, not
an application dependency/build-cache artifact. Generated sources remain PR-003.

The missing pinned Node runtime was restored from the official Node 24.19.0
Darwin-arm64 archive after matching SHASUMS256, with pnpm 11.25.0 unchanged.
After F-007, the aggregate passes with 214 Rust/29 Vitest/four Node tests.
The complete npm audit now reports zero known findings. Four owner-approved
parent-scoped overrides replace minimatch 10.2.6 with 11.0.1 and add only
`@isaacs/nonrecursive-glob-match` 1.1.0; brace-expansion 5.0.12 is unchanged.
The new package and minimatch are BlueOak-1.0.0 development tools: preserve
license text or the permitted license link on redistribution. Source/exports,
Node compatibility, license texts and explicit preprocessing budget were
reviewed; five regressions exercise actual consumer resolution and file
selection as well as bounded glob behavior. Frozen scripts-disabled install
passes with 225 locked entries. Direct pins and prior overrides are unchanged.
See `reviews/2026-11-14-npm-tooling-findings.md`; R-022 is closed, R-019 is not.
No new application Cargo/npm dependency is added by the protocol schema gate.

PR-003 now selects prost/prost-build/prost-types 0.14.6 (Apache-2.0,
Rust >=1.85), syn 3.0.7 and protobuf-es 2.17.0. Manifests and regenerated
Cargo/pnpm lockfiles contain the exact pins; installation used official
registries, normal integrity checks and no npm lifecycle scripts. The npm
graph now contains 230 entries. No earlier direct pin/override changed.

| New boundary | Review / obligations |
| --- | --- |
| Rust runtime: prost 0.14.6 and its prost-derive 0.14.6 macro | Apache-2.0. Runtime uses std/derive, retaining the normal recursion limit. Reviewed encoding/UTF-8 and skip-Debug generation boundaries; no new application unsafe code. Generated parsing is not domain admission. Keep license/notices in release materials. |
| Rust development: prost-build/prost-types 0.14.6 and syn 3.0.7 | Explicit descriptor generation does not execute protoc. Optional formatting/Markdown cleanup disabled; pinned rustfmt formats the output. AST-driven constant Debug implementations cover every generated type. Macro has no filesystem/process acquisition path; invalid schema may fail compilation. |
| Seventeen new Cargo packages | aho-corasick 1.1.5 (MIT alternative selected), anyhow 1.0.104, either 1.19.0, fixedbitset 0.5.9, foldhash 0.1.5, hashbrown 0.15.5, heck 0.5.0, itertools 0.15.0, multimap 0.10.1, petgraph 0.8.3, the four prost crates 0.14.6, regex 1.13.1, regex-automata 0.4.18, regex-syntax 0.8.11. All except foldhash are Apache-2.0/MIT or selectable under those terms. |
| foldhash 0.1.5 | Zlib, development-only via prost-build → petgraph → hashbrown. Full source notice reviewed: no misrepresented origin, mark altered versions, preserve source notice. Exact license exception only, no global allowlist/advisory waiver. |
| anyhow build script / prost-derive macro | Reviewed the build script's compiler-version/feature probing and scoped OUT_DIR cleanup, and macro generation/skip-Debug boundary. No external install/network script is introduced. Existing syn 2/3 duplication is retained; hashbrown 0.15/0.17 duplication is the new reviewed prost-build constraint, not a reason to force an incompatible upgrade. |
| Browser runtime: @bufbuild/protobuf 2.17.0 | Apache-2.0 AND BSD-3-Clause, not a choice. Read upstream tagged Apache license and packaged Google varint BSD notice. Preserve both notices/conditions/disclaimers/no-endorsement obligations in PK-005. The runtime has no further npm dependency or installation hook. Plain wire objects must never enter runtime logs. |
| Development: @bufbuild/protoc-gen-es and @bufbuild/protoplugin 2.17.0 | Apache-2.0. Read packaged READMEs/options/entrypoint and local generation path. Node >=22 is compatible with pinned 24.19.0. No remote plugin, RPC or validation service introduced. |
| Development: @typescript/vfs 1.6.6 / TypeScript 5.4.5 | MIT / Apache-2.0. License files reviewed. VFS exposes optional CDN utilities, but the reviewed plugin uses the Node-modules map, not its CDN helper; Tether emits target=ts without JS transpilation. Generator transitives do not replace project TypeScript 6.0.3 or ship in browser assets. |

Primary references include the published package/source artifacts, the
[tagged protobuf-es license](https://github.com/bufbuild/protobuf-es/blob/v2.17.0/LICENSE),
its packaged varint notice and Buf's
[local-plugin configuration](https://buf.build/docs/configuration/v2/buf-gen-yaml/).
Six generator and three per-language wire regressions complement, not replace,
PR-004 validation and PR-005 cross-language goldens. The owner's one generated
capability-message Clippy expectation is documented in the generation contract.

The restored cargo-deny 0.20.2 full-graph refresh still fails on the three known
R-019 advisories; bans/licenses/sources pass after the exact reviewed Zlib
disposition. Individual Windows x86-64 and Mac arm64 target checks pass all four
categories, with visible getrandom/hashbrown/syn duplicate warnings. These are
dependency graph checks on this Mac, not native execution or a whole-project
advisory pass.

Owner-approved cargo-xwin 0.23.1 (MIT, Rust >=1.89) was built through
standard locked Cargo in a private tool directory; its version check passes.
LLVM 23.1.5's official Mac-arm64 archive SHA-256 matches the published digest,
and GitHub CLI attestation verification passes for llvm/llvm-project, tag
llvmorg-23.1.5, with the same verified subject digest. Standard trust-cache
access resolved the initial verifier-initialization failure; no trust/TLS check
was disabled. clang/lld version checks and llvm-rc help execute successfully.
R-023 blocks Microsoft SDK acquisition: the official VS 17/18 advertised manifest
hashes/sizes do not match the downloaded bodies. No SDK payload or xwin's omitted
manifest check was accepted. SDK/NSIS installation, Windows linking,
native Windows tests and any Tether executable remain unperformed. Tool binaries,
SDKs and caches will not be committed; concrete provenance is recorded as gates run.

### 2026-10-16 foundation snapshot

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

### 2026-10-23 scoped refresh

- Official, SHA-256-verified Node 24.19.0 and pnpm 11.25.0 were used for frozen
  scripts-disabled installation, source/license review, and a full zero-finding
  npm audit. Only Vite and markdown-it changed versions; every other resolved
  package integrity hash and direct pin is unchanged.
- `cargo xtask check`: passed on the pinned runtime, including 138 Rust tests,
  13 Vitest and four Node tests, frontend types/lint/format, all 64 Markdown
  files, and workspace/skeleton builds. Separate all-feature locked offline
  Clippy/tests/builds and warnings-denied Rust docs also pass.
- Local secret scan passed across 146 text files; `git diff --check` passed.
- Pinned cargo-deny 0.20.2 bans/licenses/sources pass with the reviewed duplicate
  warning. Advisories fail on the three unchanged R-019 Wayland transitives.
  No waiver or dependency migration was introduced.
- No native input posting, real browser/phone, cross-platform runtime, signed
  package, or remote CI validation ran. The Mac construction tests inspect only
  unposted events; the isolated Vite fixture is tooling evidence, not an app.

This inventory makes no claim about future dependencies, unexecuted platform
packages, CI execution, or final release artifacts. A successful local
advisory lookup reflects the database at execution time and must run again in
CI and before release.
