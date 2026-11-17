# Protocol fixture milestone — 2026-11-16

## Implementation and boundary

PR-005 now has a 76-case shared synthetic corpus (482,353 bytes), independent
Rust/TypeScript expected-value constructors and bidirectional encode/decode
checks. It covers all client/server families and operations, known enums,
optional-zero presence, unknown enums/fields, duplicate scalar/payloads,
truncation, full-width counters and hard size/text/count boundaries.

The only Cargo addition is an exact dev-dependency on the already reviewed
serde_json 1.0.151; no new registry version, transitive or production dependency.
The explicit frozen-manifest fixture writer is locked/offline, checks expected
codec/admission results before writing and never changes the initial Buf FILE
baseline. Ordinary gates verify the frozen corpus without rewriting it. The
ignored writer is a deliberate development command, not skipped runtime coverage;
it was explicitly executed and passed.

See the [fixture contract](../components/protocol-fixtures.md). Protobuf itself
is not canonical; pinned regression encodings do not impose receiver field order.
TS's default unknown-field retention is tested separately from known-field
comparison. Neither generated decoder alone authorizes input.

## Local verification

Host/toolchain: existing Apple Silicon Mac, Rust/Cargo 1.99.0, Node 24.19.0,
pnpm 11.25.0 and checksum-verified Buf 1.72.0.

- `cargo xtask fixtures`: passed, explicitly executing the sole synthetic writer.
- Rust fixture/admission verification: all 76 cases passed; existing 24 protocol
  tests passed. The writer is reported separately as an ignored command in an
  ordinary run, not counted as an unexecuted safety test.
- TypeScript fixture suite: 77 tests passed (76 cases and exact case-set test).
- TypeScript types/strict ESLint: passed after correcting fixture import depth
  and using generated enum members rather than raw numeric assignments. No lint
  policy/enum checks were weakened and no non-null assertion remains.
- Workspace/all-target/all-feature locked offline Clippy/build: passed.
- Full worktree aggregate: passed with 244 Rust tests before the additional
  xtask command-selection regression, 115 Vitest tests and four Node scanner
  tests; includes schema/generation, types/lint/format/docs/links and foundation
  builds. The later xtask regression is verified separately and in final gates.
- Protocol Windows x86-64 all-target compilation and no-dependency docs: passed;
  not Windows linking, execution or native/device validation.

The initial aggregate correctly failed strict enum/template/non-null assertion
linting; it was corrected and rerun successfully. Existing R-019/R-018/R-023
remain open. This batch adds no native input, production LAN listener, cookie
service or Windows artifact. No remote CI has run: PR-005 stays IN PROGRESS
until its required F-005 CI acceptance succeeds. Other dependency-ready work
such as SS-002 does not depend on that remote protocol gate.

Final locked all-feature/all-target workspace checks passed: 245 Rust tests,
strict Clippy and builds. The one ordinary-run ignored item remains the explicit
writer command, which ran successfully. The final isolated staged snapshot
passes the aggregate with 149 Rust tests, 106 Vitest tests and four Node scanner
tests; it excludes older unstaged platform/network code. Its Cargo-generated
lock delta is only the protocol dev-dependency reference to existing serde_json.

In that owned temporary snapshot only, changing the corpus version from 1 to 2
made the Rust frozen-document check fail (exit 101) and the TS case-set/version
test fail (exit 1, with the 76 wire cases still passing). The explicit writer
restored the original synthetic file byte-for-byte (`cmp` passed), and the
whole isolated aggregate then passed. The main worktree corpus was never
corrupted or rewritten by that negative test. No frozen baseline changed.

Final worktree secret scanning passed: 235 text files, zero binary skips.
Pinned cargo-deny 0.20.2 using the existing advisory cache still fails on the
three R-019 advisories; bans/licenses/sources pass. Its advisory database was
not refreshed online, and no ignore or dependency-security waiver was added.
