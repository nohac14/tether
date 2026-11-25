# Windows listener supervisor checkpoint

The restored supervisor drives the Windows listener through inventory,
reserved input release, candidate binding, and post-bind validation. A stop
or rebind revokes the active transport generation immediately. It reuses one
non-cancelling release ticket with a two-second deadline. Repeated stop
requests clear any queued start. Confirmed cleanup closes the old socket
set before a replacement binds. Cleanup failure or timeout closes sockets
and latches the supervisor in a faulted state.

Windows inventory queries stay on one worker. The supervisor polls the
worker without waiting, refreshes before and after binding, and requests
another observation at 500 ms of idle time or after accepting a transport.
Outstanding refreshes pause new acceptance. Timeout and cancellation retire
the request authority. The next refresh discards the late response.

## Evidence

- Real Windows IPv4 loopback connected through the supervisor's Windows TCP
  primitives using a fixture inventory and controlled release acknowledgement.
  The test observed immediate logical revocation, the draining state while
  cleanup was pending, and EOF after the supervisor closed the retained
  connection. The production driver's inventory remained read-only in its
  separate native diagnostic.
- The real input dispatcher with a mock backend held one mouse button, then
  processed a reserved release after the supervisor was dropped. The
  original ticket confirmed neutral input.
- Fault tests covered pre/post-bind inventory changes, refresh failure
  while idle, stop interrupting a pending refresh, deadline retention,
  duplicate stop requests, failed cleanup submission, non-neutral release,
  shutdown errors and panic, registry saturation, and generation exhaustion.
- The offline, locked, all-target/all-feature Windows suite passed 227
  tests, including all 40 network tests. The protocol fixture writer
  remained opt-in. This run used a standard Windows user token.
- Formatting, strict workspace Clippy, the workspace build, locked metadata,
  no-dependency network rustdoc, and the native inventory diagnostic
  passed. The Windows-target dependency audit passed. The all-target audit
  still flags the three existing R-019 Wayland advisories.

The production core still needs to supply the session/input safety coordinator
and drive the supervisor while idle. The HTTP runtime must retain each
connection's authority guard and Windows quota permit through teardown.
Physical LAN, active same-port Windows rebind, production session cleanup,
guarded input posting, firewall behavior, and packaging remain untested.
