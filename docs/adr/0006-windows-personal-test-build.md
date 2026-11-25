# ADR 0006 — Windows personal test build

**Status:** Accepted by the owner on 2026-11-21.

The owner approved an unsigned Windows x64 executable for the first live
phone test. For this executable, implementation and mock-backed validation
can precede the isolated native-input and Windows `.local` qualification
checks. Both remain required for supported-platform claims.

The test build uses the existing literal-IP QR fallback on the approved
private LAN. It uses the protected device store, local approval, authenticated
WebSocket, single-controller session actor, ordered dispatcher, and reserved
cleanup. The desktop administration surface remains the local Tauri window.
The build uses the Windows WebView2 runtime and requires no Windows feature
change for its own installation.

The original test window closed the service on exit. On 2026-11-25 the owner
requested the Windows tray and opt-in launch at sign-in. Closing the window
now hides it. Quit Tether in the tray runs the existing full shutdown. Its UI supports private-network
selection, local approval, QR rotation, stop, and bulk revocation.

Implement the test composition and native adapter now. Automated verification
uses mock input or inert native-call tests. The owner runs the first live phone
test. Verified startup, released input, and actual operating limits determine
what can later be reported as supported.
