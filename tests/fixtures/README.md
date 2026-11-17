# Test fixtures

Only deterministic, synthetic, non-sensitive fixtures belong here.

The version-one protocol corpus is shared by independent Rust/TypeScript tests.
`cargo xtask fixtures` deliberately regenerates it after review; normal aggregate
checks verify it without rewriting. See the
[fixture contract](../../docs/components/protocol-fixtures.md). The separate
`protocol/v1-baseline.json` is the frozen Buf compatibility baseline, not a
regeneratable golden output.
