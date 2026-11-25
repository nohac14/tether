# Windows control upgrade preflight

On Windows MSVC, in-memory HTTP connections drove the supervised adapter and
protected device store. An approved phone's cookie reached the verifier, an
unknown credential returned 401, and revocation caused the formerly valid
cookie to return the same 401. The browser Host and Origin checks ran before
the lookup. Tests cover missing, mismatched, and duplicate Origin, wrong
Host or route, ambiguous upgrade headers, a malformed handshake nonce, and
missing, malformed, or duplicate cookies.

The actor test reserves a lease before hello, refuses a second lease for the
same device and accepts a retry after observing the first lease disconnect.
The rate test confirms that IPv4 and IPv4-mapped IPv6 share the upgrade burst
budget. Test input is synthetic and the actor uses a mock backend.

The locked all-target, all-feature Windows workspace suite passed 283 tests.
The existing manual corpus writer remained ignored by the ordinary suite.
Rust formatting and strict Clippy passed.

Successful cookie verification still returns a closed 503 response. No
WebSocket frames, real phone, or Windows input injection ran in this check.

## Frame transport review

Tungstenite 0.30.0 offers frame, reassembly, and write buffer limits and the
Tokio wrapper can take over a stream after the existing HTTP parse. The
upstream frame reader and writer emit trace records that format the entire
payload as hexadecimal. An unchanged production integration therefore needs
an enforced suppression boundary verified even at the application's highest
diagnostic level. The dependency remains unselected. The LAN probe's earlier
Tungstenite use carried synthetic messages.
[Tokio stream handoff](https://docs.rs/tokio-tungstenite/0.30.0/tokio_tungstenite/struct.WebSocketStream.html)
[Frame read and write source](https://docs.rs/tungstenite/0.30.0/src/tungstenite/protocol/frame/mod.rs.html)
[Frame display source](https://raw.githubusercontent.com/snapview/tungstenite-rs/v0.30.0/src/protocol/frame/frame.rs)
