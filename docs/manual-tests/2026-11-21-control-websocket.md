# Windows WebSocket transport checkpoint

An in-memory HTTP fixture submitted a masked binary frame in the same write
as the upgrade request. The server returned 101 with the RFC 6455 accept value
and selected `tether.control.v1`. Its test session received those prefetched
bytes and returned a binary reply. The 101 response omitted a body length and
ignored the compression offer. The same fixture verified quota refusal before
101, text and unmasked frame rejection, and reservation cleanup.

The frame reader accepted exactly 65,536 payload bytes and rejected both an
oversize frame and an oversize fragmented message. Its quota test charged
prefetched bytes to both connection and global limits and fenced pending
bytes after listener retirement. A cancelled read retained its original
deadline. Raw byte budgets include frame headers.
The actor test decoded and validated the generated server hello, control
state update and heartbeat result through the protocol's client-side
validator. The Windows workspace passed 288 Rust tests and strict Clippy.

The pinned production candidate `tokio-websockets` frame source contains no
payload logging calls. The Tokio codec's optional traces record flow and
buffer lengths. That tracing feature is absent from this lock graph. The
network adapter exposes only closed socket errors and keeps payload
representations out of its diagnostics.
[Released frame codec source](https://docs.rs/crate/tokio-websockets/0.13.3/source/src/proto/codec.rs)
[Released Tokio codec source](https://docs.rs/tokio-util/0.7.20/src/tokio_util/codec/framed_impl.rs.html)

The upgraded-socket fixture and the actor verification ran independently.
The subsequent joined [Windows session checks](2026-11-21-windows-control-session.md)
exercise those components together. No phone or native Windows input ran in
this checkpoint.
