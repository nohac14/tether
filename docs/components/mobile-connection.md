# Mobile control connection

The browser derives `ws://<current-origin-authority>/control` from the page
and offers `tether.control.v1`. The browser attaches the host-scoped device
cookie. Application code reads neither the cookie nor a credential from
storage. Pairing approval starts the connection after the completion response.
Returning to the same host opens a fresh connection.

The client accepts one binary server hello with compatible version, limits,
capabilities, control state and heartbeat policy. It claims an idle desktop
at entry. Another controller remains active until the user taps Take control
and the server grants the new epoch. Gestures and keyboard input use only
capabilities advertised for the current controller. All outgoing application
messages share one sequence, timestamp and epoch source.

Page hide sends a best-effort leaving message and drops the socket, input
queue, timers and keyboard state. A return or reconnect creates a new
connection with no held input. Requests retry with bounded exponential
jitter, with a Retry action once the browser exhausts those attempts. The
browser can display confirmed protocol incompatibility and authorization
loss. It cannot inspect a failed WebSocket upgrade's HTTP status, so an
opaque connection failure offers retry and a fresh QR as applicable.

The [Windows loopback record](../manual-tests/2026-11-21-windows-control-session.md)
describes the built app's headless Edge test against the real store and
mock input actor.
