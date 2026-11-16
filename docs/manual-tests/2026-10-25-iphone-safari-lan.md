# Physical iPhone Safari LAN probe — 2026-10-25

**Task:** BV-001 — **DONE for the owner's tested personal-use configuration**

**Evidence source:** Owner's direct report in this chat, not an agent-observed
browser session, captured packet trace, or inferred automated result.

## Configuration and provenance

- Controller: owner's physical iPhone using Safari.
- iOS: **27.0.1**, owner-reported. Separate Safari version, iOS build number,
  and phone model were not supplied; no identifiers were collected.
- Host: the existing Apple Silicon Mac development checkout. The previously
  observed host was macOS 15.7.5 arm64; that version was not re-observed during
  the owner's phone run.
- Probe: uncommitted BV-001 implementation associated with the
  [2026-10-25 preparation checkpoint](2026-10-25-iphone-probe-preparation.md),
  based on repository commit `94d8e49`. No per-run executable hash or exact
  rebuild was independently attested by the owner.
- Package/signing identity: development CLI probe, not the packaged Tauri app;
  no stable signed-identity or permission-persistence result.
- Topology: selected Mac Wi-Fi IPv4 address with a /24 subnet, phone and host
  following the same-private-Wi-Fi procedure. Test URL uses port **54321**;
  the private address, network name, and router identifiers are not persisted
  in this record. No `.local`, IPv6, VPN, or routed-LAN result is claimed.
- No input backend, pairing, credentials, or workstation injection capability
  exists in this probe.

## Owner-reported observations

The owner reported that the page loaded, displayed PASS, increased the round
trip count after Connect, and stopped the count after Disconnect. They reported
no permission prompts. In a subsequent explicit reply they reported **pass for
all** four requested checks: second Connect after Disconnect, return/reconnect
after an app switch, return/reconnect after Wi-Fi off/on, and failed Connect
after Ctrl-C stopped the server. These are owner-observed results, not agent
automation or evidence of the finished application.

| Case | Result | Evidence boundary |
| --- | --- | --- |
| Direct HTTP page load on physical iPhone Safari | **PASS — owner-reported** | Page loaded at the supplied IP/active-port test address. No browser/network trace captured. |
| Same-origin LAN WebSocket upgrade | **PASS — owner-reported** | The unchanged probe constructs its WS authority from the actual page host/port; its PASS state requires the expected subprotocol and application response. |
| Browser-JavaScript application round trips | **PASS — owner-reported** | Count increased after Connect. This is not merely TCP/WebSocket Ping/Pong activity and is not input-delivery acknowledgement. |
| Explicit browser Disconnect | **PASS — owner-reported** | Count stopped. This establishes test-UI behavior, not production ownership or native release-all. |
| Permission prompt observation | **No prompt observed — owner-reported** | No prompt reported for this run. It does not prove permission denial/reset behavior, that no permission had been granted previously, or that every OS/browser/configuration is exempt. |
| Connect again after explicit Disconnect | **PASS — owner-reported** | Owner explicitly confirmed the requested second Connect resumed the count. |
| Safari app switch, return, manual reconnect | **PASS — owner-reported** | Owner explicitly confirmed the requested physical sequence. No automatic reconnect or input-state claim. |
| Wi-Fi off/on, same-network return, manual reconnect | **PASS — owner-reported** | Owner explicitly confirmed the requested physical sequence; not a DHCP or cross-network test. |
| Mac Ctrl-C server stop, subsequent Connect fails | **PASS — owner-reported** | Owner explicitly confirmed the requested physical sequence. No native release-all capability exists in this probe. |
| Permission deny/reset | **Not exercised** | No exposed prompt/setting was reported. Do not reset broad OS permissions merely to create this case. |
| `.local`, discovery, pairing/cookies, software keyboard, touch gestures, native input | **Not tested by this probe** | These remain their own implementation and verification gates. |
| Android Chrome, iPad, Brave, other OS/browser runtime | **Not tested** | ADR 0002 does not turn an iPhone result into an expansion-platform claim. |

## Gate disposition

The initial physical HTTP and same-origin WS feasibility result is positive
for this owner's iPhone Safari/iOS 27.0.1 configuration. With all four follow-up
checks explicitly confirmed, BV-001 is **DONE** under ADR 0002 and NW-001 is
dependency-ready. R-012 remains open for packaged identity, permission denial
where testable, later browser versions, and Android/other expansion targets.
No acceptance criterion was removed or silently marked passed.

The [existing procedure](iphone-safari-lan-probe.md) remains reproducible for
future retesting. The absence of a prompt is an
actual observation, not a failed test or a reason to add an invented browser
permission query. Packaged macOS Local Network privacy and Accessibility remain
separate untested concerns. MAC-002 is not complete and no live input occurred.

R-019's Wayland advisory failure and R-018's Windows storage boundary remain
unchanged. No authentication, pairing, session, network-server, or native-input
implementation is authorized by this positive test result alone outside its
documented dependency gates.

## Repository verification for this evidence update

This phone evidence record changes documentation only; no phone result is
invented from the automated suite. The subsequent NW-001/NW-002 source changes
and actual workspace/doc/security verification are recorded separately in
[the network foundation checkpoint](2026-10-25-network-foundation.md). Earlier
preparation results remain dated historical evidence, not a substitute for
that run or the owner's physical observations.

No credentials, private URLs, addresses, phone identifiers, raw frames, input
data, or machine-local state are committed. `.git` remains read-only; no commit
or push is attempted. Include this evidence file with the other documentation
when using the current network checkpoint's manual commit commands.
