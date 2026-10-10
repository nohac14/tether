# Test support

Deterministic, dependency-free test utilities shared across Rust components.
This crate is test infrastructure and must not be linked into release binaries.
Its random source is deliberately deterministic and is never suitable for
credentials, pairing tokens, or other production security decisions.
