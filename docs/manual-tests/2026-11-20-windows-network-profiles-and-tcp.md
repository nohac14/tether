# Windows network profiles and TCP checkpoint

The final read-only diagnostic on Windows 11 Pro 25H2 x64 reported 59
adapter entries, four eligible address entries, and seven IPv6 entries.
Windows reported one private-profile adapter and one public-profile adapter.
The policy proposed one adapter for approval and preserved the revision
across the immediate repeat scan. The diagnostic collected no network names
or secrets, and printed no addresses, labels or identifiers.

The Network List Manager reader associates each active network and its
category with the OS adapter GUID. A sole private association can support
a default proposal. Domain, ambiguous, absent and future categories
remain excluded from defaults. Any public association excludes the
adapter from defaults. The reader retains network IDs and raw categories
in memory to detect observed changes. The private-profile result grants
no automatic startup authority. [Microsoft network category API](https://learn.microsoft.com/en-us/windows/win32/api/netlistmgr/ne-netlistmgr-nlm_network_category)
[Microsoft network identity API](https://learn.microsoft.com/en-us/windows/win32/api/netlistmgr/nf-netlistmgr-inetwork-getnetworkid)

The provider checks two complete native observations on one worker.
It examines at most 128 Network List Manager connections per observation
and has a two-second wait budget. On timeout it retires the inventory
and never publishes the late sample. The worker retains its single slot
until the OS call returns, so retries cannot accumulate pending work.

## Windows socket evidence

- Exclusive literal binding rejected an occupied address and allowed
  rebinding after closing the listener with zero client connections.
  [Microsoft exclusive bind behavior](https://learn.microsoft.com/en-us/windows/win32/winsock/using-so-reuseaddr-and-so-exclusiveaddruse)
- The native IPv6 loopback socket reported IPv6-only configuration.
  An occupied IPv6 candidate rolled back an earlier IPv4 bind in the same
  set. Wildcards and stale binding plans failed before a socket was created.
- An IPv4 loopback connection returned the actual local and remote
  endpoints. An independent shutdown handle closed the client connection
  while the accepted stream still existed. Repeated shutdown succeeded.
- The connection quota is shared by all listener sockets in a generation.
  Tests covered 64 simultaneous transports, eight per canonical peer,
  IPv4-mapped identity, concurrent reservation, and old-generation
  permits. One loopback test verified rejection before application handoff
  when the peer was already at its limit.

## Verification

- The locked offline all-target/all-feature workspace suite passed 213
  tests, including all 26 network tests. The opt-in protocol fixture
  generator remained ignored by the usual command.
- Managed-sandbox TCP connection attempts returned Winsock error 10013.
  The final suite passed under the standard Windows user token with
  localhost-only sockets and no firewall changes.
- Formatting, strict workspace Clippy, the MSVC workspace build, locked
  metadata, warnings-denied network rustdoc, Markdown, local links and
  the secret scan passed.
- The cached Windows-target dependency audit passed. The full configured
  graph passed bans, licenses and sources. Its advisory check still flags
  the existing R-019 Wayland dependencies RUSTSEC-2026-0370, RUSTSEC-2026-0371
  and RUSTSEC-2026-0372.

Physical LAN traffic, guarded native input, phone pairing, installation and
packaging remain unverified. The listener still needs the core-owned
supervisor and ordered input cleanup before the desktop app can serve HTTP.
