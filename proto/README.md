# Protocol schemas

`tether/control/v1/control.proto` is the sole wire-schema authority for the
version-one control protocol. `buf.yaml` applies the current Buf v2 `STANDARD`
lint and `FILE` breaking policy without comment-based ignores.

PR-002 owns schema semantics. PR-003 implements pinned Rust and TypeScript
generation, versioned generated sources and a temporary-tree generated-diff
gate. See the [generation contract](../docs/components/protocol-generation.md).
Do not hand-edit generated sources or treat protobuf parsing as domain validation.

PR-002 now represents atomic-click availability explicitly as field 21, retaining
reserved 14–20. The positive click-count limit is not capability authorization.
`tests/fixtures/protocol/v1-baseline.json` is the intentionally versioned initial
FILE-compatibility descriptor fixture, not an application schema to edit or a
build cache. It was generated with the pinned Buf from this initial schema;
normal checks never regenerate it. A future baseline replacement requires
explicit compatibility review, not a build-time refresh.

Set `TETHER_BUF` to the absolute official Buf 1.72.0 standalone executable.
`tooling/protocol-tools.json` records the published per-host SHA-256 values;
`pnpm check:protocol` verifies the executable before running local format, lint,
build and FILE breaking checks. It downloads nothing, uses a private temporary
cache and has bounded subprocess time/output. `cargo xtask check` includes it.
Ordinary `pnpm test:unit` now also requires that tool for actual breaking-change
regressions: deleting the click field fails, adding an unrelated field passes,
and the baseline remains unchanged. Missing tools/checksum failures are errors,
not skipped tests. Binary/tool caches and developer-specific paths are not
committed. `cargo xtask codegen` explicitly updates the two generated files;
`cargo xtask check-generated` compares fresh offline output without rewriting
them. Normal builds do not require generators. Domain validation and the full
cross-language fixture suite remain PR-004/005.
