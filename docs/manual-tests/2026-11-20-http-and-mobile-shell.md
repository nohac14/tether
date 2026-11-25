# Windows HTTP and mobile-shell checkpoint

## HTTP asset service

The Windows test accepted a real TCP connection through the listener
supervisor's loopback fixture and transferred both connection guards to
the new HTTP adapter. The adapter served the fixture HTML and script,
then refused an unapproved Host on a separate connection. The test used a
synthetic inventory and release acknowledgement, and opened no LAN socket.
The desktop bundle test verified the real built Vite bytes through the same
asset responder.

Wire fixtures verified HEAD, conditional requests, Host and Origin
multiplicity, alias and port matching, IPv6 normalization, oversized headers
and targets, duplicate Content-Length, body rejection, request count,
header expiry, and revoked authority. Rate fixtures verified the shared
global and canonical peer limits and bounded peer storage. A new supervisor
test checked that prior alias generation numbers cannot be reused by a
replacement supervisor in the same process.

## Mobile layout

Headless Edge 157.0.4341.60 loaded the production Vite output over a
localhost fixture with the static stylesheet CSP. At 390×844, 780×375,
and 320×568 CSS pixels, it measured distinct touchpad and scroll regions,
the neutral intersection on either strip side, an interior gutter, and
44-pixel minimum scroll and keyboard targets. It checked theme and strip
preferences after reload, and verified an in-page resize to 320×300.
The emulated browser test ran with a temporary profile. It did not exercise
a physical device, a native soft keyboard or an OS edge gesture.

## Verification

- The locked offline Windows Rust suite passed 239 tests, including 51
  network tests. Formatting, strict Clippy, workspace builds, network
  rustdoc, and the static x64 release bundle check passed.
- The frontend build, type checks, ESLint and all 20 focused bundle and
  layout tests passed. Schema compatibility and generated Rust and
  TypeScript checks passed.
- Vitest passed 125 of 126 tests. The existing dangling file symlink
  fixture still fails during Windows setup with `EPERM`. The fixture
  remains active. All four Node secret scanner tests passed.
- The Windows dependency audit passed. The all-target audit continues
  to flag the existing R-019 Wayland advisories.

The desktop composition root still needs to start the approved listener
and supply the production session coordinator. The HTTP asset adapter
serves bodyless routes. Pairing routes, control WebSockets, native input,
physical phone qualification and packaging remain downstream.
