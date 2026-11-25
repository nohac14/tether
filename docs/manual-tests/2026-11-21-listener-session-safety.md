# Listener retirement and protocol CI checkpoint

The listener supervisor and session actor now share a reserved cleanup ticket.
The integration test kept a button held through the mock backend, requested a
rebind, and observed the supervisor waiting for session closure and neutral
input before publishing a fresh generation. The old generation could not be
reopened. The same fixture tested first publication without a prior generation
and shutdown after the replacement listener.

Three additional actor tests exercised a dropped request handle, a pending
retirement superseded by shutdown, and a release that did not complete before
its deadline. A timed-out request reported uncertain cleanup and retained the
actor fault. A dropped actor settled pending tickets as unavailable. These tests
use synthetic transport and mock input.

The protocol workflow configures Windows, Linux, and macOS jobs for schema
compatibility, locked offline generation, and both shared fixture suites. The
jobs pin checkout and Node setup to immutable commits and verify the matching
Buf executable against the repository's platform checksum before running it.
[Checkout commit](https://github.com/actions/checkout/commit/de0fac2e4500dabe0009e67214ff5f5447ce83dd)
[Node setup release](https://github.com/actions/setup-node/releases/tag/v6.2.0)
[Buf release assets](https://github.com/bufbuild/buf/releases/expanded_assets/v1.72.0)

The workflow's protocol checks ran locally on Windows with the reviewed pinned
tools. A frozen install in a fresh copy used relative package symlinks. Vite
failed to resolve the pinned Protobuf runtime through those links. Preserving
symlink import paths on Windows made all 80 TypeScript protocol tests pass in
that checkout. Root Vitest also excludes the ignored build and scratch
directory so copies of the fixtures do not run a second time.

The ordinary Windows workspace suite passed 280 Rust tests and strict Clippy.
The approved branch `codex/protocol-ci` published the workflow and Vitest
configuration in commit `0156ba3`. All three hosted jobs passed
schema compatibility, offline generation and both fixture suites on
Windows 2025, Ubuntu 24.04 and macOS 15. This closes the PR-005 CI gate.
[GitHub Actions run](https://github.com/nohac14/tether/actions/runs/44968258011)

The changes establish no native input, live authenticated WebSocket behavior or
phone-to-PC launch and pairing result.
