# Windows paired-device storage checkpoint

**Date** November 19, 2026

Windows 11 Pro 25H2 x64 ran the tests as a standard user with Rust 1.99.0,
MSVC and Windows SDK 10.0.26100.0. Synthetic fixture files stayed in a
checkout-local temporary directory. The store applied the protected
permissions and handle checks from [ADR 0005](../adr/0005-windows-private-storage.md).

The checkout now has Node.js 24.19.0, pnpm 11.25.0 and Buf 1.72.0 under
`target/verification-tools`. Their Windows downloads matched the
[Node.js release checksum](https://nodejs.org/en/blog/release/v24.19.0),
the [pnpm release asset](https://github.com/pnpm/pnpm/releases/tag/v11.25.0)
checksum and the repository's Buf checksum. The tools used a process-local
PATH. The frozen pnpm install passed its lockfile supply-chain checks.

## Scenarios exercised

- Enroll a claimed approval, persist only the SHA-256 digest and check the
  resulting cookie after reopening the store. Reject the wrong host, expired
  credentials, and a receipt whose approval expired or was revoked before
  delivery.
- Rename with revision checks and limit last-seen writes without extending
  credential expiry.
- Inject each enrollment commit failure. Confirm the receipt cannot be
  delivered and the staged file is removed. Windows initially surfaced a
  cleanup ordering bug. The corrected transaction closes the file handle
  before its cleanup guard tries to remove the temporary file.
- Revoke immediately at the verification boundary, recover from injected
  replacement and post-replacement failures, and confirm the durable
  revocation survives restart.
- Use the revocation notice to trigger dispatcher emergency release in a
  test harness. The mock confirms a held key and mouse button were released
  before the device snapshot changed on disk.
- Supersede a targeted revocation with revoke-all, reconcile a failed
  factory removal, and recreate an empty device store after identity
  rotation. Reject duplicate, over-capacity or installation-mismatched
  documents. Keep unknown newer schemas in place and quarantine corrupt
  records until explicit recovery.

## Results and remaining checks

- The locked, offline, all-target, all-feature workspace test command passed
  192 tests, including all 41 configuration tests and 11 new paired-device
  tests. The existing opt-in protocol fixture writer remained ignored by
  the default test command.
- Formatting, strict workspace Clippy, the all-feature MSVC workspace build,
  and no-dependency rustdoc for configuration and pairing passed.
- The desktop and mobile TypeScript scaffold builds, type checks, ESLint,
  Markdown formatting and lint for the changed docs, local links, and the
  secret scan passed. Buf schema compatibility and the byte-for-byte
  generated Rust and TypeScript check passed. Vitest passed 105 of 106
  tests. The existing dangling file symlink test stopped during fixture
  setup because the standard Windows user token returned `EPERM` when
  creating the link. The test remains active. The four secret scanner
  unit tests passed in a separate run.
- The cached Windows-target cargo-deny audit passed. The full configured
  graph passed bans, licenses and sources and failed advisories on the
  existing Wayland dependencies RUSTSEC-2026-0370, RUSTSEC-2026-0371
  and RUSTSEC-2026-0372. R-019 remains open.
- Production approval-generation checks, session teardown integration, LAN
  cookie delivery, physical browser pairing, guarded Windows input,
  installed upgrade, power-interruption recovery and packaging remain
  unverified. The desktop executable still has a placeholder entry point.
