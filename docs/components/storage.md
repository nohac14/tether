# Local storage contract

**Tasks:** ST-001, ST-002, ST-003
**Status:** Contract accepted; settings and identity portions implemented
**Authority:** `SPEC.md`, especially sections 18.1, 19, 27, and 28

The `config` crate owns versioned local persistence, migration, corruption
handling, file permissions, and the process data lock. It has no Tauri, network,
input, or platform-backend types. The desktop composition root resolves the
platform application-data base and injects it before any service starts.

## Storage root and files

Production uses the per-user local application-data location, never the current
directory, executable directory, a shared system directory, or cloud-synced
documents:

| Platform | Root |
| --- | --- |
| Windows | Current user's `FOLDERID_LocalAppData` followed by `Tether` |
| macOS | Current user's `Library/Application Support/Tether` |
| Linux | `$XDG_DATA_HOME/tether`, or `$HOME/.local/share/tether` only when `XDG_DATA_HOME` is unset/empty |

The packaging identifier and product directory must remain stable across
upgrades. Tests inject a `PrivateTempDir`; they never redirect production using
an unvalidated LAN/IPC value or a general-purpose environment variable.

The root contains only these owned entries:

- `.instance.lock` — held exclusively for the process lifetime;
- `settings.json` — user preferences, selected/approved interface identities,
  listening opt-ins, and non-secret service settings;
- `identity.json` — random installation identifier, preferred hostname seed,
  and identity schema metadata;
- `paired-devices.json` — opaque device IDs, SHA-256 credential digests,
  host-pairing scope, user labels, and bounded lifecycle metadata;
- `quarantine/` — owner-only files moved aside after corruption, with generic
  generated names and no content-derived filename;
- `logs/` — separately bounded sanitized logs when logging is implemented.

No plaintext credential, pairing token, cookie, typed text, key identity,
clipboard data, raw protocol frame, or URL fragment is persisted. Device
credential digests are still treated as sensitive and never displayed, logged,
or exported.

## File envelope and schemas

Each JSON file has its own closed top-level envelope:

```json
{
  "format": "tether.settings",
  "schema_version": 1,
  "revision": 1,
  "payload": {}
}
```

`format` is an exact file-kind discriminator. `schema_version` is a positive
integer for migration. `revision` starts at one and increases on each committed
mutation of that file. Unknown fields are rejected until an explicitly reviewed
forward-compatibility policy exists; silently ignoring a misspelled security
setting is unsafe. Duplicate JSON object keys, non-integer/out-of-range numbers,
invalid Unicode, excessive nesting, and trailing data are rejected.

Every read is bounded before allocation. Version 1 ceilings are:

| File | Encoded ceiling |
| --- | ---: |
| `settings.json` | 256 KiB |
| `identity.json` | 16 KiB |
| `paired-devices.json` | 1 MiB |

Nested collection counts and individual strings have smaller domain limits.
The device store is globally bounded; reaching its reviewed device count fails
the new pairing rather than growing the file indefinitely. Exact domain limits
are constants tested beside each schema.

The three payload types are separate so ordinary settings recovery cannot
silently manufacture identity or credential state. Cross-file references use
opaque installation/device IDs and are validated after all files load.

## Exclusive process ownership

The desktop shell's single-instance mechanism improves UX but does not protect
the files. Before reading or mutating any persistent file, `config` opens
`.instance.lock` and takes a non-blocking exclusive OS lock:

- Windows opens the lock with sharing denied for read/write/delete.
- Unix uses an exclusive advisory file lock held by an open descriptor.

Failure means another process or unrecovered platform error owns the data. The
new process does not inspect a PID, break a “stale” lock, start services, or
write storage; it notifies the existing instance where possible and exits.
Kernel-released locks after process death are naturally reacquired. The lock is
released last during graceful shutdown.

All store APIs require an unforgeable in-process guard tied to the live lock.
There is no independent writer per file and no background process that bypasses
this guard.

## Atomic write protocol

One serialized storage actor performs writes. A mutation follows this order:

1. Validate a typed new payload and expected current revision entirely in
   memory.
2. Serialize to a bounded byte buffer. Serialization of credential state uses
   redacted error types.
3. Create a new unpredictable temporary file in the destination directory with
   create-new semantics and owner-only permissions. Never follow a symlink or
   reparse point supplied in place of an owned entry.
4. Write all bytes, flush the language/runtime buffer, and sync/flush the file
   to stable storage where the OS supports it.
5. Re-read and parse the temporary bytes through the normal bounded validator.
6. Atomically replace the destination without creating a plaintext backup.
   Windows uses supported replace/move semantics in the same volume; Unix uses
   same-directory rename.
7. Sync the parent directory where supported so the directory entry is durable.
8. Only then publish the new revision to in-memory consumers.

A failure before replacement removes the exact temporary file when possible
and leaves the previous committed file authoritative. A failure after the OS
reports replacement but before directory sync is reported as indeterminate and
forces a reload before another mutation. The application never reports success
from only an in-memory change.

Temporary names contain only random/counter material and the file kind. Startup
removes abandoned temporary entries only after verifying they are regular files
inside the owned root and match the exact Tether temporary-name pattern. It
never recursively deletes an unresolved path.

There is no multi-file transaction. Operations that affect multiple domains
are ordered fail-closed by their owner. For example, a factory identity reset
first stops admission and sessions, releases input, removes/invalidates the
paired-device store, commits the new identity only after confirming that file
is absent, and only then may recreate an empty store and publish the new state.
A partial failure keeps services stopped until recovery reconciles the files.
The complete coordinator contract is in
[`pairing-lifecycle.md`](pairing-lifecycle.md).

## Permissions and path safety

On Unix-like systems, create the root and `quarantine`/`logs` directories with
mode `0700` and regular data/lock/temp files with mode `0600`, independent of
the caller's umask. Existing entries with group/other permission bits are
tightened before sensitive reads; failure to tighten credential storage blocks
pairing and control.

When a regular file has already been opened and its path/inode/link count
validated, permission tightening operates on that descriptor, then validates
the path/inode relationship again. It does not reopen or chmod the pathname.
Directory modes and atomic rename destinations necessarily remain path-based;
the root is canonical, owner-only, and rejects symlinks. A same-user process
with write access to the private root is outside the file-isolation boundary,
but path swaps are still rejected where the platform exposes stable file
identity and link counts.

On Windows, create under the current user's Local App Data and apply an explicit
DACL for the current user plus required local system/administrative principals.
Do not inherit a broad writable ACL from a caller-controlled directory.
Failure to establish or inspect the expected ACL blocks credential storage.

Every owned entry must be a regular file or directory of the expected kind.
Symlinks, junctions/reparse points, device files, FIFOs, sockets, hard-link
anomalies detectable by the platform, and paths escaping the canonical root
fail closed. The code never accepts an arbitrary path over IPC or LAN.

## Loading and migrations

Loading occurs under the instance lock before LAN/input services:

1. Validate root/entry kinds and permissions.
2. Read each file through its byte limit.
3. Parse the exact envelope and current or known older schema.
4. Apply one pure, deterministic migration step at a time in memory.
5. Validate the final current typed state and cross-file invariants.
6. Atomically commit a migrated file before exposing it.

Every migration has golden old-version fixtures, semantic assertions, corrupt
and boundary cases, and an idempotence test. Migration never generates a new
credential, approves an interface, enables autostart/listening, or broadens a
permission. A missing boolean whose safe default is off migrates to off.

An unknown newer schema is not “corrupt.” Tether leaves it untouched, refuses
to downgrade/write it, keeps LAN/input disabled, and asks the user to install a
compatible version. Downgrades require an explicitly implemented migration;
they are never automatic JSON field dropping.

## Missing and corrupt state

First-run missing files are created from typed safe defaults:

- listening and auto-listen off;
- no approved interfaces;
- autostart off until effective OS state is reconciled;
- empty paired devices;
- a newly generated installation identity using the production cryptographic
  RNG, not the deterministic test source.

An unreadable or invalid file is moved, without copying, to an owner-only
`quarantine` entry using a generic random name. Quarantined bytes are never
logged, parsed for display, exported, or used as a fallback. If a safe rename is
not possible, the original remains untouched and startup fails closed rather
than overwriting evidence in place.

Recovery by file kind is:

- corrupt `settings.json`: use typed safe defaults with listening off and no
  approved interfaces, then require desktop confirmation before committing;
- corrupt `identity.json`: keep LAN/input disabled and require an explicit
  identity reset that also invalidates paired-device state;
- corrupt `paired-devices.json`: treat every credential as invalid, release any
  session, create no implicit device, and require explicit reset/re-pairing.

Parse errors contain only file kind, safe category, and numeric offset where the
parser can provide it. They never include source excerpts or values.

## Reset semantics

Reset is an administrative desktop-only operation with explicit confirmation.
Before any reset that affects network, identity, or devices, core stops
admission, releases all held input, closes sessions/listeners, and withdraws
discovery.

The UI offers distinct scopes:

| Scope | Effect |
| --- | --- |
| Preferences | Restore non-security UI/service preferences; preserve identity, approved interfaces, and paired devices unless separately selected. |
| Network approvals | Clear selected/approved interfaces and auto-listen; preserve identity/devices but require manual listener approval. |
| Paired devices | Atomically replace the device store with empty state and immediately revoke/close every session. |
| Factory identity | Clear settings, approvals, devices, and identity; remove/invalidate the paired-device file and create a fresh identity only after the coordinator preconditions in `pairing-lifecycle.md` hold. |

Reset targets the exact known files and never recursively removes the
application-data root. Logs/quarantine have separate bounded retention and
explicit deletion controls. Failure partway through a security-sensitive reset
leaves services stopped and resumes through the same recovery path on restart.

## Concurrency and API behavior

The storage actor serializes reads/mutations after startup. Callers submit typed
commands with the revision they observed. A stale revision returns a conflict
without writing. Cancellation before acceptance has no effect; after acceptance
the actor completes or reports the atomic commit result so callers cannot assume
an ambiguous mutation was canceled.

Long-lived services consume immutable snapshots and revisions, never references
to mutable serializer objects. Disk I/O does not run on async network executor
threads. Shutdown stops new mutations, drains the one accepted write within a
bounded deadline, persists only validated state, and releases the lock last.

## Required tests for ST-002 and ST-003

- clean first run and restart persistence;
- exact permissions/ACL checks on each native OS;
- partial write, short write, sync, replace, directory-sync, full-disk, and
  read-only failures at every atomic step;
- abandoned temporary files, unexpected entries, symlink/reparse/path attacks,
  and concurrent lock attempts;
- every migration and unknown-newer-version refusal;
- duplicate keys, excessive size/depth/counts, invalid Unicode/numbers, corrupt
  JSON, and cross-file identity/device inconsistencies;
- safe settings recovery, fail-closed identity/device recovery, and every reset
  scope including injected partial failures;
- redaction tests proving serialization errors and logs contain none of the
  forbidden content classes;
- deterministic installation-identity/hostname derivation tests with injected
  random bytes, while production uses a cryptographic source.

CI success on one OS does not validate another platform's replacement, locking,
or permission semantics. Native package tests record those results separately.

## ST-002 and ST-003 implementation status

The `config` crate implements the settings and installation-identity portions of
this contract. The current exact settings envelope is schema version 2; a
schema-version-1 fixture migrates deterministically by defaulting
`start_at_login` to false and log level to `info`, then commits revision `N + 1`
before exposure. Paired-device documents remain owned by PA-003.

Implemented settings behavior includes:

- validated optional server port, bounded/deduplicated opaque approved-interface
  identities, auto-listen, start-at-login, and closed log-level values;
- a 256 KiB pre-allocation/read ceiling, strict envelopes and payloads,
  duplicate/unknown-field rejection, trailing-data rejection, recursion limits
  from the maintained JSON parser, positive revisions, and safe error categories
  containing no values or paths;
- the Rust 1.99 standard library's non-blocking lifetime `File::try_lock`, whose
  documented implementations use `flock` on Unix and `LockFileEx` on Windows;
- create-new same-directory temporary files, full write/flush/file sync,
  re-read through the normal decoder, `fs::rename` replacement, and Unix parent
  directory sync;
- explicit indeterminate state after a post-replacement directory-sync failure,
  with further commits blocked until an authoritative reload;
- exact abandoned-temporary cleanup, corruption quarantine by rename, explicit
  recovery confirmation, stale-revision rejection, and redacted diagnostics;
- Unix `0700` directories and `0600` files, permission tightening, symlink
  rejection, opened-file identity comparison, hard-link-count rejection, and
  descriptor-based permission changes for validated open files.

Implemented identity behavior includes:

- independent 256-bit installation ID and 128-bit hostname seed generated from
  `getrandom`'s OS-backed production source; deterministic seeded entropy is
  injected only by tests;
- strict `tether.identity` schema version 1, positive revision, lowercase exact
  hex fields, redacted diagnostics, and a 16 KiB pre-allocation/read ceiling;
- stable preferred `tether-<48-bit-lowercase-hex>.local` derivation plus bounded
  deterministic conflict candidates through `-10000`;
- the same create-new, sync, re-read, atomic-replace, parent-sync, permission,
  symlink, opened-inode, hard-link, and exact temporary-cleanup protections as
  settings;
- fail-closed quarantine on malformed, legacy/invalid, or oversized identity,
  with no silent regeneration on later restart;
- untouched refusal of unknown newer schemas;
- explicit reset/rotation with revision checks and a hard block while any
  paired-device entry remains; and
- no identity or settings document committed when initial OS entropy fails.

The settings and identity implementation fails closed on non-Unix platforms with
`PlatformPermissionEnforcementUnavailable`; it does not silently treat inherited
Windows ACLs as compliant. Explicit Windows DACL and reparse/hard-link inspection
remain release-blocking work under R-018. Therefore the automated macOS result
does not establish Windows storage support or native Linux behavior.

Twenty-eight automated tests cover first run/restart, settings and identity
persistence/conflicts, deterministic identity derivation, entropy failure,
migration, corrupt/duplicate/oversize/unknown/trailing input, unknown-newer
schema refusal, explicit identity reset and paired-device invalidation,
concurrent locking, settings/identity abandoned partial files, injected failures
before every replacement phase, post-replacement sync ambiguity, redaction,
descriptor-targeted permission changes, and Unix permission/symlink/hard-link
behavior. The fault and entropy hooks affect tests only; the production API uses
real filesystem and operating-system entropy operations.
