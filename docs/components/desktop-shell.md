# Desktop shell contract

**Task:** DT-001  
**Status:** Accepted implementation contract  
**Authority:** `SPEC.md`, especially sections 5, 17, 20, 27, 28, and 38

The Tauri 2 desktop shell presents local status and administration. It is not a
second application core: `agent-core` owns service lifecycle, and the config,
pairing, session, discovery, network, and input crates own their respective
state. No Tauri type may cross those component boundaries.

## Trust boundary

The packaged desktop webview is an administrative principal. The LAN-served
mobile application is not. Administrative functions are available only through
commands registered for the bundled main-window label; they are never exposed
as HTTP routes, WebSocket messages, CORS endpoints, custom URL handlers, or
remote Tauri capabilities.

The desktop frontend receives only view models and invokes narrow semantic
commands. It does not receive storage paths, credential digests, persistent
credentials, cookies, raw tokens except the short-lived pairing offer needed on
the actively displayed pairing screen, raw control frames, or typed/key input.
Pairing-offer values use redacted diagnostics and are removed from frontend
state when the screen closes, the offer expires, or a new offer replaces it.

All IPC inputs are untrusted even though the assets are packaged. Rust validates
string lengths, identifiers, enum values, revisions, current state, and
authorization before delegating to core services. Errors are closed typed codes
plus safe user guidance, never payload echoes or arbitrary native error text.

## Window and capability policy

The initial product has one administrative window with the fixed label `main`.
No arbitrary window creation, navigation to remote content, iframes, shell,
process execution, filesystem, HTTP client, clipboard, global-shortcut, or
unscoped opener permission is granted. Opening OS permission settings is a Rust
command that selects a reviewed platform constant; the frontend cannot supply a
path, URI, executable, or command line.

Capability files are explicitly named from `tauri.conf.json`; merely placing a
file in `capabilities/` must not enable it. Tauri warns that capabilities for the
same window merge, so DT-002 maintains one auditable main-window capability
rather than overlapping broad sets. See Tauri's
[capability model](https://v2.tauri.app/security/capabilities/).

The main window needs only:

- event listen/unlisten for the typed shell event channel;
- show, hide, focus, minimize, maximize, and normal window-close behavior;
- the exact custom commands listed below;
- official plugin access only where a reviewed Rust-side wrapper cannot provide
  the narrower boundary.

Production CSP permits only packaged application assets and required Tauri IPC.
There are no remote capability URL patterns. Development-only origins and CSP
exceptions never enter release configuration.

## IPC command surface

Command and DTO names below are semantic contracts; DT-002 may refine Rust and
TypeScript spelling without broadening the data or authority.

### Read-only commands

| Command | Result |
| --- | --- |
| `get_app_snapshot` | Revisioned status, listener/discovery state, selected interfaces, current safe addresses, permission/capability state, connected-device summaries, active controller, settings summary, and last safe error. |
| `list_network_interfaces` | Current conservative interface candidates with stable identifier where available, type, flags, prefixes, addresses, selection state, and exclusion reason. No SSID/location request is made solely for display. |
| `list_pending_pairings` | Bounded request IDs, source address summary, untrusted bounded client label, requested time, and expiry. Never includes the token. |
| `list_paired_devices` | Opaque device ID, user label, host pairing scope, creation/last-seen metadata, connection/control state, and revocability. Never includes a credential or digest. |
| `get_settings` | Versioned user-editable settings and effective values; no internal secrets or arbitrary paths. |
| `get_diagnostics` | The safe diagnostics fields required by section 38 and bounded aggregate metrics. |
| `get_autostart_state` | Effective OS registration plus last safe error; it does not change registration. |

Every read model contains a monotonically increasing core revision. The UI may
discard an event older than its current revision and request a fresh snapshot
after a gap. The shell does not merge security-critical partial state locally.

### Mutating commands

| Command | Required validation and effect |
| --- | --- |
| `approve_interfaces` | Requires explicit foreground confirmation and an exact current interface-snapshot revision. Persists selected stable interface identities before starting the first non-loopback listeners. |
| `set_auto_listen` | Records a separate explicit opt-in. Enabling it cannot approve a new interface and start-at-login cannot imply it. |
| `start_listening` | Starts only currently approved interfaces after peer/authority policy is ready; otherwise returns a typed precondition error. |
| `stop_listening` | Stops admission, releases active input, terminates affected sessions, withdraws discovery, and closes listeners in core order. |
| `create_pairing_offer` | Creates one bounded, expiring, single-use fragment URL/QR view model for the currently selected hostname or IP scope. Replacing it invalidates the prior token atomically. |
| `approve_pairing` / `deny_pairing` | Accepts only an opaque pending-request ID and expected revision. Timeout, duplicate, stale, or already-decided requests fail closed. |
| `rename_device` | Accepts an opaque device ID and bounded normalized display label. |
| `disconnect_device` | Releases ownership if held and closes current sessions without revoking the credential. |
| `revoke_device` / `revoke_all_devices` | Atomically invalidates credential state, releases ownership, and closes affected sessions before reporting success. |
| `request_control_transfer` | Names an already connected paired device and delegates the atomic release/epoch transition to the session actor. |
| `stop_control` | Uses the reserved emergency path to stop admission and release all held input; it cannot wait behind ordinary IPC or input queues. |
| `update_settings` | Uses a typed patch plus expected schema/revision; validates before atomic persistence and applies coordinated service changes. |
| `reset_settings` | Requires an explicit confirmation token scoped to the current window action. It preserves or removes identity/devices only according to the separate reset choice displayed to the user. |
| `set_autostart` | Runs only from a direct user setting action, reconciles actual OS registration, and records success. It never enables listening or auto-listen. |
| `open_permission_settings` | Accepts only a closed permission kind and opens the reviewed OS destination. |
| `show_main_window` | Shows, focuses, and requests attention for the existing administrative window. |
| `quit_application` | Starts the one ordered graceful shutdown; repeated requests join the same shutdown and cannot create concurrent cleanup. |

Mutations return the new revision or a typed stale-revision response. The UI
must refresh and ask again rather than replaying a stale destructive action.
Commands are bounded and cancellable where the OS permits; dropping a frontend
request does not cancel safety cleanup already accepted by the core.

## Events

Rust emits one typed event family to the `main` window:

- `snapshot_changed { revision, reason }`
- `pairing_requested { revision, request_summary }`
- `permission_changed { revision, permission_summary }`
- `attention_required { revision, safe_code }`
- `shutdown_progress { revision, phase }`

High-frequency input, WebSocket frames, logs, and raw metrics are never Tauri
events. Events are hints to fetch/refresh authoritative snapshots, not mutable
authority. Event payloads obey the same redaction and size rules as commands.

## Single-instance execution

The official Tauri single-instance plugin is registered first, before plugins
or setup that can open storage, listeners, discovery, tray state, or input. The
[single-instance documentation](https://v2.tauri.app/plugin/single-instance/)
explicitly requires first registration and notes that Linux uses session D-Bus;
each claimed Linux package must validate that mechanism in its real confinement
model.

The primary process also takes the application-data lock defined by ST-001
before it mutates identity or settings. Plugin notification alone does not
replace storage locking. Failure to establish either exclusive ownership stops
startup before LAN or input initialization.

A secondary launch passes no authority to the primary. Its arguments and
working directory are ignored and never logged; the primary only shows and
focuses the existing main window, then the secondary exits successfully. This
avoids accidentally ingesting a URL fragment, credential, path, or command.

## Startup and first-run ordering

1. Establish process single-instance ownership.
2. Acquire the owner-only application-data lock.
3. Load/migrate settings and identity or enter safe recovery.
4. Initialize the core with LAN admission disabled.
5. Initialize the native backend and current permission state.
6. Inventory interfaces and construct the tray/window snapshot.
7. On first run, show the main window, trusted-LAN warning, and proposed
   interfaces.
8. Start non-loopback listeners only after explicit approval. On later starts,
   automatic listening occurs only when separately enabled and every selected
   interface still satisfies the admission policy.
9. Start discovery only for successfully bound approved listeners.

Loopback-only diagnostics may start before interface approval if they expose no
LAN route or secret. A partial startup failure unwinds initialized services in
reverse order and leaves input neutral.

## Window close, tray, and quit

Closing the settings window hides it and keeps the background agent/tray alive.
It does not stop listening, release control, or quit; the UI and first-run
onboarding make this behavior explicit. Tauri close-request handling must call
`preventDefault` synchronously before returning and then hide the window, as
required by the [window close API](https://v2.tauri.app/reference/javascript/api/namespacewindow/#closerequestedevent).

The tray/menu-bar surface always provides:

- Show Tether;
- status text;
- Stop Control, prominently enabled whenever input could be active;
- Start/Stop Listening as appropriate;
- Quit Tether.

Tray states are `STARTING`, `NEEDS_SETUP`, `READY_NOT_LISTENING`, `LISTENING`,
`PAIRING_PENDING`, `CLIENTS_CONNECTED`, `CONTROLLING`, `SUSPENDED`, and
`ERROR_SAFE`. `CONTROLLING` has an unmistakable visual state. A stale tray
snapshot may request an operation, but the core revalidates current state.

Full quit is distinct from window close. It disables new IPC/LAN admission,
uses emergency release-all, terminates sessions, withdraws discovery, closes
listeners, flushes valid settings, releases OS handles and the data lock, then
exits. If persistence fails, cleanup and input release still proceed and the
failure is reported safely where timing permits.

## Start at login

Autostart defaults off and changes only after direct user consent. Tether uses
the official platform implementation only after native package validation. The
Tauri [autostart plugin](https://v2.tauri.app/plugin/autostart/) supports the
three intended desktop platforms and exposes separate query/enable/disable
permissions; if guest bindings are used, grant exactly those three to `main`.
Prefer a Rust wrapper so the frontend has one semantic command and cannot call
plugin operations outside the settings flow.

Autostart launches background/tray mode. It does not bypass first-run UI,
approve interfaces, enable automatic listening, accept pairing, or suppress a
permission error. Removing user consent disables the OS registration and is
verified by reading its effective state.

## Operating-system lifecycle

Platform adapters translate observable events into core lifecycle messages:

| Event | Required action |
| --- | --- |
| Suspend/sleep preparation | Stop normal input admission, emergency release, invalidate ownership epoch, and close or suspend sessions before sleep when timing permits. |
| Resume/wake | Re-inventory interfaces, addresses, permissions, and backend state; never restore held input; require a fresh epoch. |
| Session lock | Stop input admission, release all, invalidate ownership, and update tray/UI. |
| Session unlock | Recheck permission/backend/interface state and require fresh control. |
| Logout or graceful OS shutdown | Run the observable portion of ordered quit, prioritizing release over persistence/UI. |
| Network address change | Delegate coordinated release, listener/authority transition, discovery update, and new QR snapshot to `agent-core`. |
| Permission revoked | Stop admission, release what remains possible, mark capability unavailable, and show platform-specific guidance. |

Duplicate and reordered native notifications are expected. Core transitions are
idempotent and keyed by lifecycle generation. Uncatchable termination and OS
failure remain the documented D-05 limitation.

On macOS, Accessibility and Local Network privacy are independent. The shell
must distinguish them, trigger Local Network-sensitive work from an appropriate
foreground lifecycle, and package `NSLocalNetworkUsageDescription` plus Bonjour
service declarations. Release validation requires a stable bundle identifier
and signing identity; development behavior is not evidence of permission
persistence.

## Verification gates

DT-002 must test command input validation, stale revisions, redaction, event
coalescing, and least-privilege capability snapshots with core services faked.
DT-003 must prove second launch cannot initialize services and only focuses the
primary. DT-006 must inject every observable lifecycle event and assert the mock
backend ends neutral. Native package gates separately validate tray behavior,
autostart, single-instance mechanics, permissions, and quit on each claimed OS.
