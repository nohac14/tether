# Mac-native Bonjour qualification boundary — 2026-10-25

## Scope and decision

The owner explicitly approved ADR 0003. SPEC, AGENTS, discovery contracts,
plan, support matrix and R-021 now reflect that accepted decision; D-01–D-05
remain unchanged. DS-002 is **IN PROGRESS**, not DONE. DS-003 remains blocked
until its required native evidence exists. This record supersedes the
approval-pending status in the earlier listener checkpoint, not its historical
test results.

The binding review rejects neither the wrapper's API coverage nor its safety
without evidence. It documents concrete unbounded queues/draining in
`async-dnssd` 0.5.1, and the local six-function system boundary's SDK ABI,
ownership, callback and cancellation requirements. No wrapper/fork/daemon or
new external dependency version was adopted.

## Implemented and verified boundary

- Native providers issue ephemeral current scope mappings, never persisted
  native indexes as identity; stale/missing/zero/special mappings fail closed.
- Advertisement plans validate exact current binding revision, metadata, same
  actual port, unique eligible addresses and bounded TXT before registration.
- One non-Send/non-Sync owner retains a parent connection, explicit interface
  A/AAAA records and one NoAutoRename shared service per selected interface.
- At most 128 pinned callback slots; no application callback channel. First
  failure is sticky across later success. Error callbacks never read undefined
  result parameters. Readiness, exact names, descriptor ownership and parent-
  before-context deallocation are tested through inert ABI entry points.
- Thirty-second registration limit, eight ready processing calls per drive,
  stale-generation rejection, stop/drop/partial rollback, conflict, removal,
  permissions, unknown errors and clock regression fail closed. This bounds
  calls, not OS-internal work or uncatchable blocking/failure.
- The explicitly guarded qualification example binds the selected interface's
  current eligible addresses, uses a private OS-random scratch identity, and
  aborts on inventory change. Its parser tests and help-only invocation run;
no actual registration is performed by ordinary tests. It serves no HTTP or input.

The initial `-ldns_sd` link failed because this SDK exposes DNS-SD through
`libSystem`; the corrected system link passes. Strict harness lints were fixed,
not waived. Native construction tests only obtain function addresses; readiness
tests use a local Unix socket pair. Ordinary automation calls no live DNS-SD
function. The separately approved native procedure below does.

## Actual verification

| Check | Result |
| --- | --- |
| Rust fmt / strict all-target/all-feature workspace Clippy | Pass, locked offline |
| All-target/all-feature workspace tests | 214 pass, zero failed/ignored; includes 15 discovery, two diagnostic-parser and 50 network tests |
| Workspace all-target/all-feature builds | Pass, locked offline |
| Warnings-denied workspace Rust docs | Pass, no dependencies, all features |
| Discovery single-owner compile-fail doctest | One pass; Send is rejected |
| Official SDK C ABI assertions | Clang C11 syntax-only with all warnings as errors passes; no executable/registration |
| Windows/x86-64 and Linux/x86-64 discovery/network strict all-target Clippy | Pass, compile only; no native backend/runtime claim |
| Qualification `--help` | Pass; no listener, identity file or Bonjour registration |
| Pinned cargo-deny bans/licenses/sources | Pass; existing getrandom/syn duplicate warnings remain visible |
| Pinned npm audit | Pass, zero known vulnerabilities |
| Refreshed pinned cargo-deny advisories | Fail, unchanged RUSTSEC-2026-0370/0371/0372 in Wayland dependencies; no ignored finding |

The pinned Node 24.19.0/pnpm 11.25.0 aggregate passes: Rust checks, frontend
types/ESLint/Prettier, Markdown/local links (85 Markdown files), 19 Vitest and
four Node scanner tests, plus existing skeleton builds. Local secret scan
passes across 193 text files, zero binary skips. Locked offline Cargo metadata
and `git diff --check` pass. The scanner does not inspect Git history.
The existing Quartz tests have WindowServer access for unposted event inspection
only. No Accessibility prompt or native input posting is performed.

## Explicitly approved native checks

The owner separately approved **en0 Wi-Fi / TCP 54322**, a trusted private LAN,
temporary DNS-SD registration and literal listeners, with no capture or changes
to firewall/privacy settings. All processes stopped and listeners are closed.
Only sanitized booleans/counts were retained; no private IPs, temporary hostname,
identity bytes, raw inventory, DNS output or packet files are recorded here.

The first live attempt failed closed with InvalidCallback and left no listener.
The SDK callback wording did not justify the assumption that the service type
would be returned without a final dot. Apple's source shows canonical dotted
conversion. Changing the fake ABI callback to that form reproduces the original
failure (exact-scope regression test exits 101); correcting the exact comparison
then passes it. Wrong types/domains/extra labels and renames still fail closed.
The [binding review](../reviews/2026-10-25-bonjour-binding-review.md) links evidence.

After correction, the diagnostic was exercised through scoped OS DNS-SD queries
and bounded subprocess observation. These checks actually passed:

- Native registration reaches Ready for two literal bound en0 addresses.
- Direct A (type 1) and AAAA (type 28) queries each return one address; their
  union exactly matches the current eligible interface inventory.
- SRV target and TCP 54322 are correct; TXT has `pv=1`, `av=0.1.0` and the
  intended public instance label. No arbitrary metadata/credentials are tested.
- Scoped browse reports the matching Add and, after graceful stop, Remove.
  This is local daemon evidence, not a packet capture or remote cache guarantee.
- Repeated runs reach Ready, respond to Ctrl-C with exit zero/Stopped, and leave
  no TCP 54322 listener according to the OS open-file inspection.

The first auxiliary address-info parser counted only one family because it
ignored Apple's CLI percent/interface display suffix on IPv6. Direct A/AAAA
already returned both. A repeat `-G v4v6` query, normalizing only that diagnostic
notation, reports two addresses (one per family), matching the plan. No network
configuration or production zone-ID URL rule changed to get a pass.

No permission prompt/denial result is inferred from automated output. User
observation of prompts was not collected; no privacy reset or settings change
occurred. This executable/daemon result does not validate the packaged app.

## Unperformed checks and blockers

The [opt-in procedure](macos-bonjour-qualification.md) requires explicit selected
interface/port approval for further registration. Initial native callbacks,
local records/browse and graceful withdrawal now have evidence; multiple
same-family RRsets, packet containment, full coexistence, real conflicts,
zero-TTL packet delivery, permission denial/restore, daemon failure, physical
network changes and iPhone `.local` remain not tested. Further capture or
permission/network changes need separate explicit approval. The CLI is not a
packaged privacy result. R-021/R-002/R-007 stay open.
Read-only permission inspection confirms `/dev/bpf0` is root-only and not readable
by the current account. No capture was attempted and no device permission was
changed. Containment capture requires separate owner approval and appropriate
administrator assistance; do not bypass that restriction or request passwords.
R-018 Windows persistence remains closed; the separate R-019 advisory gate is
not waived. No Windows/Linux runtime, remote CI, signing or end-to-end Mac app
validation is claimed. No authentication, pairing or input admission was added.

## Changed paths and Git

New code is in `crates/discovery/src/{plan,bonjour}.rs` and their test/native
modules, `crates/discovery/tests/bonjour_abi.c`, and the guarded example. Network
provider/inventory modules now expose validated scope mapping; their previous
work is preserved. Manifest/lockfile edges reuse existing packages. The binding
review, accepted ADR, qualification procedure and synchronized docs record the
scope, evidence and remaining gates.

`.git` is read-only: no staging, commit, history rewrite, permission bypass or
push occurred. Preserve older pending Mac/probe/network/tooling changes.
After reviewing this batch and completing any desired earlier checkpoint
commits manually, the focused commands are:

```sh
git diff --check
git add -- AGENTS.md SPEC.md Cargo.lock crates/discovery crates/network/src/provider.rs crates/network/src/provider docs/adr/0003-macos-native-discovery.md docs/architecture-decisions.md docs/components/discovery.md docs/dependency-inventory.md docs/implementation-plan.md docs/open-risks.md docs/operations/support-matrix.md docs/reviews/2026-10-25-bonjour-binding-review.md docs/manual-tests/macos-bonjour-qualification.md docs/manual-tests/2026-10-25-bonjour-boundary.md
git commit -m "feat: qualify scoped Mac Bonjour boundary with inert tests"
```

Some listed tracked files also contain older changes: inspect `git diff` and
commit those prerequisites first or stage shared hunks selectively. Do not use
`git add .`, commit local identities/settings/caches/artifacts, or push.
