# Windows authenticated control session

The in-memory supervised HTTP fixture used temporary owner-protected storage
and the real session actor with the mock input backend. It submitted a QR
request, approved it locally, completed the request and sent the issued
cookie to the WebSocket endpoint. The actor reserved a connection before
101. The test negotiated hello, requested control, delivered one held mouse
button and received an application heartbeat echo. Disconnect retired the
lease and returned the backend to neutral. A duplicate connection for the
same device received 429 while the first reservation was still active.

Incompatible version and malformed hello tests received the fixed numeric
close codes, with zero mock input. Existing tests cover missing or invalid
Host or Origin, nonce/framing checks, malformed or revoked credentials,
global lease capacity and per-peer upgrade limits.

The built mobile site ran in Edge 158.0.4370.33 against the real Rust HTTP,
pairing store, WebSocket and actor on Windows loopback. An isolated Edge
profile opened a fresh QR. The page scrubbed the fragment, received an
HttpOnly cookie, gained control, and sent a trusted touchpad tap. Reload
reconnected from the existing cookie, stayed in control past a heartbeat
interval, and sent a second tap. The fixture observed exactly two mock
clicks and neutral state when the final socket closed. The fixture's
approval and supervised adapter were synthetic and its input was mock.

The rate regression exhausted the authenticated message burst with a button
held and confirmed reserved release. A second test exhausted the shared
expanded-operation budget across multiple connections. The Windows Rust
workspace passed 292 tests and the frontend passed 209 tests. Physical phone
interaction and native input were outside the scope of this run.

The desktop executable still needs its composition root, native backend and
physical phone verification. This test used no external network or native
input injection.
