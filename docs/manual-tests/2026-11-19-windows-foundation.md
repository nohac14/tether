# Windows foundation checkpoint — 2026-11-19

**Scope:** WB-001 build prerequisites and the WIN-001 input mapper.

## Windows host and build tools

- Windows 11 Pro 25H2 x64, build 26200.9974. PowerShell resolved the
  `SendInput` export from the system `user32.dll` via .NET `NativeLibrary`.
  The probe sent no input and establishes only entry-point availability.
- Rust 1.99.0 (b940084d7) for `x86_64-pc-windows-msvc` was installed with
  rustfmt and Clippy. Rustup selected it as the default because none was set.
  The locked Windows dependencies were fetched into `target/cargo-home`.
- Before installation, Microsoft `vswhere` listed no instances. The pinned
  MSVC `cargo check` could not find `link.exe`. A diagnostic retry with the
  Rust toolchain's bundled LLD also reported missing Windows import libraries
  such as `kernel32.lib`.
- A separate approved Rust 1.99.0 `x86_64-pc-windows-gnu` toolchain was installed
  under `target/windows-gnu-rustup`. The existing MinGW compiler reported
  GCC 12.2.0 with target `x86_64-w64-mingw32`. Repository pins and lockfiles
  remain intact.
- After owner approval, WinGet verified the installer hash and installed
  Visual Studio Build Tools 2022 17.14.45 (17.14.37906.3) with the C++ desktop
  workload, x86/x64 tools and Windows 11 SDK. `vswhere` reports the instance
  is complete and requires no reboot. The x64 developer shell selected MSVC
  toolset 14.44.35207, `cl.exe` 19.44.35230.0, `link.exe` 14.44.35230.0 and
  SDK 10.0.26100.0 (`rc.exe` 10.0.26100.9457).

## Mapper correction

`WindowsMapper` now converts 1/1024 fixed-point input to relative pixels and
Win32 wheel deltas. Each pointer and wheel axis retains its own signed
remainder. Release clears all four fractions, and sub-unit input produces no
native plan record until a whole native unit is available. Test fixtures cover
signed conversion, maximum deltas, wheel axis isolation and release reset.

## Automated checks on Windows

- `cargo fmt --all --check` passed. With the pinned Rust 1.99.0 toolchain and
  installed MSVC/SDK, the locked offline `x86_64-pc-windows-msvc` mapper
  `check`, strict all-target and all-feature Clippy, eight inert tests and
  no-dependency rustdoc with warnings denied passed. The all-target,
  all-feature workspace build passed. `dumpbin /headers` identified the
  desktop skeleton executable as PE x64 (`8664`). Its entry point is empty.
- Before installation, the corresponding mapper checks and eight tests also
  passed with the checkout-local Rust GNU toolchain.
- The locked offline all-target, all-feature workspace build passed with Rust
  GNU. `objdump -f` identified the desktop skeleton executable as PE x86-64.
  These are link checks for the skeleton.
- On both Windows toolchains, strict workspace Clippy failed on
  `unnecessary_wraps` in the existing non-Unix `validate_link_count` and
  `sync_directory` storage stubs. The Windows persistence gate stays closed
  and the warnings remain unsuppressed.
- Each locked offline all-target, all-feature workspace test run had 140
  passing tests and 20 failures, all in `tether-config`. Those tests expect a
  working settings store and encounter `PlatformPermissionEnforcementUnavailable`
  under R-018. The explicit fixture-writer test remained ignored as configured.
- The repository's local-link and secret checks and four Node secret-scanner
  tests passed. The full frontend and generated-code aggregate did not run.
  The pinned frontend environment is not provisioned on this host.

## Remaining gates

The [later Windows checkpoint](2026-11-19-pairing-and-assets.md#windows-storage)
records the settings and identity storage implementation and supersedes the
old storage test and lint results above.

The native MSVC linker and SDK are verified for development. WB-001's
remaining toolchain pins and packaging checks are pending. WIN-001 needs an
isolated, explicitly opted-in `SendInput` spike and independent review.
WIN-002 and R-018 remain gated. No
live mouse or keyboard input, UIPI behavior, mixed-DPI monitor behavior,
application/phone control, signed installer, firewall setup, or real-browser
test ran in this checkpoint.
