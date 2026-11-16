# Protocol generation contract

**Task:** PR-003

**Status:** Implemented; dated verification in the Windows-first checkpoint

**Authority:** `SPEC.md`, PR-001 contract and `proto/tether/control/v1/control.proto`

## Boundary

The `.proto` is the only wire-schema authority. Generated Rust/TypeScript are
versioned outputs, not handwritten models, authorization, domain validation,
session state or input dispatch. PR-004 owns validation; PR-005 owns the complete
cross-language golden suite. Neither task is implicitly satisfied by round trips.

Pins and official Buf executable hashes are in `tooling/protocol-tools.json`;
Cargo/npm manifests and lockfiles enforce the library versions. Rust 1.99.0's
rustfmt formats the generated Rust. Buf 1.72.0 provides a descriptor directly to
prost-build 0.14.6; no system `protoc`, download helper or build script runs at
application build time. prost-build's optional formatter/Markdown cleanup are
disabled. `syn` 3.0.7 parses output for exhaustive redaction coverage.

The local protobuf-es 2.17.0 plugin emits TypeScript with explicit `.js` imports,
ordinary enum values and bigint uint64s. Its 2.17.0 runtime is the browser's
only new runtime dependency at this task. Plugin/VFS/TypeScript 5.4.5 transitives
are development-only; they do not replace the project's TypeScript 6.0.3 pin.
No RPC, validation framework, remote Buf plugin or cloud dependency is added.

## Reproducible commands

With the pinned Node/pnpm/Rust and checksum-verified `TETHER_BUF` available:

```sh
cargo xtask codegen
cargo xtask check-generated
```

The first writes only the two explicit output files after successful scratch
generation. The second is read-only: regenerate into a new private temporary
tree, then compare file sets and exact bytes. Both remove only their own scratch
directory on success/failure. The initial breaking baseline is never rewritten.
Normal builds consume versioned generated files; ordinary compilation does not
run generation or require Buf/Node. `cargo xtask check` includes the diff gate.
Narrow `.gitattributes` rules keep generated sources, the schema and compatibility
fixture at LF even under a Windows autocrlf checkout; a real `git check-attr`
regression checks both output paths. This is not Windows execution evidence.

Generation uses local schema, plugin and cached Cargo packages only, with Cargo
`--locked --offline`. Missing cache/tools, version/hash mismatch, generator
failure, timeout, malformed output, stale/missing/extra files, oversize files or
symlinks (including ancestors/dangling links) fail the task, never skip it.
Subprocesses have explicit time/output limits; descriptor/output sizes and tree
depth/count are bounded. This is development tooling, not an untrusted LAN API
or a hardened filesystem sandbox against a malicious local workspace owner.

`proto/buf.gen.yaml` is a JSON-shaped valid YAML local-plugin template. Buf
parses it; it must not be parsed as strict JSON after YAML formatting. The
generator prefixes the actual pinned Node executable directory to the plugin's
PATH; no machine-specific absolute executable path is committed.

## Diagnostics and lint invariants

prost's automatic `Debug` is disabled for every message, oneof and enum. The
generator walks the Rust AST and emits a constant `WireMessage(<redacted>)`
implementation for every generated struct/enum, including nested types. It
rejects unexpected generics/external modules rather than silently missing a
type. Tests compare type/implementation counts and exercise nested text,
input-action and envelope diagnostics. Raw library decode errors must also be
mapped to safe errors in PR-004, never logged verbatim at runtime.

TypeScript wire objects are plain inspectable values, not safe diagnostics.
Never console-log, serialize to logs, attach to errors or inspect a real wire
object. Only deliberately fake test values appear in assertions.

Generated prose keeps its content with Rustdoc code-word formatting, and enum
conversion helpers receive `#[must_use]`. The owner approved exactly one
`#[expect(clippy::struct_excessive_bools)]` on generated `CapabilitySnapshot`:
its independent capability flags are mandated by the accepted wire contract,
not an invalid mutually exclusive state. No broad/module/crate lint waiver is
added. A regression asserts exactly that expectation, no `allow`, and unchanged
redaction coverage; strict workspace Clippy remains enabled.

## Verification scope

Rust and TypeScript tests cover UTF-8, full-width counters, both atomic-click
availability values with positive limits, truncated decoding, absent oneofs and
unknown enum retention without claiming domain acceptance. Generation tests
cover exact regeneration, stale/missing/extra files, bounds, symlinks and redacted
diagnostics. These run locally without native injection or a LAN listener.
Compilation, browser/device execution, Windows runtime and complete bidirectional
fixtures are distinct evidence; none may be inferred from these unit tests.
