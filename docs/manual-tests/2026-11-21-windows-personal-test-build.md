# Personal Windows test executable

The [ADR 0006](../adr/0006-windows-personal-test-build.md) artifact is
`artifacts/Tether-Windows-Test/Tether.exe`. The x64 MSVC release build uses
the pinned Rust and Node toolchains, `--locked`, and the `windows-test`
feature. The executable was copied to an empty directory and verified its
embedded phone interface. Its import table contains Windows system DLLs.
The app uses the installed Windows 11 Edge WebView2 Runtime.

The assembled desktop window loaded in WebView2. Its real, local IPC
returned the network-selection state, rejected a malformed start revision,
refreshed the Windows inventory, and accepted stop while idle. One eligible
private interface was available. The app used an owner-protected LocalAppData
`Tether` directory after separating storage from the framework-managed
browser profile. The published copy opened with its normal WebView2 profile,
created no TCP listener on launch, and closed normally.

Headless Edge on Windows exercised the built mobile page and a loopback Rust
host with protected storage. QR approval, cookie authentication, WebSocket
control, heartbeat, reload reconnect, two touchpad taps delivered to a mock
backend, and neutral input after disconnect passed. The final 300-test Rust
run and the 209 browser tests passed. An earlier Rust run left one record in
the test for rollback of unpublished pairing. The failure did not recur in
25 isolated runs, 12 runs of the concurrent agent-core test group, or the
final workspace run. The cause of that one failure is unresolved. The
Windows-target license, source, ban, and advisory checks passed with the
reviewed dependency notices.

The owner runs the first private-LAN pairing and actual mouse and keyboard
test with a phone. Firewall admission, real Windows input and release,
session-lock and suspend behavior, and `.local` discovery await physical or
isolated native validation. This artifact is unsigned.

## Windows excluded-port follow-up

The owner's first attempt reached private-network selection and failed on
Start. On this host, Windows excluded TCP ports 54236–54335. No application
was listening on 54321. An exclusive bind on the selected Ethernet address
returned Winsock `10013` for 54321. Bind-only checks succeeded for 55001,
57181, and 59631. These probes opened no listeners and changed no network
settings.

The rebuilt window offers those ports for explicit selection. It saves the
selected port and uses it for binding and the QR code. A denied bind shows
guidance to choose another port. A standalone WebView2 smoke test replayed
Start on the reserved port and verified the new guidance, return to network
selection, and absence of a pairing offer. The 302-test Rust workspace run,
strict Clippy, frontend lint, and documentation checks passed. The first
phone retry on this PC should select 55001.

## Start after private firewall approval

The owner selected Ethernet port 55001 and allowed Tether on private
networks. The listener could bind, but the control actor rejected an
earlier timestamp during generation activation and shut down. Recovery now
uses the control inbox's monotonic clock. Repeated supervisor faults keep
the first failure message in the window.

The published executable completed two consecutive starts on the owner's
approved private Ethernet interface at port 55001. Both starts displayed
the pairing QR and returned the embedded phone page over HTTP from the PC
using that Ethernet address. Both stops cleared the QR, closed the listener,
and returned to network selection. The probes used the existing private-only
firewall approval and sent no input. The 303-test Rust suite, including the
control-clock regression, and strict Clippy passed. Physical phone access,
pairing and actual mouse and keyboard control still need testing.

## iPhone Home Screen follow-up

After successful Safari pairing, the owner reported that opening Tether
from its standalone Home Screen icon after closing the Safari tab showed
the connection retry error. During follow-up the desktop was listening
on 55001 and returned the phone page to a request from the PC. That check
does not establish that the Home Screen app can access the listener or
use the existing credential. Compare a fresh Safari tab and the Home
Screen icon at the same address with the listener still running.

iOS can create a Home Screen bookmark by turning off **Open as Web App**.
This opens the page in the browser and provides a workaround if a fresh
Safari tab still controls the PC. Standalone behavior remains unresolved.
[WebKit's Home Screen behavior](https://webkit.org/blog/17333/webkit-features-in-safari-26-0/)

## Pairing renewal

The pairing deadline now rolls forward after authenticated use. The
304-test Windows suite covered renewing a version 1 record, restarting
with the renewed deadline, and rejecting expired credentials. The local
Edge fixture shortened its browser cookie, reconnected over WebSocket,
and verified that the successful upgrade restored the longer expiration.
It used mock input. Renewal in the iPhone Home Screen app has yet to be
verified.

## Movement and scroll latency

The owner reported delayed pointer movement and scrolling on the live
Windows build. The reactor read one input mailbox entry per periodic
16 ms sleep. At higher browser sample rates, serialized WebSocket
admission could leave frames waiting behind that timer.

Accepted control work now wakes the reactor. A periodic timeout still
drives housekeeping and safety when no work arrives. The Windows
workspace tests passed. A browser loopback test used the notified wait
path and confirmed pointer and scroll gestures reached the mock
dispatcher. Perceived latency on the physical phone still needs testing.

## Expanded phone touch surface

The phone layout removes the bottom row of click and modifier buttons.
The touchpad and vertical scroll strip fill the freed height, and the
horizontal strip reaches the usable bottom edge. Browser checks covered
portrait, landscape, both scroll-strip sides, the neutral corner, and
viewport resizing. Tap, drag, scroll, and keyboard checks passed.

On 2026-11-22, browser checks verified that the touchpad and both strips
dim on press in light and dark themes, and that the vertical arrow is
the same text glyph as the horizontal arrow rotated 90 degrees.

## Windows tray and startup on 2026-11-25

On Windows, the packaged executable opened hidden with the sign-in argument.
A tray click revealed the window, closing it hid it, and a second launch
restored the same process. The native popup contained Quit Tether and
dispatching that menu action exited with code zero. The WebView2 check
also queried the startup checkbox through local IPC. An actual Windows
sign-out/sign-in and physical phone control while hidden remain untested.

The published executable also served the mobile page over the selected
private Ethernet port 55001 while its window was hidden. Unit tests exercised
enable, query, repeated disable, and protection of a different executable's
entry in a disposable Windows registry subtree.
