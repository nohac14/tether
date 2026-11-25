# Manual validation environment inventory

**Inventory date:** 2026-10-16
**Task:** A-004  
**Status:** Complete as an availability inventory; no native-input, package, or
real-device product validation has been performed.

This inventory records what is actually available to the current workspace and
what remains a release dependency. Availability is not a support claim and does
not count as a passed manual test.

The [November 19 Windows checkpoint](2026-11-19-windows-foundation.md) records
direct observations of Windows 11 Pro 25H2 x64, toolchain setup and the
read-only `SendInput` export check.

## Locally observed environment

| Item | Observed value | Evidence status |
| --- | --- | --- |
| Host OS | macOS 15.7.5, build 24G624 | Observed with `sw_vers` |
| Host kernel/architecture | Darwin 24.6.0, arm64 | Observed with `uname -srm`; exact hardware model was not exposed to the sandbox |
| Available hardware/resources | Apple Silicon Mac with 16 GiB RAM and roughly 60 GiB free disk; Windows 11 PC with AMD processor | Owner-reported on 2026-10-16; Windows architecture/build, resources, developer tools, access, and native tests not yet observed; no physical Android device reported |
| Xcode | Xcode 16.4, build 16F6 | Observed with `xcodebuild -version` |
| Safari | 26.4 | Observed from the installed application bundle; no Tether browser test run |
| Chrome for macOS | Not observed in `/Applications` | Absence from that path is not a machine-wide installation audit |
| Rust | Rust/rustc 1.99.0 and Cargo 1.99.0, `aarch64-apple-darwin`; `rustfmt` and Clippy installed; repository override selects `rust-toolchain.toml` | Observed and used to pass the F-001 workspace gate on 2026-10-09 |
| Node.js | Workspace runtime v24.19.0; system Homebrew runtime v23.7.0 | v24 is the maintained LTS line selected for workspace validation; the EOL v23 runtime must not be used for release checks |
| pnpm | Workspace runtime 11.25.0 | Used with pinned Node 24.19.0 for F-002 installation, frozen verification, audit, and aggregate checks |
| Apple code-signing identities | 0 valid code-signing identities reported | Signed/notarized macOS permission and package tests are unavailable |
| Android tooling | ADB 36.0.0 is installed | Device enumeration could not run inside the sandbox; an escalation request was denied, so no device presence or version was observed |
| Rust cross-target components | `x86_64-pc-windows-msvc` and `x86_64-unknown-linux-gnu` standard libraries | Installed from the official Rust 1.99.0 channel on 2026-10-16 and used only for compile/Clippy checks; this is not a Windows or Linux runtime test |
| Buf | Official temporary Buf 1.72.0 Darwin-arm64 binary | Release SHA-256 verified; format/lint/build ran. The binary was not installed or committed, and PR-002 remains blocked on its schema decision/baseline |
| npm registry path | Bundled Node 24.19.0/pnpm 11.25.0 reached the official npm registry | F-002 graph resolved (224 entries), frozen install and audit passed after approved remediation; tooling tests and skeleton builds ran, not browser/production builds |
| cargo-deny | 0.20.2, built with the published tool lockfile through Cargo | Locked advisories, bans, licenses, and sources passed after explicit internal path versions were added; tool binary and caches remain outside the repository |

The initial F-002 failure for nonexistent `@eslint/js@10.11.0` and subsequent
parser advisories were resolved by explicit owner-approved pin corrections and
scoped overrides. Full local checks now pass; exact evidence and remaining gaps
are recorded in `../reviews/2026-10-16-npm-tooling-findings.md` and
`../reviews/2026-10-16-codex-wave8-response.md`.
Use [`test-environment-setup.md`](test-environment-setup.md) to provision the
remaining environments; this inventory does not claim they have been created.

No typed input, credential, token, clipboard, device identifier, network address,
or signing-identity name was collected for this inventory.

## Required platform environments

| Intended claim or risk | Environment required | Current availability | Gate |
| --- | --- | --- | --- |
| Windows 11 Tier 1 | Clean Windows 11 x86-64 machine/VM with real interactive desktop, supported WebView2, installer privileges, normal and elevated target applications, mixed-DPI displays where possible | Windows 11 Pro 25H2 x64 build 26200.9974 observed on the candidate PC. Clean install, interactive input tests and packaging remain pending. | WIN-003, PK-001, RL-002 |
| Serviced Windows 10 | Each exact LTSC/IoT/ESU edition intended for a support claim | Unavailable; no Windows 10 claim may be made | WIN-003, IT-006 |
| macOS Apple Silicon | This arm64 host plus a stable Apple-issued signing identity and clean install/upgrade environment | Host available; signing and clean permission lifecycle unavailable | MAC-003, PK-002 |
| Intel macOS | Maintained Intel Mac capable of running the claimed macOS release | Unavailable | MAC-003, IT-006 |
| Linux X11 | Named x86-64 distribution/desktop with Xorg, XTEST, active XKB layouts, and clean package environment | Unavailable | X11-003, PK-003 |
| GNOME Wayland | Named x86-64 distribution with exact GNOME, portal, and EI versions | Unavailable | WAY-001, WAY-004 |
| KDE Wayland | Named x86-64 distribution with exact Plasma, portal, and EI versions | Unavailable | WAY-001, WAY-004 |
| Linux arm64 | Native arm64 hardware for package and input validation | Unavailable; not a release target | IT-006 |
| iPhone Safari | Current physical iPhone and maintained iOS/Safari release | Unavailable | IT-005 |
| iPad Safari | Current physical iPad and maintained iPadOS/Safari release | Unavailable | IT-005 |
| Android Chrome early network gate | Current physical Android phone with current Chrome, connected to the same controlled LAN as the test server | Unavailable per owner hardware inventory; no device/browser test ran | BV-001 |
| Android tablet Chrome | Current physical Android tablet with current Chrome | Unavailable | IT-005 |

## Required network topologies

The current host's ordinary network attachment was not treated as a controlled
test topology. The following environments must be constructed and recorded
without storing private addresses or credentials:

1. Trusted private Wi-Fi with working multicast and same-subnet IPv4.
2. Trusted Ethernet and Wi-Fi concurrently, with only one interface approved.
3. IPv6 on-link peers, including bracketed URL authorities and non-default port.
4. DHCP/address change while paired through `.local` and separately through IP.
5. mDNS blocked and hostname-conflict/rename cases with the IP QR fallback.
6. Guest/client-isolated Wi-Fi where the phone cannot reach the desktop.
7. VPN, VM, container bridge, tunnel, and public-profile adapters that must not
   be proposed or admitted by default.
8. Port collision followed by approved, persisted fallback and synchronized QR,
   mDNS, and firewall behavior.
9. Browser Local Network Access allow, deny, reset where supported, and an open
   same-origin LAN WebSocket using the page's active server port.

## Early Android validation requirement

BV-001 is deliberately placed before the network-server contract. It must use a
physical Android device and the current Chrome release, and must record actual
Local Network Access prompt behavior and same-origin LAN WebSocket behavior.
Desktop Chrome, Android emulation, documentation review, or successful TCP tests
from another client do not satisfy the gate.

Until that device is available and the gate is run, R-012 remains open and
NW-001 remains dependency-blocked. Independent foundation, protocol, input-core,
and storage work may continue in dependency order.

## Inventory maintenance

Update this file when hardware, signed identities, clean OS images, browser
versions, or controlled network environments become available. Actual test
results belong in separate dated records under `docs/manual-tests/`; do not
overwrite availability with an unlinked “passed” claim.
