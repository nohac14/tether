# Session ownership and application heartbeat checkpoint

The Rust session actor now reserves room for a validated batch before submitting
any of its actions. It holds one controlling lease at a time, processes scoped
disconnect and global stop signals ahead of normal work, and advances the epoch
after the dispatcher confirms neutral input. A fresh ownership grant checks the
lease, transport generation, backend state and ability to queue control status.

## Automated evidence

- Twenty-one session-actor tests exercised mock execution, protected ownership
  transfer, ordered down/motion/up, malformed or unauthorized frames, revocation, output
  and input saturation, retained text accounting, dropping a transfer target,
  release timeouts, transport retirement, recovery, panic unwind and shutdown.
  Actor/dispatcher tests observed neutral held state on successful
  cleanup. An uncertain release remained faulted.
- Dispatcher regressions verified whole-batch capacity and position reservation
  and the shutdown acknowledgement that follows an active terminal release.
  A recoverable action rejection from a session batch cancelled later queued
  work before the actor could observe the failed receipt.
- The four-second desktop deadline ignored ordinary traffic. Fake time verified
  exact expiry while the lease was still open, renewal from the desktop clock,
  expired takeover cancellation, and clock overflow. Eight TypeScript cases
  verified Protobuf correlation, local RTT, callback suspension, no catch-up
  burst, reconnect, policy validation and send failure.
- On Windows, 263 locked Rust workspace tests passed. Formatting, strict
  workspace Clippy, frontend builds, type checks, ESLint, and pinned protocol
  checks passed. Four Node secret-scanner tests passed.
- The full frontend rerun passed 185 of 186 tests. The longstanding dangling
  file symlink fixture failed during setup with Windows `EPERM`. The fixture
  remains active.

The desktop reactor, cookie verification and WebSocket bridge still need live
composition with these components. Tests used a mock backend and exercised no
native Windows input, physical phones, LAN pairing or clean-machine executable
startup.
