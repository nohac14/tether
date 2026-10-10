# Repository task runner

Run tasks from the repository root with `cargo xtask <command>`.

- `check` runs non-destructive Rust and web quality gates; it never runs native
  input injection.
- `codegen` requires the reviewed Buf configuration created by PR-002.
- `fixtures` requires the fixture manifest and generator created by PR-005.
- `package-preflight` verifies the repository inputs required before packaging
  work; packaging tasks will extend it with platform-specific checks.
- `manual-test-guard <scope>` only validates the explicit acknowledgement and
  named target. Passing the guard is not evidence that a native test ran.

Native/manual runners must require
`TETHER_MANUAL_TEST_ACK=I_UNDERSTAND_TETHER_INJECTS_INPUT` and a non-empty
`TETHER_MANUAL_TEST_TARGET`. They must also record actual environment evidence
under `docs/manual-tests/`.
