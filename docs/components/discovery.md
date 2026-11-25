# Local service discovery

The desktop advertises the installation-derived hostname and a
`_tether._tcp.local` service only after binding the approved listener
generation. The service supplies bounded PTR, SRV, TXT, and exact current
A/AAAA data. TXT fields contain protocol and application versions and a
non-secret local instance label. They carry no pairing proof, credential,
user label, persistent interface ID, or raw installation identifier.

The listener generation and current inventory revision fence all addresses.
Discovery projects only addresses the listener actually bound. Each address
carries the current native index for its family. An index of zero or an
ambiguous native scope cannot start an OS advertisement.

The Windows candidate uses one request per explicit interface. It accepts
only the unambiguous request shape expressible by the documented API. The
inert projection handles one IPv4 and one IPv6 address at most per request,
with a shared nonzero index if both are used. It preserves the complete
literal fallback when an interface lacks an index or the request shape
cannot encode all bound addresses. No name enters the HTTP authority set
until the native adapter confirms it.
[Windows candidate API review](../reviews/2026-11-21-windows-discovery-api.md)

Conflicts use the installation's bounded deterministic suffixes. Listener
changes invalidate the previous scope, require session cleanup, withdraw the
old advertisement, and publish fresh addresses only after binding the new
generation. The native adapter must bound registrations, callbacks, pending
work and deadlines, including a stop during registration. It must verify
containment, conflict naming, and withdrawal on the target before use.

If mDNS cannot publish or the phone cannot resolve it, the desktop offers
a QR code for a current approved literal IP address and port. A different
hostname or IP address requires its own browser pairing.
