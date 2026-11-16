# Generated protocol sources

`generated/` contains reproducible TypeScript Protocol Buffers sources. Do not
edit them manually. Run `cargo xtask codegen`; `cargo xtask check-generated`
compares fresh offline generation without altering the worktree. See
[`proto/README.md`](../../../../proto/README.md) for pinned-tool setup.

Wire objects are not validated input. Never log/inspect/JSON-stringify them or
pass them directly to transport/session/backend code. Domain conversion and
safe diagnostics belong to PR-004, not generated parsing.
