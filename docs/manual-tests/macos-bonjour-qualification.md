# Opt-in macOS Bonjour qualification procedure

**Gate:** DS-002 / R-021 under accepted ADR 0003. **Status: PARTIAL.**

The owner separately approved en0/TCP 54322. Initial Mac registration, exact
scoped local A/AAAA/SRV/TXT queries, browse add/removal and graceful cleanup pass;
the [dated record](2026-10-25-bonjour-boundary.md) preserves the initial failure,
regression fix and actual limits. The additional matrix below is not complete.

This procedure exposes a temporary public hostname/service on the selected
private LAN and opens literal TCP listeners. It does not inject input, prompt
for Accessibility, serve HTTP, authenticate, pair, or change production state.
Do not navigate the diagnostic port on the phone expecting the app to load.
The earlier iPhone HTTP/WebSocket probe did not test Bonjour.

## Consent and safety

Obtain explicit owner approval naming the Wi-Fi/Ethernet interface and port
before running registration or network capture. The example approves **all
eligible current addresses of that one interface**, not every interface on the
Mac. Confirm this is a trusted private network; disable testing if it is public
or shared with untrusted peers. Do not change firewall/privacy settings or use
sudo automatically. Packet capture is a separate permission and can reveal
other devices; never commit packet payloads, capture files or raw network logs.

The current inventory of this Mac shows Wi-Fi `en0`; recheck before a live run.
Do not infer that its IP, prefix, consent or network context stayed unchanged.
The example restricts selection to a current default-eligible interface and
refuses zero/special DNS-SD indexes. It binds every eligible literal address
before registering; any partial bind, stale inventory or DNS-SD error rolls
back rather than selecting an alternate interface/port.

Ordinary automation runs parser/inert tests only. The following is safe without
LAN registration and has already been executed:

```sh
cd /Users/jl/tether
cargo run -p tether-discovery --example bonjour_check --locked --offline -- --help
```

After explicit approval of `en0` and unused TCP port `54322`, run:

```sh
cargo run -p tether-discovery --example bonjour_check --locked --offline -- --interface en0 --port 54322 --approve-private-lan --approve-bonjour
```

The two flags are deliberate opt-ins. No flags/default LAN mode, extra arguments,
wildcard, oversized interface or zero port is accepted. The tool creates an
OS-random test identity in a private temporary directory and removes it on
normal return. It neither loads nor resets actual Tether settings/credentials.
Its hostname changes between runs by design; the product will use ST-003's
persistent identity instead. An abrupt crash can leave scratch state until OS
temporary cleanup; it must never be committed or included in a bug report.

## First native pass

1. Observe any Local Network prompt. Record granted/denied/no prompt, exact OS
   and executable provenance. Do not assume this CLI result validates a signed
   Tauri bundle, its usage strings, upgrade identity or permission persistence.
2. Wait for `READY` (all A/AAAA and service callbacks confirmed). Thirty seconds
   without confirmation fails closed. Record the number/families, port and
   sanitized result; do not persist identity bytes, private addresses or raw
   inventory. A command-line printed public hostname can be used transiently.
3. In a second Terminal, use the printed hostname and its label (without `.local`)
   with the OS diagnostic, scoped to the same interface:

   ```sh
   dns-sd -i en0 -t 15 -G v4v6 <printed-hostname.local>
   dns-sd -i en0 -t 15 -Q <printed-hostname.local> 1 1
   dns-sd -i en0 -t 15 -Q <printed-hostname.local> 28 1
   dns-sd -i en0 -t 15 -L <printed-label> _tether._tcp local.
   ```

   Replace angle-bracket placeholders; do not type them literally. Confirm exact
   current addresses, SRV target/port and the three public TXT items. A local
   resolver/cache result alone does not prove remote traffic containment. The
   CLI may add a percent/interface display suffix even to an IPv6 address;
   do not mistake that diagnostic notation for permission to use zone-ID URLs.
4. Press Ctrl-C in the example. Confirm `STOPPED`, no remaining listeners, and
   scoped browse removal/zero-TTL goodbye where observable. Peer cache removal
   is best effort, not a hard promise. The normal run ends after ten minutes;
   synchronous OS calls can still stall during daemon/OS failure. A stuck call
   must not be concurrently freed or moved to another thread.
5. Repeat start/stop and abort during registration. Confirm no automatic restart
   after inventory change/failure. Never enable automatic test relaunch.

## Additional required evidence, not covered by a first pass

- Approved-vs-unapproved interface packet containment, including AWDL, VPN,
  loopback and any alternate active physical link. Capture/query traffic must
  distinguish the test's fresh name from unrelated OS Bonjour activity.
- Multiple A/AAAA records on one interface and on multiple explicitly approved
  interfaces, full rollback and no unbound/temporary address publication.
- Remote physical resolution on the owner's iPhone/Safari with the later
  bounded HTTP probe/production host integration. This diagnostic deliberately
  has no web route, so phone navigation cannot establish that gate yet.
- Deliberately isolated name conflict, no implicit rename, and cleanup after
  asynchronous conflict. Do not impersonate an existing user's service/name.
  Bounded alternative selection belongs to DS-003, not this single-attempt tool.
- Permission deny/revoke/restore and daemon unavailability, without resetting
  privacy databases or killing system services without separate approval.
- Physical DHCP/interface flaps and IPv4/IPv6 reachability, followed by clean
  withdrawal and explicit restart. The diagnostic aborts, rather than rebinding.
- Packaged stable identity/Local Network configuration and actual signed/public
  deployment gates as applicable; public distribution remains deferred.

Record each result as pass/fail/not tested with provenance in a new dated
manual record. Do not mark DS-002 DONE or begin DS-003 merely because the first
registration succeeds. No packet capture ran. The initial native Mac checks
are recorded separately from the still-unperformed phone and permission gates.
