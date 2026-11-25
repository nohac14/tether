# Mobile control engine checkpoint

## Gesture, scroll, and dispatch evidence

- The 52 focused tests covered both drag modes separately and together,
  per-pointer multitouch, lost capture, neutral-region routing, lifecycle
  interruption, independent signed scroll deltas, bounded momentum, and
  malformed samples. Fake-time tap tests measured zero default recognizer delay
  and a 250 ms maximum opt-in decision window.
- The dispatch suite coalesced 10,000 samples into ten frame sends. It decoded
  generated Protobuf bytes to verify flush-before-action order, exact negotiated
  size limits and sequence after batch splitting. It tested sustained congestion,
  a full queue, suspension, a failed writer and an up event that reached a
  congested buffer. Each fault retired admission and called the mock abort hook.
- Headless Edge 157.0.4341.60 loaded the production layout under a localhost CSP.
  A second fixture mounted the same shell and source modules with a mock
  controlling session and writer. Emulated touch produced a tap and the exact
  button-down, pointer-motion, button-up sequence on drag cancellation. The
  horizontal strip produced only horizontal scroll. The neutral corner and
  disconnected touch produced no input. Changed settings survived reload.
  The production layout passed both strip positions at three viewport sizes and
  the live resize to 320×300 CSS pixels.

## Windows artifact and validation scope

- Rebuilt the x64 release executable with the static CRT. An executable copied
  alone into a fresh directory passed `--verify-mobile-bundle` with System32 on
  PATH. The executable imported only Windows system DLLs.
- The locked offline Windows Rust suite, formatting and strict Clippy passed.
  Frontend build, type checks, ESLint, four Node secret-scanner tests, pinned
  protocol compatibility, and offline generated-source checks passed.
- Vitest passed 177 of 178 tests. The existing dangling file symlink fixture
  failed before its assertion because Windows returned `EPERM`. The test
  remains active.

Authoritative LAN ownership, real WebSocket abort and server release, Windows
input injection, phone hardware and native scrolling behavior, and the final
package were outside this fixture. The production shell starts disconnected.
