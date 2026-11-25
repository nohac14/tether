# Local HTTP and WebSocket server contract

**Task:** NW-001 — implementation contract

**Authority:** `SPEC.md` §§5, 15–20, 27–28, 33–34; D-01–D-05; ADR 0002;
[`protocol.md`](protocol.md), [`storage.md`](storage.md), and
[`desktop-shell.md`](desktop-shell.md)

## Scope, evidence, and dependencies

`tether-network` owns interface inventory, approved-address listeners, embedded
static delivery, bounded HTTP/WebSocket IO, and transport validation. It does
not own credentials, approval decisions, control ownership, input translation,
settings authority, or Tauri. Pairing and session actors supply narrow typed
interfaces; only the session actor submits validated input to the dispatcher.
No HTTP/WS path exposes desktop administration or calls an input backend.

The owner physically tested Safari bundled with reported iOS 27.0.1 over Wi-Fi:
HTTP, actual-page-host/port WS upgrade, application round trips, manual
Disconnect/reconnect, reconnect after app switch/Wi-Fi change, and failure after
server stop passed. No prompt appeared. See
[the evidence](../manual-tests/2026-10-25-iphone-safari-lan.md). This establishes
BV-001's personal-use feasibility, not cookies/pairing, production liveness,
`.local`, IPv6, packaged macOS privacy, native input, or another browser. Android
Chrome's evolving LNA behavior remains a separate expansion test; do not infer
permission APIs, add permissive preflights, or bypass browser restrictions.

NW-002 implements the shared inventory policy and read-only Mac provider. NW-003
implements listeners only after this contract and inventory pass. NW-005
requires PA-004; NW-006 requires PR-005, SS-003, and PA-003. This contract is not
permission to implement those components before their gates or enable a LAN
listener while developing the model. The test-only `/probe` route never ships.

## Port and startup policy

The initial proposed product port is **54321/TCP**, configurable by the desktop
user. The current [IANA registry](https://www.iana.org/assignments/service-names-port-numbers/),
reviewed on 2026-10-25 (registry updated 2026-10-22), puts it in the unassigned
Dynamic/Private range 49152–65535. This is not an IANA reservation, an assurance
against other local uses, or a firewall rule. No elevated binding/helper is
required by this choice. Native firewall/package behavior remains unvalidated.
The earlier probe independently used 54321 as a test port; that did not itself
select or validate all product-default behavior.

- A settings `server_port = None` means unconfigured, not permission to bind an
  ephemeral port. Present the proposed port with first-run interface approval.
- Persist an explicitly approved port and selected stable interface identities
  through the config actor before starting non-loopback listeners. A stale
  interface/settings revision requires a refreshed confirmation.
- Every selected address uses the same port. Bind all addresses in the accepted
  plan before publishing readiness, mDNS, or QR data. Partial failure closes
  the candidate set; do not quietly expose a subset or wildcard.
- Collision/denial is actionable and leaves listening stopped. Offer at most
  three concrete alternative ports in the private range for explicit selection;
  a preliminary availability check is not a reservation. Persist/retry only
  after approval, and publish only successfully bound authorities. Never choose
  a fresh ephemeral/fallback port silently on each restart.
- Non-zero custom ports are storage-valid. The baseline network UI proposes
  unprivileged ports only; any stored port unavailable to the unelevated process
  fails visibly, without elevation, deletion, or silent rewriting.
- Stop/rebind follows the release and authority-generation transition below.
  Port changes do not isolate cookie storage: browser cookies are host-scoped,
  not port-scoped. The active port still participates in Host/Origin checks.

The Windows personal test window offers 55001, 57181, and 59631 as alternatives
to 54321. It uses the last explicitly selected port for the next proposal.
A bind denied by Windows or an occupied port leaves the listener stopped
and prompts the owner to choose another port. The same selected port is used
for every bound address and for the QR URL.
Once the supervisor confirms the bound set, the control inbox activates
the current generation using the same monotonic clock as session polling.
The desktop publishes the QR only after this activation succeeds.

## Inventory, selection, and peer policy

Expose revisioned, bounded `InterfaceSnapshot` values: opaque ID and its
persistence quality, interface kind, flags, addresses/prefix lengths, address
lifetime/eligibility, proposed-default state, and closed exclusion reasons.
Names are bounded display labels, never identity or network authority. Do not
collect SSIDs/location, DNS resolver settings, MAC addresses, routes, or packet
contents merely for display. Platform providers return only required fields.

Default candidates must be known Wi-Fi or Ethernet, up/running, non-loopback,
non-point-to-point, non-virtual, with at least one usable non-temporary unicast
address. Unknown kind, VPN/tunnel, VM/container/bridge, link-local-only, down,
and public-profile interfaces are not proposed. Manual selection is not an
automatic trust classification. Unsupported addresses get actionable reasons.
The first implementation rejects browser URLs needing IPv6 zone identifiers;
this does not reject on-link global IPv6 or ULA prefixes.

`ApprovedSelection` is derived only from exact IDs in the current snapshot and
explicit desktop consent. `BindingPlan` contains the accepted revision,
generation, literal address/port set, actual current prefixes, and permitted
authorities. Never accept user-supplied CIDRs or DNS answers as production
evidence of an interface's on-link prefix. Never bind `0.0.0.0` or `::`.
The probe's operator-supplied prefix is not a production discovery mechanism.

Persistence uses the existing bounded approved-ID settings field. Stable
platform identities may be persisted; session-only/index-only/unknown IDs must
not authorize automatic listening after restart. Removed/reused identities and
changed inventory revisions are revalidated, not matched by display name or IP.
Auto-listen defaults off and is a separate opt-in from autostart or selection.
New/ambiguous interface or network trust requires foreground confirmation.
An ordinary DHCP change on a still-approved interface is reconciled safely;
it never authorizes an additional interface or widens its current prefix.

At TCP accept, require the local socket address to belong to the current
binding generation and the peer to be on-link under a current prefix of that
same approved interface. IPv4 and IPv6 remain family-specific; normalize any
mapped form consistently before comparing and never create duplicate peer
quota identities. Prefer v6-only IPv6 sockets alongside explicit IPv4 sockets.
Unspecified, multicast, broadcast, non-on-link, stale-generation, or invalid
addresses are rejected before HTTP parsing/session allocation. Do not trust
private-looking ranges, DNS, forwarding headers, or arbitrary global IPv6 as
substitutes. No routed-LAN, reverse proxy, NAT traversal, or wildcard exception.

## Routes and HTTP semantics

Only the following public routes exist. Asset paths are exact entries in the
embedded build manifest, not filesystem paths. Unknown routes return 404 after
peer/Host validation; wrong methods return 405. Never include a request target
or submitted value in an error. Queries and absolute-form/proxy targets are
rejected; no authentication material is allowed in a path or query.

| Method / path | Purpose and authority | Response/cache contract |
| --- | --- | --- |
| `GET` / `HEAD /` | Embedded mobile entry, no administrative data | 200, `text/html; charset=utf-8`, `no-store`; HEAD has no body |
| `GET` / `HEAD /assets/<manifest-key>` | Exact hashed embedded JS/CSS/images/fonts | Correct fixed MIME, content hash/ETag, `public, max-age=31536000, immutable`; bounded 304/HEAD support |
| `POST /pairing/request` | One fragment-to-JSON submission to pairing service; no input authority | 202 or closed safe error; always `no-store` |
| `POST /pairing/complete` | Bounded approval-status/completion exchange owned by pairing service | Pending 202; approved 200 with host-only cookie only after durable credential commit; expired/denied safe terminal response; `no-store` |
| `GET /control` with valid HTTP/1.1 WS upgrade | Authenticated binary protocol, exact subprotocol and Origin | 101 only after peer/authority/cookie/quota checks; otherwise closed HTTP error, `no-store` |

Do not implement directory listing, arbitrary SPA fallback, log/status/admin
APIs, environment dumps, source maps, developer-server endpoints, or the probe
in production. The mobile app can use in-page navigation; adding a real entry
path requires an explicit allowlist/cache test rather than catch-all serving.

`tether_network::assets::EmbeddedAssets` builds responses from static byte
slices and an exact build manifest. It validates a separate root entry and at
most 256 one-level ASCII asset keys using the Vite default eight-character
filename hash. It verifies supported media types and the 4 MiB per-entry and
16 MiB total limits. Immutable entries receive strong SHA-256 ETags, with
bounded weak comparison for `If-None-Match` on GET and HEAD. Missing paths,
queries, absolute targets, request bodies, and unsupported methods receive
empty errors without request values. The CSP uses a policy built from the
approved binding authority. The server adapter still owns raw framing, peer,
Host/Origin checks and approved-interface binding. The production Vite manifest
is now validated during the desktop build. The pinned offline build emits a
single HTML entry and hashed assets, checks the manifest against emitted
files, and generates `include_bytes!` references in the Cargo output directory.
The executable uses that compiled manifest through `EmbeddedAssets`. The
HTTP adapter accepts the supervisor's transport and retains its generation
and quota guards. It bounds each raw head before parsing, preserves header
multiplicity, rejects ambiguous lengths and body framing, and admits only
the accepted local endpoint or its current-generation advertised alias.
Each poll of the socket checks revocation. Listener generation numbers
are unique within the process even when the core replaces a supervisor.
The desktop core still needs to drive the listener and spawn this adapter
on a runtime.

The asset service applies five seconds for headers, ten seconds for a
complete request and response, ten seconds between keep-alive requests,
and at most 32 requests per connection. Shared request budgets cap each
canonical peer and the whole service. An omitted port means HTTP port 80.
For the other ports, Host and any Origin must include the active port.
[HTTP and mobile-shell checkpoint](../manual-tests/2026-11-20-http-and-mobile-shell.md)
[Mobile bundle build](../manual-tests/2026-11-20-mobile-bundle.md)
[HTTP validator semantics](https://www.rfc-editor.org/rfc/rfc9110.html#section-13.1.2)

Reject request bodies on static/upgrade routes. Pairing requires exact
`application/json`, bounded strict types, and rejects content encoding, unknown
fields, duplicate security fields, trailing data, and malformed Unicode.
The token is submitted once after `history.replaceState` removes the fragment.
PA-001 must define the short-lived opaque completion capability returned for
polling; it is not the persistent credential, is memory-only, expires with the
pending request, and never appears in a URL, log, or local storage. Completion
must be single-consumer and bound to the pending request/host scope. Its entropy,
expiry/race handling and cancellation must pass PA tests before NW-005. No
credential is exposed in JSON or JavaScript. Desktop approval remains IPC only.

The HTTP/1.1 routes require one canonical positive Content-Length of at most
4,096 bytes and an exact admitted Origin. They close after each response and
reuse the existing authority, rate, and request deadline checks. A separate
pair-request bucket permits an initial burst of three per canonical peer and
eight globally. Completion also has its per-proof two-second poll minimum.
The bounded bridge admits at most eight waiting requests and one running
storage operation. An expired HTTP future drops its response reservation, which fences
unpublished credentials during durable completion. The cookie expiration header
uses the same attributes as issuance and is available to the `/control` upgrade
path. [HTTP pairing evidence](../manual-tests/2026-11-21-windows-pairing.md)

## Host, Origin, credential, and upgrade validation

Parse and canonicalize typed authorities rather than comparing substrings:

1. Require exactly one Host value. Reject userinfo, comma lists, whitespace,
   control characters, percent escapes, malformed/non-numeric ports, and
   ambiguous IPv4 representations. IP literals use strict parsing and bracketed
   canonical IPv6. Generated mDNS names are ASCII, case-insensitive, with at
   most one optional trailing dot. Arbitrary caller-chosen DNS names are denied.
2. Permit only authorities built from currently bound selected addresses and
   the current successfully advertised installation `.local` name/explicit
   trailing-dot alias, with the active port. Never resolve a supplied Host to
   decide trust. mDNS conflict/address changes replace the authority generation.
3. Every POST and WS upgrade requires one non-null Origin with the permitted
   HTTP scheme and matching canonical authority/port, also matching that
   request's Host. Another otherwise allowed alias is not cross-origin access
   permission. Bodyless static navigation may omit Origin; if present it is
   still validated. Forwarded/X-Forwarded headers confer no authority.
4. Validate the one credential cookie at `/control` through pairing's typed
   verifier. Reject missing, duplicate, malformed, revoked, expired, or wrong
   scope credentials without creating a session or revealing which check failed.
   Never record Cookie/Set-Cookie or raw cookie parser errors.
5. Reserve the connection/device quota atomically, negotiate only
   `tether.control.v1`, disable compression, then register a session and require
   hello. No input or ownership exists before validated hello/session gating.

Cookie attributes remain D-01: omit Domain, use `HttpOnly; SameSite=Strict;
Path=/control`, an explicit PA-owned persistent Max-Age, and no Secure/prefix
claim on HTTP. Clearing uses the same name/path/scope. Hostname/IP changes may
require re-pairing; port changes and trailing-dot allowlist normalization never
promise cookie portability or merge browser cookie stores. No token in hello.
Non-browser clients can forge Origin, so it never replaces authentication.

The HTTP implementation validates the exact upgrade headers and a single
canonical device cookie before requesting a store lookup. That lookup uses
its own eight-item bounded channel and the session actor's monotonic time
origin. A per-peer bucket allows four attempts initially, then one per second.
The HTTP adapter rechecks the listener generation after the lookup. The actor
reserves at most eight connection leases and one lease per device, including
its pre-hello phase. The HTTP adapter writes 101 only after the caller returns
a reservation. It passes prefetched bytes to the binary frame reader and
drops the reservation before attempting a bounded close. When composed with
`pairing_channel_with_control`, the store's proof reaches the real actor and
the socket exchanges binary session messages. Tests exercise that composition
on Windows with a synthetic supervised transport and mock input.

The actor charges messages and expanded operations against both connection
and shared limits before committing a validated operation. A rate or queue
failure retires the lease through the reserved release path. Each socket
can enqueue one input while it waits for the actor's receipt. A 128 KiB
shared allowance accounts for the bridge's queued payloads. The writer asks
for the next output only when ready, so it does not duplicate the actor's
bounded queue.
[Windows preflight evidence](../manual-tests/2026-11-21-control-upgrade-preflight.md)
[Frame transport evidence](../manual-tests/2026-11-21-control-websocket.md)

## Concrete initial resource budgets

These are conservative initial maxima, not measured performance claims.
Lower negotiated limits are allowed. Raising a ceiling needs synchronized
contract/tests and load evidence, never a silent retry without limits.

| Resource | Initial maximum / deadline |
| --- | --- |
| Interface snapshot | 64 interfaces, 32 addresses each, 256-byte ID / 128-byte name, 32 selected IDs, 64 planned listener addresses. Windows examines at most 128 profile connections with a 2 s refresh wait budget. Provider overflow fails closed. |
| TCP/HTTP connections | 64 global, 8 per canonical peer; at most one HTTP request executing per connection |
| Request target / headers | 1,024 target bytes; 32 headers; 16 KiB aggregate, 4 KiB per value; 2 KiB cookie header |
| HTTP deadlines | Headers 5 s; body 5 s; whole request 10 s; idle keep-alive 10 s; at most 32 requests per HTTP connection |
| Pairing JSON / response | 4,096 bytes each, no compression; approval request does not hold an HTTP handler open |
| Pending approvals | 8 global, 2 per peer; pending/completion expiry no longer than 120 s; PA-001 may choose less |
| Pair request rate | Per peer burst 3, replenish 1/20 s; global burst 8, replenish 1/5 s |
| Completion polls | One per pending request per 2 s, at most 60 within its lifetime; no unbounded client/server retry |
| Static HTTP rate | Per peer 30 requests/s, burst 60; global 240/s, burst 480 |
| WS admission | 8 global, one live control connection per paired device; per-peer connect burst 4, replenish 1/s; atomic duplicate rejection/retry |
| Hello | First/only compatible hello within 5 s after upgrade; otherwise close without ownership |
| WS frame/message | 65,536 bytes each including reassembled message; text application messages and compression forbidden |
| Raw socket read budget | Per connection 256 KiB/s, burst 128 KiB; global 1 MiB/s, burst 512 KiB; includes fragmented/control-frame overhead |
| Incoming WS messages/actions | Per connection 240 messages/s burst 120, 1,024 operations/s burst 256; global 960 messages/s and 4,096 operations/s with corresponding bounded buckets |
| Parsed/validated work | One decode in progress per connection; queue at most 8 validated envelopes and 128 KiB of accounted retained bytes, whichever fills first; protobuf hard limits still apply |
| Normal dispatcher | At most 256 logical actions, or a lower existing configured capacity; emergency mailbox remains independent |
| WS outbound queue | 8 messages / 128 KiB accounted bytes per connection; coalesce replaceable status only, never an accepted release or transition |
| WS IO | Reassembled-message read 5 s; write 2 s; close handshake 1 s; no error delivery delays cleanup |
| Application liveness | Browser protobuf heartbeat 1 s; ownership expires after 4 s without valid page-runtime heartbeat; socket Ping/Pong/input do not refresh it |
| Embedded assets | 4 MiB per asset, 16 MiB total build manifest; no runtime filesystem reads or registry/CDN fetch |

Counters use checked monotonic arithmetic. Use fixed-capacity or bounded-key
quota tables, not an unbounded IP map: 128 peer entries total, expire idle
entries after 60 s, and reject new identities when all entries are occupied.
Do not evict an active peer to reset its limiter. Quotas are shared across all
listeners/aliases, not multiplied by each address. Count operations in addition
to messages; raw-byte budgets prevent tiny fragmented/control-frame floods from
bypassing application counters. Unexpected library buffering must be measured
and bounded before NW-006/NW-007 completion. PR-004 must verify decoder depth,
field-count/allocation amplification, and limits before production admission;
network queues hold validated domain work, never multiple unvalidated graphs.

Unauthenticated limits return a fixed 413/429 or close before allocation where
possible. Authenticated sustained abuse, invalid sequences, queue/byte overflow,
or a safety-affecting protocol fault stops admission and requests reserved
cleanup; do not silently discard key/button ups. Release/leaving/ownership-loss
messages bypass the normal action-rate/queue path once authenticated and
validated, but cannot create new input authority. A flood may close the session
and neutralize it instead of accepting further traffic. No unbounded retry.

## Headers, caching, and privacy

All responses use nosniff, no-referrer, frame denial and a restrictive CSP:
`default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self';
font-src 'self'; connect-src 'self' ws://<validated-current-authority>;
frame-ancestors 'none'; base-uri 'none'; form-action 'none'; object-src 'none'`.
Construct that authority from the binding plan, never reflect a raw header.
Use local hashed files, no inline executable script/eval, remote assets, broad
CORS, HSTS/forced HTTPS on baseline HTTP, or pretend-encryption headers. Add
`Cross-Origin-Resource-Policy: same-origin` to assets. Error/pairing/entry/upgrade
responses are no-store; only content-hashed immutable assets get long caching.
Any CSP accommodation needed by UI frameworks requires a scoped reviewed test,
not an automatic wildcard/unsafe-inline script exemption.

Disable default access/body/header/frame tracing. Diagnostics are typed closed
codes and bounded aggregate counts, never request targets, headers, tokens,
pending handles, names, keys, text, payloads, or raw library errors. Network
addresses are shown only in the intended local UI, not routine log streams.
The baseline HTTP delivery and cookies remain observable/modifiable by capable
LAN attackers; CSP/Origin/rate controls are not encryption or authentication of
downloaded JavaScript. No new threat model or payload cryptography is added.

## Failure, cancellation, and lifecycle

Listener state is `Stopped → AwaitingApproval → Binding → Listening`; failures
enter `StoppedWithError`. Stop/rebind uses a separate `Draining` generation:
stop admission, invalidate old-generation reservations/queued motion, request
session release, await the dispatcher's bounded outcome, close sessions and old
listeners, withdraw old discovery/authorities, then consider a fresh plan.
No new input is admitted before cleanup and new ownership epoch. Cleanup
timeout/non-neutral reports leave control faulted, not falsely neutral.

Every close/error, liveness expiry, revocation, saturation, permission/backend
failure, interface loss, lock/suspend, shutdown, and reset is routed to the same
idempotent session cleanup contract. Cancellation after accepting a stop cannot
cancel cleanup. Never call a backend from a second thread to "unstick" a blocked
native call. Uncatchable failure retains D-05's limit. A new connection/plan
never restores old held state. macOS foreground Local Network retry and stable
bundle privacy validation remain separate from the successful CLI phone probe.

## Verification gates and exclusions

NW-002 tests pure fixtures for Wi-Fi/Ethernet/VPN/VM/container/loopback,
identity reuse, unavailable flags, mixed prefixes, mapped/ordinary IPv6, stale
approvals and overflow. NW-003 tests real loopback/namespace sockets, concurrent
start/stop, partial bind/collision, peer rejection, and no pre-approval exposure.
NW-004 snapshots every MIME/cache/security header and rejects traversal and
unknown assets. NW-005 tests single-use fragment/completion races, no secrets in
responses/logs, cookie scope and uniform failures. NW-006/007 test strict
Host/Origin/port/alias parsing, negotiation, revocation, parser/flood/slow-client
bounds, emergency release under blocked outbound IO, and no held-state replay.
Test-only hooks must not bypass any of these on a live backend.

Automated networking tests use mock input and loopback/isolated namespaces;
actual non-loopback/native tests remain explicitly opted in. Pairing/session
contracts, generated protocol checks, full mobile UI tests, native app privacy,
firewall/install tests, and independent security review remain prerequisites to
their implementation/support claims. This document does not mark those gates
passed. NW-001 adds no dependencies, server functionality, or open listener.

## NW-002 implementation checkpoint

The following Mac record describes an earlier worktree. The tracked Windows
checkout restores the shared policy and adds an IP Helper inventory adapter.
The Windows adapter uses documented interface IDs and assigned prefixes, with
revision changes on different records, link/index changes or failed refreshes.
It keeps native allocations in an owned module and performs two bounded
collections per refresh to detect observed inconsistencies. The address policy
rejects ambiguous literal bindings across the entire inventory and requires an
on-link peer for the exact bound address and active generation.

IP Helper supplies link and address facts. Network List Manager associates
active network categories with adapter GUIDs. An unambiguous private profile
can support a default proposal. Any public association excludes an adapter
from the default proposals. Domain-authenticated, missing, ambiguous, and unknown profiles
receive no default proposal. The provider keeps network IDs in memory to
invalidate old approvals when the observed network changes. The core still
needs independent continuity evidence before automatic restoration. The
diagnostic reports only aggregate counts. See the
[Windows profile checkpoint](../manual-tests/2026-11-20-windows-network-profiles-and-tcp.md).

The Windows provider owns one worker and one outstanding native request.
The supervisor starts and polls refreshes without waiting for Windows.
Timeout or read failure retires the current inventory. Cancellation or
timeout discards a late OS result before fresh work can use that worker.
The supervisor remains available for stop and lifecycle commands while a
refresh is pending.

`crates/network/src/interfaces.rs` implements pure validated records, explicit
current-revision consent, stable versus session-only identities, conservative
proposal exclusions, settings projections, current-snapshot binding plans, and
local-address/generation-specific peer checks. Its `AutoListenContext` must be
established independently by the core/provider: changed or unknown context fails
closed even with saved IDs. The policy module does not enumerate or bind;
the separate `provider` module implements read-only native inventory.

Settings projections preserve autostart/log preferences, explicitly report the
number of session-only choices, and never persist them as reusable identity.
Auto-listen cannot be enabled with those choices or a non-default interface.
The config actor must still commit with its revision under the existing storage
security policy. Binding plans reject changed/reused records even if a provider
incorrectly reuses the revision, duplicate local addresses, or more than 64
bindings. Invalid plans produce no partial result. Routine diagnostics redact
all labels, IDs, and literal addresses.

Thirteen model tests include real Unix settings-store restart, prefix boundary
cases, default exclusions across interface kinds, stale revisions, provider
overflow, unsupported flags/address states, setting preservation, auto-listen
trust revalidation, and per-local-address peer separation. Fourteen additional
provider tests cover contiguous masks, malformed/duplicate metadata, caps,
revision/counter overflow, identity retirement, service ambiguity, unknown or
layered kinds, IPv6 eligibility and the Darwin ioctl ABI. No unit test performs
live enumeration, sends packets, or posts input.

### Read-only macOS provider

`MacInterfaceProvider` uses nix `getifaddrs` for assigned addresses, current
netmasks and known interface flags. SystemConfiguration's public read-only
interface/service APIs identify unlayered Wi-Fi/Ethernet and associate a BSD
name with an OS service ID. Only an unambiguous service association is persistent;
names/indexes are never persistent IDs. Missing or multiple associations produce
OS-random, provider-scoped session-only IDs, retired after disappearance or any
refresh failure. Removed/recreated OS services produce new persistent IDs.
Both identity classes remain subject to snapshot/content revalidation.

The provider requests no SSID/location, DNS, routes, hardware-address string or
user service label, and never invokes configuration setters. It bounds native
rows, interface/service arrays, strings and retained collections; no truncation
is interpreted as a complete inventory. API failures/contradictory records fail
closed with redacted typed errors. Missing/unknown kinds stay unknown, and the
provider never labels a network private merely from its address or interface.

IPv4 eligibility uses current assigned addresses and actual masks. IPv6 also
requires a successful read-only `SIOCGIFAFLAG_IN6` query: failed queries stay
unknown, not all-false safe flags. Temporary, tentative/optimistic, duplicated,
detached, deprecated, anycast, CLAT and unknown flag bits are ineligible. The
query uses an unbound owned datagram descriptor, no packets/listening, libc's
ABI layout and a size-derived request number. It sets close-on-exec; Darwin
cannot set that atomically at socket creation. Future app child-spawn integration
must prevent inheritance during that short window (e.g. Darwin's close-default
spawn policy), rather than treating the call as an atomic descriptor operation.

Local unsafe code is limited to that documented ioctl and two casts of Apple's
returned retained CF arrays to `CFArray<CFType>`; each element is then checked
by concrete runtime type. There are no handwritten ABI layouts, extern blocks,
unsafe Send/Sync implementations or crate-wide unsafe waiver.

Unchanged inventories preserve their checked revision so a pre-bind refresh
does not invalidate identical foreground approval. Observable record/index
changes or recovery after a failed refresh advance it; failed refreshes retire
previous cached metadata/session identities. A regression demonstrates the
original always-advance bug failing before correction, then verifies the full
approval/poll/binding-plan path and invalidation on index change or failure.
Config associations
and indexes are rechecked across collection because native APIs are not one
atomic transaction. NW-003 must refresh before binding and stop/revalidate on
errors or change; DS-003 supplies ongoing address-change coordination. No
inventory guarantees absence of changes after it returns. Stable interface IDs
do not prove network continuity. Auto-listen restoration remains closed when
independent evidence is unavailable. The Windows personal build samples the
private Network List Manager profile and the unique on-link IPv4 default
gateway with its resolved link-layer address. Both samples must agree. The
saved SHA-256 fingerprint also covers the eligible PC addresses and prefixes.
The supervisor checks the fingerprint before and after binding and on its
normal inventory refresh. An absent or changed fingerprint closes automatic
admission through the ordinary lifecycle and input release path. The evidence
is a local network continuity check alongside authenticated peer admission.

On this Mac, the explicit aggregate-only diagnostic observed 22 interfaces,
four persistent associations, one proposed Wi-Fi candidate, and unchanged
contents and a preserved revision across consecutive refreshes after the
revision correction. This is local read-only
validation, not a permission, signing, listener, physical IPv6, or Windows/Linux
provider claim. Those providers remain future platform gates under ADR 0001.
The [provider checkpoint](../manual-tests/2026-10-25-interface-provider.md)
records exact checks and remaining gaps.

## NW-003 implementation checkpoint

The current checkout implements the supervisor and its Windows driver.
The driver composes the nonblocking inventory provider with exclusive
literal binding, IPv6-only sockets, whole-set rollback, and the existing
64-global/eight-per-peer quota. The supervisor owns the release transition,
an independent shutdown handle for each retained transport, and an atomic
revocation guard for each generation. The HTTP owner retains both the
guard and the Windows quota permit through teardown.

The Windows path takes new observations before binding and before publishing
the complete set. It requests a new sample at 500 ms while idle and after
each admitted transport. Pending refreshes pause new acceptance. Any
observed revision change, refresh failure or clock regression revokes the
generation and requests reserved release. The core must drive polls on its
idle schedule and supply the production session/input coordinator. See the
[listener supervisor checkpoint](../manual-tests/2026-11-20-listener-supervisor.md).

`supervisor.rs` implements the core-owned serialized lifecycle. An
`ApprovedSelection` is necessary but not sufficient to bind: the local core
must first commit settings, then `start` requests independent reserved safety
work. The supervisor owns its non-cancelling cleanup ticket, closes logical
admission immediately and drives a fixed two-second acknowledgement deadline.
Repeated stop requests reuse that ticket/deadline and cancel pending starts.
An abandoned caller cannot cancel accepted cleanup; Drop requests best-effort
shutdown without waiting or calling a backend from another owner.

The coordinator's nonblocking ticket reports Neutral only after obsolete session
admission/teardown and confirmed dispatcher neutrality. It is a core integration
contract, **not** a production session implementation or an input capability
grant. The dev-only integration actually holds a mock button through the real
dispatcher and proves cleanup still completes after the supervisor is dropped.
Production session/input coordinator wiring remains gated by SS/agent-core.
The supplied generation is the last attempted transport generation, possibly
unpublished, never an input ownership epoch. It must not cause the coordinator
to skip mandatory global release merely because no matching session exists.

After Neutral, old connections and listeners close before a candidate set is bound.
Inventory is refreshed immediately before and after the entire bind set; stale
approval or any bind/metadata failure rolls back all candidates. No wildcard,
partial readiness, silent port fallback or implicit restart occurs. A new start
requires explicit valid selection. Kernel sockets may briefly exist during the
already-approved candidate phase but no HTTP/session work or readiness is
published until the complete set validates. AwaitingApproval is an outer UI
policy state; Binding is this synchronous private subphase, not an exposed
partially-ready public mode.

The core must independently drive `poll_transition` while idle, not only on
traffic, to revalidate inventory and deadlines. It must prioritize stop/lifecycle
delivery over normal HTTP work; no network-internal autonomous actor duplicates
the core coordinator. This implementation makes no deadline guarantee if the
core stops being scheduled. During Draining, existing connection generations
are immediately invalid, acceptance stops, and sockets are retained until
neutral acknowledgement or forced closure at failure/timeout. Cleanup failure,
panic, poisoned state, counter exhaustion or invalid transition permanently
latches Faulted for that supervisor instance. Later successful release cannot
clear it. Graceful teardown closes all peers even if another close fails/panics.

Acceptance polls each listener at most once round-robin and returns at most one
candidate. It verifies actual local/remote socket endpoints and fresh inventory
before allocating a shared quota slot. Limits are 64 active transports globally
and eight per peer across all listeners; the active peer set cannot exceed 64.
Reject mapped/invalid/non-on-link peers and stale local endpoints before HTTP.
Each retained connection owns an RAII quota lease and generation; old leases
cannot free a new generation's slot. The HTTP/WS layer must retain the wrapper
and check `is_current` before accepting work. TCP admission is never
authentication or input ownership. Normal authenticated-session disconnect
cleanup remains the session owner's responsibility, not an unauthenticated
TCP peer's power to release another controller.

The historical Mac adapter used the existing locked Tokio 1.53.3/socket2 0.6.5. It required
an IO-enabled runtime, literal nonblocking addresses, v6-only IPv6 sockets,
TCP_NODELAY and a backlog request of 64 per listener (the kernel may clamp it).
Kernel backlog is distinct from the shared admitted-connection bound. Unix
SO_REUSEADDR permits restart with TIME_WAIT connections; SO_REUSEPORT stays off.
Actual collision and immediate-rebind tests verify that restart works without
sharing a live literal listener. An owned duplicate shutdown handle can
terminate a retained transport on invalidation; native EOF/idempotence tests
exercise this behavior. No local unsafe block/ABI declaration was added.
Darwin child-spawn inheritance constraints still apply to dependency-created
descriptors as documented above. Windows/Linux native adapters are deferred;
shared-model cross-compilation does not establish their socket semantics.

The 22 new supervisor tests include four real Mac loopback socket tests, one
Mac pre-IO target/runtime guard test, inert lifecycle/quota/failure/concurrency
tests and real mock-dispatcher cleanup.
No non-loopback listener, multicast, live input, packaged privacy/firewall,
Windows/Linux runtime, namespace or physical DHCP/IPv6 LAN test ran. See
[the listener checkpoint](../manual-tests/2026-10-25-listener-supervisor.md).

The core's listener safety adapter uses the session actor's reserved quiescence
ticket for the supervisor's `NeutralityTicket`. The supervisor revokes transport
authority first, waits for obsolete session closure and neutral input, then
closes the old sockets before binding. The new generation is eligible for actor
recovery only after publication. Mock rebind and timeout evidence is recorded in
the [listener safety checkpoint](../manual-tests/2026-11-21-listener-session-safety.md).

### Listener API sources

- [Tokio listener API](https://docs.rs/tokio/1.53.3/tokio/net/struct.TcpListener.html): nonblocking `from_std`, IO-runtime requirement, single-owner `poll_accept`; checked in the official cached package source.
- [socket2 API](https://docs.rs/socket2/0.6.5/socket2/struct.Socket.html): family-specific options, owned handles and shutdown; native Mac tests verify the chosen behavior rather than inferring other OS semantics.

## Earlier transport API references

- [Axum WebSocket configuration](https://docs.rs/axum/0.8.9/axum/extract/ws/struct.WebSocketUpgrade.html): explicit read/write/frame/message limits and subprotocol selection.
- [Hyper HTTP/1 builder](https://docs.rs/hyper/1.12.0/hyper/server/conn/http1/struct.Builder.html): header sizes/counts, timer-backed read timeout, connection configuration.
- [RFC 6455](https://www.rfc-editor.org/rfc/rfc6455): origin/upgrade and framing/close semantics; limits and authorization above are Tether policy.
- [RFC 6265](https://www.rfc-editor.org/rfc/rfc6265): host-only cookies, path/lifetime attributes, and lack of port isolation; SameSite policy remains the approved specification.
- [Apple Local Network privacy](https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy): no universal status query or inferred packaged permission pass.
- [nix getifaddrs](https://docs.rs/nix/0.31.3/nix/ifaddrs/): owned iterator, OS flags and socket-address access; native failure remains explicit.
- [Apple network-service interface](https://developer.apple.com/documentation/systemconfiguration/scnetworkservicegetinterface(_:)): association and public service-ID API; not an assurance of unchanged network trust.
- [Apple XNU IPv6 ABI](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/netinet6/in6_var.h): read-only address-flag request and flag semantics; libc supplies the actual struct layout.
