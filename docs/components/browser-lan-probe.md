# Early physical-browser LAN probe

**Task:** BV-001, complete for the owner's tested iPhone Safari configuration

The [owner-reported iPhone evidence](../manual-tests/2026-10-25-iphone-safari-lan.md)
records successful HTTP/WS application round trips and manual Disconnect on
iOS 27.0.1 Safari, with no prompt observed, plus explicit follow-up passes for
reconnect after Disconnect/app switch/Wi-Fi change and failure after server
stop. This completes the early BV-001 personal-use gate, not packaged privacy,
production lifecycle/input safety, native input, or other browser support.

**Authority:** `SPEC.md` §§2.6, 15.3, 17, 20, 34; ADR 0002

The private `tether-lan-probe` workspace executable lives only under
`tests/e2e/lan-probe`. No application crate depends on it. It serves embedded
test HTML/CSS/JavaScript through Axum/Tokio/Hyper, the intended HTTP stack. It
has no input backend, production protocol, credentials, pairing, storage,
discovery, or administration dependency. It must never ship in the application.

Startup requires either explicit loopback-only mode or a literal unicast local
IP/port, actual interface prefix length, and `--approve-private-lan`. Wildcard,
multicast, mapped-IPv6, scoped-link-local IPv6, and zero-port LAN bindings fail
closed. LAN mode requires a non-loopback address and a conservative prefix
length (IPv4 /16–/32, IPv6 /48–/128). The operator must verify that prefix against
the selected interface before starting. Binding a specific assigned address
fails if it is no longer local; the probe does not auto-rebind or auto-listen.
Only peers matching that prefix are admitted. Loopback mode admits loopback
only and is the sole mode used by automated tests. This is test exposure, not
permission to enable production automatic listening.

The server accepts at most 200 connections over ten minutes, paces accepts to
at most 50/second, and bounds HTTP and WebSocket concurrency separately at four.
HTTP/1 uses at most 32 headers/8 KiB, a five-second header deadline, ten-second
connection deadline, and no keep-alive. Only bodyless GETs to the exact three
asset paths and `/probe` are supported; queries, bodies, unexpected Host, and
foreign Origin fail closed. WS requires exact same-origin Origin and the
test-only `tether.lan-probe.v1` subprotocol. Compression is not enabled.
WebSocket read/frame/message/write buffers are bounded; a session lasts at
most 40 seconds/60 messages, with five-second receive and two-second send
deadlines. Only the fixed `probe-ping`/`probe-pong` text exchange is accepted;
control Ping/Pong does not count as browser-JavaScript success. Nothing echoes
arbitrary peer input. Shutdown closes test sockets; no native cleanup is needed
because there is no input capability.

The browser constructs `/probe` from `location.protocol` and `location.host`,
preserving the actual port and IPv6 brackets. It has a manual Connect/Reconnect
button, bounded retries/counters/deadlines, no automatic reconnect after page
hide, and displays safe success/failure milestones without payload logs or
device identifiers. It uses only local assets, external scripts/styles, a
restrictive CSP, no-store, no-referrer, nosniff, and frame denial. It collects no
typed text, cookies, secrets, arbitrary messages, or persistent telemetry.

Physical iPhone Safari results must name actual OS/browser versions and observed
permissions. A command-line process's macOS Local Network behavior does not
establish packaged Tauri permission behavior. iPhone success does not validate
Android LNA, `.local` discovery, pairing, keyboard/gestures, native injection,
or production security. Those retain their own gates.

API review uses official [Axum 0.8.9](https://docs.rs/axum/0.8.9/axum/),
[WebSocket limits](https://docs.rs/axum/0.8.9/axum/extract/ws/struct.WebSocketUpgrade.html),
and [Hyper HTTP/1 builder](https://docs.rs/hyper/1.12.0/hyper/server/conn/http1/struct.Builder.html).
Apple's [Local Network privacy technote](https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy)
remains the authority for OS privacy behavior; actual browser behavior requires
the physical test, not an assumed prompt model.
