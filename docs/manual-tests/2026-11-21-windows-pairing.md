# Windows pairing and native record checkpoint

## Pairing across the browser and desktop boundaries

The Windows integration fixture put the HTTP pairing routes, bounded queue,
desktop approval coordinator and protected credential store on one exercised
path. A synthetic supervised transport carried the real HTTP/1.1 bytes. The
first reply supplied a transient completion proof. Approval from the local
coordinator then produced a host-only, HttpOnly cookie after durable storage.
Replaying completion returned 410. The queue test rejected a ninth waiting
request and verified that cancelled requests did not consume the QR. After a
durable write, cancellation, authority retirement, clock regression and expired
approval each removed the unpublished credential. A paired credential
also drove the real session actor with a mock backend. Revocation released held
input, closed its lease, and rejected a proof already queued for registration.

Headless Edge 158.0.4370.33 ran the built mobile page with an independent
localhost HTTP fixture. It opened a QR link in an existing tab, removed the
fragment, posted once, observed a pending response and then accepted the cookie.
The cookie was available to `/control` with HttpOnly, SameSite=Strict and the
configured path. The same run verified both scroll-strip sides at three viewport
sizes and a live viewport resize. The phone uses the default display label
`Phone` at this stage.

## Win32 record and executable checks

Eleven Windows mapper/record tests verified lowering to the maintained Win32
bindings, including signed wheel data, scan and extended-key flags,
and UTF-16 surrogate ordering. The diagnostic executable refused to run
unarmed. An armed invocation moves the pointer of the isolated interactive
test session. Actual `SendInput` insertion did not run on this workstation.

The rebuilt x64 release executable again passed embedded-bundle verification
when copied alone into a fresh folder and run with System32 on PATH. It imported
only Windows system DLLs.

## Verification boundaries

- The locked Windows Rust workspace passed 276 tests. Workspace formatting,
  strict Clippy, frontend type checks, ESLint, and build checks passed.
  The Windows-target dependency audit passed.
- Pinned protocol compatibility and generated-source checks passed. The full
  frontend suite initially passed 190 of 191 tests. Windows returned `EPERM`
  during setup of the dangling file symlink fixture. After the owner enabled
  Developer Mode, the full rerun passed all 191 tests across 16 suites. The
  dangling symlink fixture reached its rejection assertion and passed.

The test transport and headless browser fixture do not establish clean-PC
launch, desktop listener startup, authenticated WebSocket control, physical
phone pairing or native UI behavior. Those remain downstream.
