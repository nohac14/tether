# Protocol schemas

`tether/control/v1/control.proto` is the sole wire-schema authority for the
version-one control protocol. `buf.yaml` applies the current Buf v2 `STANDARD`
lint and `FILE` breaking policy without comment-based ignores.

PR-002 owns schema semantics and validation. PR-003 will add pinned Rust and
TypeScript generation, checked-in generated sources, and a temporary-tree
generated-diff gate. Do not hand-edit generated sources or treat generated
protobuf parsing as domain validation.
