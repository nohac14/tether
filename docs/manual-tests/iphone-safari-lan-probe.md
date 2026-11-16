# iPhone Safari Wi-Fi compatibility check

**Task:** BV-001 — complete for the owner's tested iPhone Safari configuration

The owner's [2026-10-25 result](2026-10-25-iphone-safari-lan.md) confirms HTTP
load, a same-origin WS PASS/count on iOS 27.0.1 Safari, and Disconnect stopping
the count, with no prompts observed. The owner subsequently confirmed all four
reconnect/app-switch/Wi-Fi/server-stop checks passed. This completes the early
personal-use gate only, not the finished application or other browser support.

This is a test page, not the finished Tether app. It cannot move the mouse,
type, pair a device, read a clipboard, or access credentials. Do not grant
Accessibility for it. It uses unencrypted HTTP only on your trusted private
Wi-Fi. Do not run it on guest/public Wi-Fi or expose it through a router.

## 1. Select the Mac Wi-Fi address

Connect the iPhone and Mac to the same ordinary private Wi-Fi. On the Mac, open
System Settings → Wi-Fi → Details → TCP/IP and note the IPv4 address and subnet
mask for that selected Wi-Fi interface. Use the actual displayed values, not
the example below. Read only; do not change TCP/IP settings or set a static IP.
No SSID/password/Apple ID is needed. Apple's
[TCP/IP settings guide](https://support.apple.com/guide/mac-help/change-tcpip-settings-on-mac-mh14129/mac)
shows the settings panel.

Convert the subnet mask to the prefix length:

| Actual mask | Prefix |
| --- | ---: |
| 255.255.0.0 | 16 |
| 255.255.254.0 | 23 |
| 255.255.255.0 | 24 |
| 255.255.255.128 | 25 |
| 255.255.255.192 | 26 |
| 255.255.255.224 | 27 |
| 255.255.255.240 | 28 |
| 255.255.255.248 | 29 |
| 255.255.255.252 | 30 |

If yours is different, ask for the conversion rather than guessing. The prefix
controls which peers can reach this test server. Do not choose a VPN, bridge,
wildcard, or unrelated interface address. The initial physical test is IPv4;
IPv6 prefix/URL unit coverage is not physical IPv6 validation.

## 2. Start the explicitly approved listener

In a Mac terminal:

```sh
cd /Users/jl/tether
cargo build -p tether-lan-probe --locked --offline
./target/debug/tether-lan-probe --bind 192.168.1.20:54321 --prefix 24 --approve-private-lan
```

Replace `192.168.1.20` and `24` with the actual Wi-Fi address/prefix above.
Running that final command is your explicit approval to listen on that selected
address and admit its on-link peers for this test. The probe is test-only;
NW-001 subsequently selected the same initial port for the future product.
If occupied, stop and select a different explicit high
port; nothing silently falls back or rebinds. The server prints the actual URL.
It stops after ten minutes/200 accepted connections or when you press Ctrl-C.

If macOS displays a firewall or Local Network prompt, review the specific
terminal/test executable and allow only the access you intend. Do not disable
the firewall, change DNS/TLS settings, or grant general permissions to make it
pass. Record whether a prompt appeared; no prompt is also a valid observation.
CLI permission behavior does not validate the future packaged Tauri app.

## 3. Open the printed URL in iPhone Safari

Enter the complete `http://<actual-Mac-IP>:<active-port>/` in Safari's address
bar. Keep the port; do not search for the URL or substitute HTTPS.

1. Confirm the page says **HTTP page loaded**.
2. Tap **Connect / reconnect**. Expect a same-origin WebSocket pass and a
   JavaScript round-trip count that increases about once a second.
3. Tap **Disconnect**, then **Connect / reconnect**. Confirm the count advances
   again with the same host and port.
4. Switch away from Safari, return, and reconnect manually. Then briefly turn
   the iPhone's Wi-Fi off/on and reconnect after the same Wi-Fi is restored.
   Record any failure; do not claim an unrun case passed. Cellular cannot reach
   the private Mac address and is not a supported transport.
5. If the OS/browser actually exposes a LAN permission prompt or setting, test
   allow/deny/retry only through that specific control when safe. If none is
   exposed, record **no prompt observed; deny/reset unavailable**, not a pass
   for permission denial. Do not reset all system permissions.
6. Stop the server with Ctrl-C in the Mac terminal. A subsequent reconnect must
   fail; no listener should be intentionally left running after the test.

The page auto-stops a connection after 30 seconds and permits six manual
attempts per load. Reload/restart to repeat. These are deliberate test bounds.
The exchange uses browser JavaScript messages, not just network Ping/Pong.

## 4. Report the result

Send only:

- iOS version/build from Settings → General → About (not serial/IMEI/Apple ID);
- Safari version if separately visible, otherwise the iOS version it ships with;
- HTTP page loaded: yes/no;
- same-origin WebSocket pass and increasing count: yes/no;
- reconnect after Disconnect, app switch, Wi-Fi change, and server stop;
- exact non-sensitive error text, and whether any permission prompt appeared.

If using screenshots, crop addresses, network names, notifications, and personal
data where not needed. Do not send cookies, tokens, credential headers, or
unredacted console/network dumps. Optional Safari Web Inspector can diagnose a
failure later; it is not necessary for the initial observation and should not
be enabled broadly in advance.

The accepted evidence closes only the tested personal-use BV-001 scope. It does
not validate `.local` discovery, QR pairing, software keyboard/gestures, native
input, signed permissions, Android Chrome LNA, or other platforms.
