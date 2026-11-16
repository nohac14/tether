# iPhone Safari probe preparation — 2026-10-25

**Task:** BV-001, implementation prepared; **physical gate NOT RUN in this snapshot**

This preparation snapshot precedes the owner's initial phone test. Subsequent
owner-reported physical results and remaining cases are recorded separately in
[the iPhone Safari evidence](2026-10-25-iphone-safari-lan.md); the historical
not-run rows below are not today's complete evidence state.

The owner confirmed iPhone/Safari over Wi-Fi and explicitly approved ADR 0002
on 2026-10-23. SPEC, instructions, plan, risks, and environment guidance now
apply that approval consistently. Android Chrome LNA is not marked validated.
The desktop-hosted, LAN-only transport and D-01–D-05 are unchanged.

## Implemented scope

`tests/e2e/lan-probe` is a private workspace executable. It has no input backend,
pairing, authentication, device store, session ownership, discovery, or Tauri
dependency. It cannot manipulate the workstation. It serves only embedded
local test assets and a fixed WS ping/pong exchange. It is never shipped.

See the [component contract](../components/browser-lan-probe.md) for exact
limits and the [physical procedure](iphone-safari-lan-probe.md) for selected
Wi-Fi address/prefix consent and safe reporting. No actual LAN listener or
physical phone test was started during this preparation; automated TCP/WS tests
bind only `127.0.0.1` on ephemeral ports. No Accessibility prompt or live native
input occurred.

Nine Rust tests cover consent and unsafe-address rejection, IPv4/IPv6 prefixes,
security headers, strict Host/Origin/body/path guards, actual loopback upgrade
and JavaScript-equivalent messages, reconnect, shutdown, protocol/connection
limits, malformed/binary/oversized traffic, bounded flood, HTTP saturation
recovery, oversized headers, and slow-header timeout. Six additional Vitest
tests cover runtime-origin URL construction, active ports/IPv6/HTTPS mapping,
manual connect, application-message liveness rather than socket activity,
stale callbacks, suspension/cleanup, deadlines, bounded attempts, congestion,
unexpected replies, and synchronous browser failures. Fakes are not Safari.

The initial real handshake test failed because Hyper's `keep_alive(false)`
stripped the 101 Connection upgrade header. The fix preserves upgrades while
explicitly closing every ordinary HTTP response. No test was skipped. Oversized
headers still must produce 431; the test also recognizes macOS TCP reset after
those response bytes when unread oversized input remains. That is not accepted
as success without the expected 431 response. The HTTP idle case must close,
optionally with 408, within its bound.

## Dependency and policy record

Official standard Cargo acquisition added 44 locked packages needed by the
test-only Axum/Tokio/Hyper stack; all prior package versions/checksums remain
unchanged. Required feature selection and every new package/license/build macro
are inventoried in `../dependency-inventory.md`. Existing Tokio 1.53.3 is reused;
there is no unrelated Wayland dependency change or third-party installer.

The first license check failed correctly on `matchit` 0.8.4's MIT **AND**
BSD-3-Clause terms. Both bundled license texts were reviewed. The exact-version
exception retains both notices, conditions, disclaimers, and no-endorsement
obligation; the global Rust license allowlist is unchanged. Reviewed getrandom
0.3/0.4 and existing syn 2/3 duplicate warnings stay visible. No advisory ignore,
severity filter, source override, TLS/DNS/checksum bypass, or lint waiver exists.

## Verification

Host: available macOS arm64 development workstation; pinned Rust/Cargo 1.99.0,
official isolated Node 24.19.0, pnpm 11.25.0. This is not signed-package evidence.

| Gate | Actual result |
| --- | --- |
| Probe strict Clippy | Passed |
| Probe Rust tests | Passed: nine, none ignored/failed |
| Vitest/Node tests | Passed: 19 Vitest and four Node scanner tests |
| Full workspace format/strict Clippy | Passed, including all targets/features, locked offline |
| Full workspace tests | Passed: 147, none failed or ignored; probe sockets loopback only |
| Full workspace builds and warnings-denied Rust docs | Passed, all targets/features, locked offline |
| Pinned cargo-deny bans/licenses/sources | Passed after the documented exact license review |
| Pinned cargo-deny advisories | **Failed**, unchanged three R-019 Wayland findings; no new probe advisory reported |
| Full npm audit | Passed, zero findings |
| Aggregate `cargo xtask check` | Passed on pinned Node/pnpm; frontend build targets are still skeletons |
| Markdown/local links | Passed across 69 Markdown files |
| Local secret scan | Passed: 158 text files, zero binary files skipped; history not scanned |
| `git diff --check` | Passed |
| Physical iPhone Safari, actual Wi-Fi listener, permission prompts/denial, packaged app | **Not run** |
| Live Mac input, signing persistence, other OS/browser runtime, remote CI | **Not run** |

R-012 and BV-001 remain open until accepted physical evidence. A CLI probe
cannot establish the future app bundle's macOS Local Network privacy identity.
No `.local`, QR pairing, keyboard, gestures, or input safety integration claim
follows from this probe. MAC-002 remains incomplete, R-018 remains fail-closed,
PR-002 remains blocked on its schema baseline/atomic-click contract, and R-019
still needs a separate Wayland migration review. F-005 has not executed remotely.

## Git handoff

`.git` remains read-only; no permission bypass or push occurred. Existing
personal-deployment, macOS-backend, and npm security changes were preserved.
The later interface/config changes share Cargo.lock too. Current explicit
manual staging/commit commands are maintained in
[the network checkpoint](2026-10-25-network-foundation.md), replacing this
snapshot's older path groups. Review the whole diff first; no checkpoint marks
an unfinished task done. Do not push without separate owner approval, and do
not commit binary test executables or temporary tools.
