# Cross-language protocol fixtures

**Task:** PR-005

**Status:** Complete. The locked generation and both fixture suites passed
on [GitHub Actions](https://github.com/nohac14/tether/actions/runs/44968258011)
on Windows, Linux, and macOS.

## Boundary

`tests/fixtures/protocol/v1-golden.json` contains only deterministic synthetic
messages. It is not a second schema, production codec or source of real input.
The `.proto` remains authoritative; generated Rust/TypeScript define all shapes.
Rust and TypeScript independently construct expected values. Both must encode
them to the same frozen known-field bytes and decode the shared wire bytes back
to those values. This covers Rust → TypeScript and TypeScript → Rust compatibility
without accepting a single-language self-round-trip as interoperability evidence.

Each case records an ID, direction/hello phase, raw `wire_hex`, expected
`known_encoding_hex` (or null for codec failure), and the separate Rust admission
result. All nine envelope payload families, twelve operation variants, supported
logical/media/button/edit/scroll enums, capabilities, optional error fields and
nested metadata/limits are represented across 76 cases. Boundary cases exercise
non-BMP/combining/multiline UTF-8, 4 KiB text, aggregate text, 256 operations,
exact 65,536/65,537-byte messages and full-width uint64 counters. Unsupported
versions, unknown enum/union fields, duplicate scalar/payload and truncation
remain rejected by PR-004 even where generated decoding succeeds.

Protobuf serialization is not generally canonical; these are pinned-library
regression bytes, not a protocol requirement for field order or a cryptographic
fingerprint. Alternate legal field order remains accepted by the framing tests.
See the official [serialization guidance](https://protobuf.dev/programming-guides/serialization-not-canonical/).
`known_encoding_hex` specifically describes these constructed known-field values.

prost drops unknown fields; protobuf-es preserves them by default. The TS suite
tests that preservation separately, then uses `readUnknownFields: false` only
for synthetic known-value comparison. That option is not runtime authorization
and does not replace strict union/unknown-operation validation. No browser-domain
validator or network service is implemented by this test suite.

## Verification and deliberate updates

Ordinary `cargo test --workspace --all-targets` and `pnpm test:unit` verify the
corpus. Consequently the existing `cargo xtask check` runs both toolchains and
detects byte/case drift. Rust compares the complete frozen document, including
metadata, exact bytes and case set. TS independently checks the case set and
encodes/decodes each fixture. There is no automatic snapshot update on failure.

After reviewing an intentional compatible schema/codec change:

```sh
cargo xtask fixtures
cargo test -p tether-protocol --all-targets --locked --offline
pnpm test:unit
cargo xtask check
```

The explicit fixture command selects one ignored writer test; that test is a
development command, not omitted runtime coverage. It verifies expected admission
and codec results before replacing one fixed synthetic output file, validates
the small fixed manifest and refuses a symlink/directory output. Ordinary test
runs always execute the nonignored corpus-verification test and never write it.
Generation uses locked offline Cargo and the existing reviewed serde_json
1.0.151 as a dev-only dependency. No runtime dependency or registry pin changes.
Keep the initial Buf FILE baseline unchanged; it is not regenerated here.

No native input, LAN listener, physical browser/device, signed package or remote
CI was validated by this corpus. PR-005 remains incomplete until F-005 runs the
two-language gate in CI; NW-006 stays blocked by that requirement.
