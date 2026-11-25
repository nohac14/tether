# Windows development checkpoint — 2026-11-19

## Pairing and assets

`tether-pairing` implements the one-use, scoped QR and completion capabilities,
temporary desktop approval state and constant-time SHA-256 digest comparison.
Ten tests cover concurrent claims, denial, peer and host isolation, saturation,
expiration, poll frequency, clock regression, invalid input, redaction, restart,
and both injected and real OS entropy. The device store and durable cookie
publication still belong to PA-003 and PA-004.

`tether-network` implements the NW-004 embedded-asset response policy. Five
tests cover cache and security headers, MIME, strong content ETags, bounded
weak `If-None-Match`, HEAD lengths, exact paths, request body rejection and
invalid manifests. The mobile manifest generator and listener adapter remain
pending. The older interface-provider and listener-supervisor implementation
described in the plan was outside this checkout. The tracked network crate was
a stub before this change, and reachable Git history contains no prior
implementation of those components.

## Windows storage

The owner approved [ADR 0005](../adr/0005-windows-private-storage.md). Windows
settings and identity now use protected ACLs at creation, validate existing
owners and grants, hold the root and quarantine directories open, and check
handle identity, reparse attributes and single-link regular-file state. The
lock denies sharing and takes the OS lifetime lock. Commits retain bounded
re-reads and file sync, request `MoveFileExW` write-through, and revalidate
directory protection before publishing the revision.

Thirty configuration tests ran on Windows 11 Pro 25H2 x64 as a non-administrator.
They include all 20 tests that failed before this change and six new Windows
tests for inherited directories and files, an extra ACL principal, hard links,
junctions at the root and in an ancestor, and root rename protection. Every
test passed. The initial run encountered the sandbox's reparse-backed temporary
location and denied link creation. The final command used an ordinary
checkout-local temporary directory and ran outside the sandbox with approval.
All records and filesystem links were synthetic test fixtures.

## Verification and remaining work

- Pinned Rust 1.99.0, MSVC, Windows SDK 10.0.26100.0. Locked offline formatting,
  strict workspace all-target/all-feature Clippy and builds, and no-dependency
  rustdoc for `config`, `network` and `pairing` passed. The locked offline
  workspace test run passed 181 tests. The explicit fixture writer remains
  configured for separate execution.
- Checksum-verified cargo-deny 0.20.2 passed Windows-target advisories, bans,
  licenses and sources. The full configured graph passed bans, licenses and
  sources. It failed advisories on the three existing Wayland dependencies
  RUSTSEC-2026-0370/0371/0372 under R-019, with no waivers.
- Power interruption, clean installed upgrade, persistent paired devices,
  guarded native input, a running Tauri UI, LAN pairing, physical phone testing
  and packaging remain open. There is still no usable desktop executable.
