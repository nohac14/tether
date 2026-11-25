# Ordered mobile input

The browser dispatch queue is scoped to one current session and ownership epoch.
It accepts gesture and strip output and semantic button, key, text, command,
ownership and release operations. Recognizers emit CSS-pixel motion and normalized
scroll. Pointer sensitivity and optional bounded linear acceleration apply to
each motion sample before accumulation. The desktop can separately tune native
translation and should use unity normalization for already configured client gain.

Consecutive pointer samples or same-axis scroll samples can share a run. Changing
axis or motion kind preserves relative order. A state-changing operation drains
earlier runs before its operation. An ownership or release barrier closes the
local queue, so a new epoch creates a new neutral instance. The wire uses signed
1/1024 deltas. Each axis keeps an independent signed fraction across frame drains.
A state-changing barrier discards sub-wire fractions before they can cross it.
Anomalous samples clamp to the negotiated range and discarded overload carries
no remainder. Platform pointer and wheel units belong to the native backends.

The queue has at most 32 pending runs and drains on actual animation frames.
It respects negotiated operation, delta, text, and atomic-click limits. The
connection's envelope writer checks the exact encoded byte length against the
negotiated ceiling before each send. An oversized batch may split in its original
order. One unsendable operation retires the queue. The connection keeps sequence,
epoch and timestamps and also manages heartbeats through its shared writer.

Below 32 KiB of browser-buffered data the queue can send. At that threshold it
coalesces motion and scroll for at most 80 ms. A barrier reaching a congested
connection, a full pending queue, a failed send, a sustained stall or 128 KiB
buffered data retires admission and calls the reserved connection-abort hook.
That hook closes or resynchronizes the session via the server's safety path.
Ordinary WebSocket data cannot jump ahead of its own send buffer. The server's
disconnect and application-heartbeat deadlines remain the eventual release path
if the browser cannot reach the server immediately.
