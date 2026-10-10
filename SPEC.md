# Tether — Complete Product and Engineering Specification

**Document:** `SPEC.md`  
**Status:** Authoritative product specification  
**Revision:** 2026-10-09 architecture reconciliation  
**Working name:** Tether (provisional; not trademark-verified)  
**Target platforms:** Windows, macOS, Linux, iOS browsers, Android browsers  
**Architecture:** Self-hosted, local-LAN-only remote input  
**Implementation approach:** Production architecture from the beginning; dependency-ordered development, comprehensive testing, no disposable prototypes

---

## 1. Project Overview

Tether is a lightweight application that allows a phone or tablet to control the mouse and keyboard of a computer on the same local network.

The user installs one native application on the computer being controlled. This application contains an embedded web server that hosts Tether's mobile interface.

The user opens that interface in a mobile browser, normally by scanning a QR code displayed by the desktop application.

Once connected, the phone functions as a wireless touchpad, scrolling device, keyboard, and shortcut controller.

**The computer being controlled is also the computer hosting the web application.**

The system must operate without internet access once installed.

### Primary objectives

1. Excellent touchpad responsiveness and accurate pointer movement.
2. Dedicated vertical and horizontal scrolling regions.
3. Reliable keyboard input, including modifiers, shortcuts, and Unicode text
   within explicitly reported platform/application capabilities.
4. Native integration with Windows, macOS, and Linux.
5. Straightforward phone-to-computer pairing.
6. Reliable operation with DHCP-assigned, changing local IP addresses.
7. No central server or external hosting.
8. Low CPU, memory, and battery usage.
9. A modern, polished, mobile-first interface.
10. A maintainable architecture that remains viable for years.

### Explicit non-goals

Tether is not intended to provide:

- Screen streaming or desktop mirroring.
- Video or audio streaming.
- Remote desktop access over the internet.
- Cloud-hosted device accounts.
- Centralized authentication.
- Remote file browsing.
- Unattended access to locked computers.
- Control of operating-system security prompts that prohibit input injection.
- A native iOS or Android application as a requirement for basic functionality.

The desktop agent must support normal user-session control. No claim should be made that it can control every application or every privileged operating-system interface.

---

## 2. Fundamental Architectural Decisions

These decisions are requirements, not implementation suggestions.

### 2.1 LAN only

All runtime communication must remain on the local network.

Do not implement:

- AWS or other cloud hosting.
- A remote database.
- Centralized API infrastructure.
- STUN/TURN.
- WebRTC signaling.
- External WebSocket relays.
- User accounts.
- Internet-based device discovery.
- Mandatory telemetry.
- Runtime calls to third-party services.

Development tools may download dependencies. Release distribution and optional version checking may use the internet, but the installed application must never require it for normal operation.

### 2.2 The desktop agent is the server

The native desktop application must host:

- Static mobile UI assets.
- Local HTTP endpoints.
- A WebSocket control endpoint.
- Pairing/authentication services.
- Device management.
- The operating-system input controller.

The mobile browser must not depend on a separately hosted frontend.

### 2.3 One desktop installation

A normal installation must include everything required to run the desktop agent.

The user must not need to install:

- Node.js.
- Python.
- Rust.
- A web server.
- A database.
- A browser extension.
- An external input-automation program.

### 2.4 No permanent IP configuration

Users must not be required to configure:

- Static IP addresses.
- DHCP reservations.
- Public DNS.
- Port forwarding.
- Dynamic DNS services.

The application must handle ordinary DHCP-based home networks.

### 2.5 Production architecture

Do not implement a throwaway version using a different transport, language, or input architecture.

Components may be developed incrementally, but each component must conform to the intended production architecture.

---

## 3. High-Level System Architecture

### Mobile browser

- React
- TypeScript
- Vite
- Browser Pointer Events
- WebSocket
- Protocol Buffers
- Responsive CSS

### Desktop application

- Rust
- Tauri 2
- Tokio
- Axum
- Protocol Buffers
- Local HTTP/WebSocket server
- mDNS/DNS-SD
- Native OS input backends

### Communication

Phone → local HTTP server → static interface

Phone ↔ local WebSocket server ↔ Rust input core ↔ native OS input backend

All application assets should be embedded in the packaged desktop application.

The Tauri desktop window and the mobile web UI are separate user interfaces over a shared Rust application core.

Tauri must not become the sole home for business logic. The input, network, authentication, discovery, and session systems belong in reusable Rust crates.

---

## 4. Supported Environments

### Desktop

Primary release support is evidence-based rather than inferred from successful
compilation.

- Windows 11 x86-64 is the primary Windows target.
- Windows 10 is supported only on editions still serviced by Microsoft, such as
  applicable LTSC, IoT, or ESU configurations. Unsupported Windows 10 editions
  are best-effort and must not be described as supported release targets.
- Vendor-maintained macOS releases on Apple Silicon are primary macOS targets.
- Intel macOS is supported only while the selected OS/toolchain remains
  maintained and signed builds are validated on real Intel hardware.
- Linux support is claimed only for named, explicitly validated distribution,
  desktop, display-server, portal, and architecture combinations.
- X11 and Wayland remain intended Linux platforms. Wayland support requires a
  compositor and portal implementation that provide the required authorized
  remote-input capabilities.
- Linux arm64 becomes a release target only after native hardware, native input,
  and package validation on each claimed configuration.

Additional architectures may be added only after actual platform and packaging
validation. Cross-compilation or CI compilation alone is insufficient.

### Mobile

Prioritize and validate current maintained versions of:

- Safari on iPhone.
- Chrome on Android.
- Safari on iPad.
- Chrome on Android tablets.

Release documentation must name the browser/OS versions actually tested. Other
modern browsers may work where they implement the required APIs, but remain
unverified until tested on real devices.

### Platform limitations

Some operating-system restrictions cannot be solved inside this application.

The UI must clearly identify unavailable capabilities and provide troubleshooting guidance rather than claiming universal support.

---

## 5. Product Experience

### 5.1 First launch on the computer

On first launch:

1. Initialize a persistent installation ID.
2. Detect supported network interfaces.
3. Initialize the appropriate input backend.
4. Check relevant operating-system permissions.
5. Present the proposed LAN interfaces and trusted-private-LAN warning.
6. Require explicit desktop approval before opening the first non-loopback
   listener.
7. Start listeners only on the approved interfaces and addresses.
8. Advertise the local hostname where possible.
9. Display the connection screen.
10. Wait for the user to pair a phone.

Future automatic listening is disabled by default and may occur only after the
user explicitly enables it for approved interfaces. Loopback-only diagnostics
may start before LAN approval.

### 5.2 Connection screen

The desktop application should display:

- Computer name.
- Local server status.
- Current interface type/name and address.
- Local hostname.
- Current LAN IP address.
- Connection QR code.
- A short pairing status message.
- Connected devices.
- A prominent Stop Control button.

Example:

**Tether**

Computer: Desktop-PC  
Status: Ready  
Network: Wi-Fi (`en0`, `192.168.1.20/24`)

Connection address: `http://tether-a8f3.local:PORT`

**Connect a device**

[QR code]

**Connected devices**

None

The network label must not depend on obtaining an SSID. Show an SSID only when
it is already available without requesting an additional location or privacy
permission solely for display. Otherwise show the interface type/name and a
clear value such as “Wi-Fi network name unavailable.”

### 5.3 Phone pairing

1. The user scans the QR code.
2. The mobile browser opens the locally hosted Tether page.
3. The phone submits a short-lived pairing request.
4. The computer displays a request to approve that device.
5. The user approves the request on the computer.
6. The phone receives its persistent, host-scoped device credential.
7. The mobile control interface becomes available.

A previously paired phone should reconnect without repeating pairing when it
returns through the same hostname or IP and retains the credential. Each
hostname or IP address is a separate browser pairing scope. Switching between
the preferred `.local` hostname and an IP address, changing IP addresses, or an
mDNS conflict rename can require re-pairing. The UI must explain this limitation
instead of weakening the `HttpOnly` credential boundary.

### 5.4 Returning users

On the phone:

1. Open the previously saved local address or browser bookmark.
2. Wait for connection/authentication.
3. Automatically enter the control interface.

The application should remember mobile-specific preferences.

---

## 6. Mobile Interface Design

The mobile interface is the most important part of the product.

It must be designed for touchscreens rather than adapted from a desktop UI.

### 6.1 Primary layout

The main interface consists of:

1. Compact connection/status header.
2. Large central touchpad.
3. Dedicated vertical scrolling strip.
4. Dedicated horizontal scrolling strip.
5. Mouse-button controls.
6. Modifier and shortcut controls.
7. Keyboard toggle.
8. Compact settings/menu access.

The touchpad should occupy the majority of the usable screen.

Avoid large headers, unnecessary borders, excessive padding, or decorative controls that reduce the interaction area.

### 6.2 Touchpad geometry

Use a three-region system:

**Main region:** relative pointer movement.

**Side strip:** vertical scrolling.

**Bottom strip:** horizontal scrolling.

The bottom/side intersection is a small neutral corner and should not trigger either scrolling axis.

The side strip defaults to the right but must be configurable to the left.

The neutral corner is a deliberate physical interaction region and must be
preserved. Axis locking may supplement it but must not replace the dedicated
scroll regions or make the corner active.

The vertical strip's interactive target must not begin flush against a physical
viewport edge where iOS browser navigation or Android system-back gestures may
take precedence. Use safe-area insets plus a tested interior gutter or deadzone,
and position the visible affordance so users naturally start inside the target.
The exact inset is responsive and must be determined by real-device testing,
not assumed from a universal pixel constant. The strip must remain large enough
to satisfy touch-target requirements after the inset is applied.

### 6.3 Responsive dimensions

Suggested initial dimensions:

- Vertical strip: 44–64 CSS pixels, responsive to screen size.
- Horizontal strip: 44–60 CSS pixels in height.
- Mouse-button controls: at least 44 CSS pixels high.
- Primary touchpad: all remaining usable vertical space.

These values are starting design parameters, not mandatory fixed dimensions.

Use responsive sizing, layout and visual viewport measurements, safe-area
insets, and the edge-gesture gutter required by section 6.2.

The touchpad should remain usable in portrait and landscape orientations.

### 6.4 Visual design

Use a clean, modern interface with:

- Light and dark themes.
- Subtle visual separation of touch regions.
- High-contrast active states.
- Minimal unnecessary animation.
- Clear connected/disconnected status.
- Readable text without requiring zoom.
- Consistent touch targets.
- No advertisements.
- No branding or loading assets from external servers.

Avoid a generic mobile dashboard appearance. The interface should primarily feel like a physical input surface.

### 6.5 Device-specific customization

Support preferences for:

- Vertical scroll strip: left or right.
- Scroll sensitivity.
- Pointer sensitivity.
- Natural/traditional scroll direction.
- Touchpad acceleration.
- Tap-to-click.
- Drag behavior.
- Two-finger gestures.
- Scroll momentum.
- Button arrangement.
- Theme.
- Haptic feedback where supported.
- Left-handed layout.
- Keep Screen Awake.

Store purely mobile UI preferences locally in the browser where possible.

### 6.6 Keep Screen Awake

Provide a configurable Keep Screen Awake feature that is off unless the user
enables it or explicitly activates it for the current session.

The Screen Wake Lock API requires a secure context and therefore is normally
unavailable to Tether when loaded over baseline LAN HTTP. Use it only when the
current browser exposes it in an eligible context, handle rejection and
revocation, and re-request only after an appropriate user interaction or
visibility transition.

An HTTP-compatible muted inline video technique may be offered as a
browser-tested fallback, but it is best-effort, must begin from an explicit user
gesture, must stop when disabled or hidden, and must never be described as
guaranteed. Do not use continuous canvas/WebGL rendering as an unconditional
keep-awake mechanism. Explain battery impact and continue to rely on server-side
session timeouts if the screen locks or JavaScript is suspended.

---

## 7. Touchpad Interaction Model

The central touchpad must generate relative pointer movement.

Touch position on the phone must not correspond to an absolute position on the desktop display.

### 7.1 Gesture definitions

| Gesture | Default action |
|---|---|
| One-finger movement | Move cursor |
| One-finger tap | Left click |
| Double tap | Double click |
| Tap and hold | Prepare drag |
| Hold then move | Drag |
| Release after drag | Release mouse button |
| Two-finger tap | Right click |
| Three-finger tap | Middle click |

Also provide explicit left-click and right-click buttons outside the touchpad.

These buttons are necessary because gestures should not be the only way to activate essential mouse operations.

### 7.2 Dragging

Support both:

- Long-press then move.
- Double-tap-and-drag behavior.

These should be independently configurable.

Tap-to-click must remain responsive by default. If double-tap-to-drag requires
delaying a single-tap decision on a particular implementation, it must be
opt-in or use a tested recognizer that keeps the configured latency within an
explicit interaction budget. Settings must disclose any resulting tap delay.
Long-press drag and double-tap drag both remain supported and must be tested
together and independently.

The drag engine must distinguish between:

- Tap candidate.
- Hold candidate.
- Cursor movement.
- Drag start.
- Active drag.
- Drag release.
- Gesture cancellation.

A gesture must never accidentally leave the mouse button held down.

### 7.3 Multitouch

Use individual pointer identifiers.

A second touch must not be treated as an additional cursor movement.

When a gesture changes from one to multiple fingers, the state machine must deliberately transition.

Do not infer gestures by counting raw events without tracking pointer identities and their positions.

### 7.4 Browser behavior

Control surfaces must apply appropriate CSS such as:

- `touch-action: none`
- `user-select: none`
- `overscroll-behavior: none`

Use Pointer Events and pointer capture.

Handle:

- `pointerdown`
- `pointermove`
- `pointerup`
- `pointercancel`
- `lostpointercapture`

Avoid accidental text selection, context menus, page scrolling, and browser zoom where browser behavior permits.

The product must not rely on disabling browser or operating-system gestures that cannot actually be disabled.

### 7.5 Gesture architecture

Implement gestures as a dedicated, testable TypeScript state machine.

Do not spread gesture logic across React event handlers.

React should:

1. Collect pointer events.
2. Pass them to the gesture engine.
3. Render state changes.
4. Forward semantic actions to the network layer.

The gesture engine should be independent of React.

---

## 8. Vertical Scrolling

A dedicated vertical scrolling strip is a core requirement.

### 8.1 Placement

Default: right side of touchpad.

Alternative: left side.

Switching sides must not affect any other input behavior.

### 8.2 Interaction

Dragging a finger along the strip generates relative vertical scroll deltas.

The user may:

- Start a gesture anywhere on the strip.
- Swipe upward.
- Swipe downward.
- Lift and reposition.
- Repeat indefinitely.

The strip must **not** behave as an absolute scrollbar representing the position of an application window.

### 8.3 Scroll direction

Provide:

- Natural scrolling.
- Traditional scrolling.

Direction must be independently configurable for the vertical axis.

### 8.4 Sensitivity

Provide a smooth sensitivity control.

The input engine must support sub-unit accumulated deltas so small motions are not lost to rounding.

---

## 9. Horizontal Scrolling

A dedicated horizontal scrolling strip must appear directly below the main touchpad.

### 9.1 Interaction

Dragging left or right generates horizontal scrolling.

The user must be able to scroll in either direction without changing keyboard modifiers.

Do not emulate horizontal scrolling exclusively through Shift + vertical scrolling.

The native backend must use the best available horizontal scrolling mechanism for its platform.

### 9.2 Placement

The strip should span the usable touchpad width, excluding the neutral intersection with the vertical strip.

### 9.3 Settings

Provide independent controls for:

- Horizontal scroll direction.
- Horizontal sensitivity.
- Horizontal momentum.

### 9.4 Unsupported applications

Some desktop applications do not implement horizontal scrolling correctly.

The agent must report whether the platform backend supports horizontal scrolling, but it cannot guarantee that every target application responds to it.

---

## 10. Scroll Physics

Scrolling should feel smooth and predictable.

### 10.1 Input processing

Each scroll region produces relative movement deltas with timestamps.

The scroll processor converts this movement into normalized scroll units.

### 10.2 Momentum

Implement optional kinetic scrolling.

Momentum must:

- Estimate velocity from recent input samples.
- Decay naturally after release.
- Stop within a bounded period.
- Cancel immediately on a new gesture.
- Stop when the connection is lost.
- Avoid excessive jumps from anomalous input samples.

Default momentum: medium.

Offer Off, Low, Medium, and High settings.

### 10.3 Axis independence

Vertical-strip interactions must never accidentally generate horizontal scrolling.

Horizontal-strip interactions must never accidentally generate vertical scrolling.

### 10.4 Backend translation

Keep normalized scroll deltas independent of platform-specific wheel units.

Each native backend must translate normalized deltas into its operating system's scrolling mechanism.

Preserve fractional accumulation where the native system accepts only discrete scrolling increments.

Do not claim identical physical scrolling behavior on every operating system; tune the translation layer per platform.

---

## 11. Pointer Motion Processing

### 11.1 Relative deltas

Collect relative finger movement in CSS pixels.

Apply a configurable transformation:

- Sensitivity.
- Optional acceleration.
- Smoothing where necessary.
- Fractional accumulation.
- Platform-specific cursor scaling correction.

### 11.2 Coalescing

Pointer movement may be accumulated and transmitted once per animation frame.

Do not send every browser pointer event as an independent network message if this creates unnecessary load.

All pointer movement, scrolling, mouse-button transitions, keyboard events,
text actions, and release commands must pass through one ordered client dispatch
pipeline. Coalescing must never reorder events across a state-changing boundary.

Before sending a click, button/key down or up, drag transition, ownership
request, or other state-changing action, flush all pending motion and scroll
deltas that occurred before that action. They may be placed earlier in the same
binary WebSocket message when the protocol envelope preserves their explicit
order. A click must never arrive before the cursor motion that positioned it.

### 11.3 Network backpressure

Monitor WebSocket backpressure.

When congested:

- Coalesce motion deltas.
- Combine compatible scroll deltas.
- Keep latency low.
- Never allow unbounded input queues.

Never silently drop state-changing input events such as key releases or mouse-button releases.

If the connection becomes too congested to process control input safely, suspend control and release held input state.

The browser and server must use bounded queues, bounded batching, and explicit
rate limits. Movement and scroll may be coalesced or discarded only in ways that
preserve state-transition ordering. Emergency release and ownership-loss work
must not wait behind a saturated normal-action queue: clear discardable motion,
stop admission for that session, perform release in actor order through a
reserved control path, and close or resynchronize the session.

### 11.4 Performance

Target responsive cursor movement on healthy LAN connections.

Measure:

- Input sampling frequency.
- WebSocket latency.
- Queue latency.
- Input injection latency.
- End-to-end perceived responsiveness.

Do not hardcode a particular refresh rate.

The application should benefit from high-refresh-rate devices without requiring them.

---

## 12. Mouse Buttons

Provide explicit controls for:

- Left click.
- Right click.
- Middle click.
- Mouse button hold/release.

Optional advanced controls:

- Back.
- Forward.

Mouse buttons must use explicit down/up state transitions where appropriate.

Click controls may use an atomic click operation when no held state is needed.

Button press state must be tracked on the desktop agent.

---

## 13. Keyboard Input

Keyboard input must support both text entry and key-level controls.

These must be separate concepts throughout the protocol and native backends.

### 13.1 Text entry

Support:

- Normal text.
- Numbers.
- Punctuation.
- Spaces.
- Newlines.
- Unicode text.
- Accented characters.
- Emoji where the target OS/application supports them.
- Composition-based languages where feasible.

### 13.2 Special keys

Provide access to:

- Escape.
- Tab.
- Enter.
- Backspace.
- Delete.
- Arrow keys.
- Home.
- End.
- Page Up.
- Page Down.
- Insert.
- F1–F12.
- F13–F24 where supported.
- Print Screen.
- Other appropriate navigation keys.

### 13.3 Modifiers

Provide:

- Control.
- Shift.
- Alt/Option.
- Meta/Windows/Command.

Modifiers should support:

**Momentary mode:** active for a single following action.

**Locked mode:** remains active until explicitly released.

The UI must clearly distinguish these states.

All modifiers must release when control ends or connectivity is lost.

### 13.4 Shortcuts

Provide a configurable shortcut panel.

Suggested defaults:

- Copy.
- Cut.
- Paste.
- Undo.
- Redo.
- Select All.
- Save.
- Find.
- Close tab.
- Switch application.
- Open task manager or platform equivalent where permitted.

Shortcuts must map to the conventions of the controlled computer, not automatically those of the phone.

For example, Copy normally maps to Command+C on macOS and Control+C on Windows/Linux.

### 13.5 Media controls

Support where possible:

- Play/pause.
- Next track.
- Previous track.
- Volume up.
- Volume down.
- Mute.

Show unsupported capabilities as disabled rather than claiming they work.

---

## 14. Mobile Software Keyboard Handling

Software keyboards do not behave exactly like hardware keyboards.

Implement a proper mobile text-entry subsystem.

The keyboard control must focus a real editable element synchronously within
the call stack of the user's tap/click gesture. Do not wait for a React render,
network response, timer, or other asynchronous step before calling `focus()`.
Use a persistent, visually unobtrusive proxy input or textarea with appropriate
mobile attributes, while preserving accessibility and preventing its contents
from becoming an authoritative mirror of desktop state. For raw live-key input,
request `autocapitalize="none"`, `autocomplete="off"`, `autocorrect="off"`, and
`spellcheck="false"` where supported; Compose and Send may deliberately enable
editing assistance because it transmits only after confirmation.

Observe `window.visualViewport` resize and scroll changes where supported. When
the software keyboard changes the visual viewport, recompute interaction
geometry and keep the keyboard controls visible without stale pointer offsets,
unintended page scrolling, or loss of the neutral/scroll regions. The viewport
meta configuration and fallback behavior must be tested on real Safari and
Chrome devices rather than relying on one `interactive-widget` behavior.

### 14.1 Browser input events

Handle:

- `beforeinput`
- `input`
- `compositionstart`
- `compositionupdate`
- `compositionend`
- relevant `keydown` events

Do not rely on `keydown` alone for text.

`beforeinput` is not guaranteed to be emitted or cancelable for every IME,
autocorrect, autocomplete, password-manager, or browser editing operation. Live
typing must therefore use a dedicated local edit buffer, send only confirmed
representable changes, reconcile `input` when possible, and move the user to
Compose and Send when an edit cannot be mapped safely.

### 14.2 Text editing strategies

Provide two modes:

**Live typing:** Sends confirmed text/edit actions as they occur.

**Compose and Send:** Lets the user prepare text locally, then explicitly sends it to the PC.

Compose and Send is particularly important for:

- Autocorrect.
- Predictive keyboard replacements.
- Complex Unicode.
- Long pasted text.
- IME composition.
- Dictation results.

### 14.3 Correctness

Avoid:

- Duplicate characters.
- Sending partial IME compositions.
- Accidental repeated Backspace commands.
- Incorrect Unicode handling.
- Incorrect text replacement after autocorrect.
- Broken deletion around surrogate pairs or multi-codepoint characters.

Never assume the mobile text field contains an accurate reflection of the focused desktop text field.

The agent does not have general access to the target application's text or selection state.

When an editing operation cannot be mapped reliably, offer the Compose and Send workflow rather than inventing a synchronization mechanism.

### 14.4 Physical key representation

Use stable canonical key identifiers, preferably USB HID usage identifiers for physical keys.

Represent logical application commands separately.

Each OS backend maps canonical keys/actions to its own native input representation.

Do not use JavaScript `KeyboardEvent.key` as the sole cross-platform wire representation.

---

## 15. Communication Protocol

Use a binary, schema-defined protocol.

Recommended technologies:

- Protocol Buffers.
- Rust `prost`.
- TypeScript `@bufbuild/protobuf`.
- Buf or an equivalent reproducible code-generation pipeline.

Maintain one authoritative schema.

### 15.1 Protocol requirements

The protocol must support:

- Client hello.
- Server hello.
- Version negotiation.
- Capability negotiation.
- Authentication/session establishment.
- Pointer movement.
- Mouse-button actions.
- Scrolling.
- Key down.
- Key up.
- Text insertion.
- Text editing commands where representable.
- Media commands.
- Release all.
- Heartbeat.
- Error messages.
- Session/control ownership.
- Connection status.

### 15.2 Message structure

Each binary WebSocket message contains exactly one versioned Protocol Buffers
envelope. An envelope identifies its type and may contain an explicitly ordered
batch of compatible actions so pending movement can precede a state-changing
action without an intervening transport race.

Where appropriate include:

- Protocol version.
- Message sequence number.
- Client timestamp or monotonic timing data.
- Action payload.

Do not confuse sequence numbers with transport acknowledgments. WebSocket already provides ordered transport delivery within a connection.

Sequence numbers are monotonic within a connection/session epoch. They detect
stale or buggy client work and are never reused to restore held state after a
reconnect. Client timestamps are monotonic values meaningful only relative to
that client; do not compare them directly with the desktop clock or use them as
authorization evidence.

### 15.3 Transport

Use a single WebSocket control connection per active mobile client.

Normal connection:

`ws://<current-host>/control`

Construct the WebSocket URL from the page's actual runtime origin. Derive the
authority from `window.location.host`, not `window.location.hostname`, so the
active server port and bracketed IPv6 form are preserved. Select `ws:` for an
`http:` page and `wss:` for an `https:` page, and set only the `/control` path.
Do not hardcode an IP address, `.local` hostname, default port, or separately
configured authority. The resulting WebSocket must remain same-origin with the
page apart from the required HTTP-to-WebSocket scheme mapping.

Negotiate an explicit WebSocket subprotocol such as `tether.control.v1` during
upgrade. Then use `ClientHello` and `ServerHello` to negotiate application
protocol versions and capabilities. The host-scoped `HttpOnly` cookie is the
browser credential and is validated during the upgrade; do not expose a
persistent credential to JavaScript merely to repeat it inside `ClientHello`.

Disable WebSocket per-message compression by default. The control payloads are
small, and compression state and decompression risk are not justified without
measured evidence and a reviewed protocol change.

### 15.4 Protocol evolution

Use explicitly versioned messages and documented compatibility rules.

Generated client and server definitions must be validated together.

Incompatible protocol versions must produce a clear error, not silent input corruption.

Compatibility rules must distinguish major incompatibility from optional
capabilities and safely reject unsupported open-enum values and commands.
Removed fields and field numbers must be reserved.

### 15.5 Size limits

Apply strict maximum sizes for:

- Network frames.
- Text insertion messages.
- Pairing requests.
- Client metadata.
- Device names.

Malformed messages must never reach the native input backend.

The network component contract must define concrete limits before
implementation. Initial ceilings must be no larger than 64 KiB per WebSocket
message and 4 KiB per unauthenticated pairing/request body unless real protocol
fixtures demonstrate a smaller value is insufficient. Text is chunked at valid
Unicode boundaries to both protocol and backend-specific limits. Also bound
HTTP headers, request deadlines, connections, pending approvals, messages per
interval, and aggregate queued bytes. Limit violations close or reject the
session without delaying input cleanup.

---

## 16. Input State and Session Safety

Input state management is a safety-critical component.

### 16.1 Server-side state

Track for each active control session:

- Pressed keyboard keys.
- Pressed modifier keys.
- Pressed mouse buttons.
- Drag state.
- Last heartbeat.
- Authentication state.
- Control ownership.
- Last processed sequence number where relevant.

### 16.2 Release-all behavior

Implement a centralized `release_all()` operation.

Call it when:

- The WebSocket disconnects.
- Heartbeats expire.
- The user disconnects.
- Another device takes control.
- The PC application exits.
- The mobile page is suspended or reports leaving.
- Authentication expires or is revoked.
- A fatal protocol/backend error occurs.
- Input ownership changes.

Releasing input must be idempotent.

The guarantee applies to every observable connection-loss and graceful
shutdown path, including WebSocket close/error, heartbeat timeout, queue
saturation, protocol failure, credential revocation, ownership transfer,
listener rebind, suspend/session-lock notification, backend failure, orderly
quit, and panic unwind where the runtime can still execute cleanup.

Use best-effort RAII/drop and panic cleanup and attempt backend neutralization
during a safe startup/reconnect transition. No application can execute cleanup
after uncatchable termination such as `SIGKILL`, `TerminateProcess`, kernel or
OS failure, sudden power loss, or loss of the native injection facility. These
are documented platform limits, not silently claimed as covered paths.

### 16.3 Heartbeats

Use application-layer Protocol Buffers heartbeats generated and processed by
the mobile page's JavaScript runtime. WebSocket/TCP Ping/Pong or socket activity
must not count as proof that the page's event loop is alive, because a browser
network stack may remain active while page JavaScript is suspended.

Use a short heartbeat interval and bounded timeout.

A reasonable initial policy:

- Heartbeat roughly every 1 second while active.
- Revoke control and release held inputs after approximately 4 seconds without successful session liveness.

These are tunable engineering parameters.

The server records receipt of valid application heartbeats and its timeout is
authoritative because mobile browsers can suspend JavaScript without notice.
Heartbeats are liveness signals, not acknowledgments for input delivery.

### 16.4 Exclusive control

By default, only one paired mobile device should actively inject input at a time.

Other paired devices may connect in standby mode.

If another device requests control:

- Make the transition explicit.
- Release all state from the previous controller.
- Transfer ownership atomically.
- Update the desktop status indicator.

Do not permit simultaneous uncontrolled modifier/button state from multiple mobile sessions.

Ownership transfer is one serialized state transition: stop accepting old-owner
actions, discard its queued motion, release its held state, advance the session
epoch, assign the new owner, and only then admit new-owner actions. A failure at
any step leaves no controller and neutral input state rather than two owners.

---

## 17. Local Network Architecture

### 17.1 Local HTTP server

The Rust desktop agent serves the compiled React bundle.

Do not run a separate production Vite server.

The static assets must be embedded or packaged locally with the desktop agent.

### 17.2 WebSocket server

Run WebSocket and HTTP endpoints through the same server origin.

Use Tokio and Axum unless a concrete technical limitation requires a different transport library.

Choose and document one configurable product default port after checking
current service assignments and platform packaging constraints. If that port is
unavailable, do not fail silently or choose a new ephemeral port on every
launch. Present a bounded alternative, persist the approved port, update mDNS
and QR data, and keep firewall configuration synchronized. Port selection does
not weaken Host/Origin or peer-admission checks.

### 17.3 Listening interfaces

Enumerate local interfaces.

Allow the user to select which interfaces expose the server.

Before the first non-loopback listener starts, require the desktop user to
approve the selected interface(s) and trusted-private-LAN warning. Future
automatic listening is opt-in and applies only to persisted approved interface
identities or an explicit, safely handled replacement flow.

Bind only addresses assigned to approved interfaces. By default, admit a peer
only when its source address is on-link under a prefix currently assigned to an
approved interface. Support IPv4 and IPv6 prefix checks; a globally scoped IPv6
address is not automatically remote or automatically trusted. Routed-LAN access,
if later added, requires a separately approved design and explicit warning.

Distinguish where possible:

- Wi-Fi.
- Ethernet.
- Loopback.
- VPN.
- Virtual machine interfaces.
- Container bridges.
- Public-facing interfaces.

Exclude loopback, VPN, tunnel, virtual-machine, container/bridge, temporary, and
link-local-only interfaces from proposed defaults where they can be identified.
They may not be auto-enabled based solely on a private-looking address.

Interface classification is not a perfect security boundary. The server must still enforce authentication and authorization.

On interface/address loss, stop admission, release active input state, close
affected sessions, update listeners and allowed Host/Origin authorities, then
re-advertise. Never leave a wildcard listener serving an interface the user did
not approve.

### 17.4 Firewall handling

Provide instructions or narrowly scoped automated configuration where supported.

Do not create unnecessarily broad firewall rules.

Prefer private-network access rules.

On Windows, prefer an installer-managed rule scoped to the signed executable,
private network profiles, and required program behavior rather than a broad
all-program/open-port rule. The normal running application remains unelevated.

---

## 18. Dynamic IP and Discovery

Dynamic IP addresses must work without manual user administration.

### 18.1 Stable installation identity

Generate a persistent random installation identifier.

Derive a preferred local hostname such as:

`tether-a8f3.local`

The hostname must not depend on the PC's IP address.

### 18.2 mDNS/DNS-SD

Use mDNS to publish the hostname.

Use DNS-SD for service advertisement.

Suggested service type:

`_tether._tcp.local`

Suggested metadata:

- Protocol version.
- Application version.
- Service port.
- Instance identifier.

Do not advertise secrets.

### 18.3 DHCP changes

When the PC's IP changes:

1. Detect interface/address changes.
2. Update server binding if necessary.
3. Re-advertise the mDNS identity.
4. Preserve the stable hostname where no naming conflict exists.
5. Close stale sessions safely.
6. Permit clients to reconnect against the newly resolved address.

### 18.4 Reconnection

The mobile client must automatically attempt reconnection.

Use bounded exponential backoff with jitter.

Display useful states:

- Connecting.
- Connected.
- Reconnecting.
- Authentication required.
- Computer unavailable.
- Network unavailable.

Do not duplicate pressed input after reconnecting.

Never automatically restore held keys or mouse buttons from the previous session.

Seamless authentication is expected only while reconnecting through the same
browser host scope. If fallback changes the hostname/IP, request pairing for the
new scope.

### 18.5 Hostname limitations

mDNS may fail on:

- Networks blocking multicast.
- Isolated Wi-Fi clients.
- Some VLAN arrangements.
- Certain mobile/router configurations.
- Networks with hostname conflicts.

Implement a fallback QR code using the current LAN IP address.

Modern Android systems support resolver-level `.local` mDNS, but reliability
still varies with OS version, browser integration, router multicast behavior,
VPN use, and network isolation. Keep the stable `.local` hostname as the
preferred address whenever it resolves; display and offer an IP-based QR/address
fallback rather than claiming universal Android failure or success.

If the user uses an IP-based fallback and that IP later changes, the phone may need to reopen the updated address. The interface must explain this rather than falsely claiming the old IP URL remains valid.

### 18.6 Browser security considerations

Because the LAN web application normally uses HTTP rather than HTTPS:

- Do not assume it is a secure browser context.
- Do not require service workers.
- Do not require Web Crypto.
- Do not require the asynchronous Clipboard API.
- Do not require push notifications.
- Do not depend on installable-PWA behavior.

A browser bookmark or home-screen shortcut is acceptable as a convenience, but must not be essential.

Screen Wake Lock is also secure-context-only. Its optional behavior and
best-effort fallback are defined in section 6.6.

---

## 19. Pairing and Authentication

LAN-only operation does not eliminate the risk of unauthorized local clients.

### 19.1 Pairing token

The PC generates a short-lived, single-use cryptographically random token.

Use 256 bits of entropy.

Place the temporary token in the URL fragment, for example
`http://tether-a8f3.local:PORT/#pair=<token>`, never in the query string or path.
Fragments are not sent in the initial HTTP request. The loaded page must extract
the bounded fragment, immediately remove it from the visible/history URL with
`history.replaceState`, and submit it once in a bounded POST body. An IP-fallback
QR uses the same fragment design and creates a separate pairing scope.

### 19.2 Desktop approval

A new device must be explicitly approved on the desktop.

An unauthenticated device cannot control input.

### 19.3 Persistent device credential

After pairing, generate a separate long-lived random credential for the phone.

Deliver it using a host-only, `HttpOnly`, `SameSite=Strict` cookie, with the appropriate local cookie attributes for the selected transport.

Omit `Domain`; set `Path=/control` and an explicit persistent lifetime. Do not
set `Secure` or use a `__Host-`/`__Secure-` prefix on baseline HTTP because
browsers require those protections to be set from a secure origin. If a future
authenticated HTTPS transport is approved, its cookie policy must be reviewed
separately.

Because the baseline transport is HTTP, do not pretend this credential is protected from an attacker capable of observing or modifying LAN traffic.

Store only a cryptographic hash of the credential on the PC.

Use a separately generated 256-bit credential, store its SHA-256 digest in the
versioned owner-protected device store, and compare digests in constant time.
The credential is scoped to the exact browser host that issued it; changing
hostname/IP can require re-pairing. Browser cookie retention is best-effort, and
credential loss or browser clearing uses the normal re-pairing flow.

### 19.4 Device management

The desktop application must support:

- Listing paired devices.
- Renaming devices.
- Revoking devices.
- Disconnecting active sessions.
- Revoking all paired devices.
- Regenerating pairing information.

Revocation must take effect immediately.

### 19.5 QR-token hygiene

Pairing tokens must:

- Expire quickly.
- Be single-use.
- Never be logged.
- Never be included in analytics.
- Be removed from the browser address bar/history entry as soon as practical.
- Not leak through referrers.
- Never appear in query strings, request-target logs, raw traces, crash reports,
  or diagnostic exports.

Apply `Referrer-Policy: no-referrer`.

Bound pending approvals globally and per peer. Expiry, denial, successful use,
or regeneration must invalidate the token atomically.

### 19.6 Origin validation

Validate the WebSocket `Origin` against the actual permitted local origin.

Also validate:

- Host.
- Session credential.
- Protocol version.
- Request path.
- Permitted network interface.
- Control ownership.

Reject cross-origin browser requests by default.

Do not interpret CORS as authentication.

Canonicalize and allowlist both `Host` and `Origin`, including scheme, port,
bracketed IPv6 literals, hostname case, and the optional trailing dot form of an
mDNS name. A browser WebSocket request without an allowed `Origin` is rejected.
Do not enable permissive CORS. Origin checking defends against browser cross-site
use but is not authentication because non-browser clients can forge it; the
host-scoped cookie, peer-admission policy, and session authorization remain
mandatory.

---

## 20. Security Model

### 20.1 Threat model

Tether assumes a trusted private LAN.

The product must warn users against running it on:

- Public Wi-Fi.
- Hotel networks.
- Campus guest networks.
- Untrusted shared networks.

### 20.2 HTTP limitation

Plain HTTP and `ws://` do not encrypt traffic.

They do not protect input content or authentication cookies from sufficiently capable attackers on the local network.

This is a conscious usability tradeoff, not a claim of cryptographic security.

A future optional locally encrypted transport may be researched, but it must not introduce a central service or make normal installation impractical.

Do not add ad hoc WebSocket payload encryption as a substitute for authenticated
web delivery. An active LAN attacker who can replace the JavaScript served over
HTTP can also steal pairing material or plaintext before payload sealing. Any
future confidentiality design must authenticate both transport and delivered
web application end to end and requires an architectural decision. The baseline
retains the explicit trusted-private-LAN threat model.

### 20.3 Data privacy

Never log:

- Typed text.
- Passwords.
- Clipboard contents.
- Full key streams.
- Persistent credentials.
- Pairing tokens.
- Raw HTTP/WebSocket bodies or control frames.
- `Cookie` or `Set-Cookie` headers.
- Query strings or URL fragments that may contain sensitive material.

No telemetry should leave the device automatically.

Protocol/domain types containing text, key identity, credentials, or tokens must
implement redacted diagnostics rather than deriving a payload-revealing
`Debug`. Sensitive headers must be marked before HTTP tracing. These values stay
redacted at every log level and in crash/diagnostic exports.

### 20.4 Host controls

The desktop user must be able to immediately:

- Stop all remote control.
- Disconnect clients.
- Disable listening.
- Revoke credentials.
- Quit the agent.

Display a clear active-control indicator in the desktop application/tray.

### 20.5 Security hardening

Implement:

- Input validation.
- Rate limiting.
- Maximum message sizes.
- Authentication before input.
- Protection against cross-site WebSocket hijacking.
- Restricted host/origin acceptance.
- Reasonable HTTP security headers.
- No unnecessary publicly exposed endpoints.
- No remotely executable commands or scripts.
- No arbitrary filesystem access.
- No unauthenticated administrative APIs.
- Bounded HTTP bodies, headers, request timeouts, WebSocket frames/messages,
  connections, authentication attempts, pending pairings, and queued bytes.
- Global and per-peer rate limits with action-class-aware policies; coalesce or
  shed only safe motion/scroll traffic, never releases.
- Restrictive Content Security Policy with no third-party assets or inline
  executable script, `frame-ancestors 'none'`, `Referrer-Policy: no-referrer`,
  `X-Content-Type-Options: nosniff`, and appropriate same-origin resource policy.
- Immediate close and neutralization for malformed, oversized, unauthorized,
  impossible-sequence, or persistently over-limit control messages.

Desktop administration—including device lists, revoke/stop actions, settings,
logs, and diagnostics mutations—must be available only through narrow Tauri IPC,
not LAN HTTP endpoints.

A compromised paired phone is effectively a remote input device. The application must not hide this risk.

---

## 21. Native Input Backend Architecture

Define an internal Rust `InputBackend` abstraction.

The application core must not call platform APIs directly.

Required capabilities:

- Relative pointer movement.
- Mouse-button down/up.
- Mouse clicks.
- Vertical scrolling.
- Horizontal scrolling.
- Physical-key down/up.
- Text insertion.
- Media commands where supported.
- Release-all state.
- Capability reporting.
- Permission/status reporting.

Each backend must convert platform-neutral actions into the operating system's native events.

### 21.1 Capability reporting

Backends must expose capabilities such as:

- Pointer control available.
- Keyboard control available.
- Text insertion available.
- Horizontal scrolling available.
- Media keys available.
- Permission granted.
- User authorization required.
- Current backend initialized.

If a feature is unsupported, do not fake success.

### 21.2 Backend execution model

Avoid executing blocking native input operations directly on asynchronous network tasks.

Use an appropriate serialized input dispatcher or dedicated execution context.

Preserve event ordering and prevent concurrent backend mutation.

The ordered dispatcher is the only route from an authenticated session to an
`InputBackend`. Pointer movement, scroll, mouse buttons, physical keys, text,
media actions, ownership transitions, and releases share one defined order.
Provide a reserved emergency control path so saturation cannot enqueue
`release_all()` behind an unsafe backlog. Backend errors must fail closed,
neutralize tracked state, and update capability/status reporting.

---

## 22. Windows Backend

Use Rust's `windows` crate and native Win32 APIs.

Primary input API:

`SendInput`

Implement:

- Relative cursor movement.
- Left/right/middle buttons.
- Extra buttons where supported.
- Vertical wheel input.
- Horizontal wheel input.
- Keyboard scan-code injection.
- Unicode text input.
- Media keys where supported.

For `KEYEVENTF_UNICODE`, convert protocol UTF-8 to well-formed UTF-16 code units.
Emit correctly ordered Unicode press/release events for every code unit,
including high/low surrogate pairs for characters outside the Basic
Multilingual Plane. Reject malformed input and test emoji, supplementary
characters, combining sequences, and application-specific behavior.

### Windows restrictions

Respect User Interface Privilege Isolation.

Normal unelevated Tether sessions must not claim to control elevated applications or secure desktop prompts.

Do not automatically request Administrator privileges for normal operation.

Do not set `uiAccess`, add a privileged helper, or require Administrator rights
for the baseline product. These would materially change the installation and
security model. Report likely UIPI limitations honestly; `SendInput` does not
reliably identify UIPI as the cause of failure.

The UI must explain permission/integrity restrictions.

Ensure correct behavior across:

- Multiple monitors.
- Different DPI scaling.
- Different keyboard layouts.
- Common application shortcuts.
- User pointer-speed and acceleration settings.
- Mixed-DPI multi-monitor transitions.

Do not claim Tether can cancel the operating system's pointer acceleration.
Tune and validate Tether's own transfer function with the OS behavior enabled,
and offer predictable settings without using absolute desktop coordinates.

---

## 23. macOS Backend

Use Quartz/CoreGraphics event facilities through maintained Rust bindings or appropriately isolated native FFI.

Relevant functionality includes:

- `CGEvent`.
- `CGEventPost`.
- Keyboard events.
- Mouse events.
- Scroll-wheel events.
- Unicode keyboard text where supported.

### macOS permissions

Provide Accessibility permission onboarding.

Display permission state and a button opening the appropriate system settings.

Do not repeatedly attempt inaccessible input operations.

Preflight Accessibility trust before accepting control and re-check it at
startup, activation, and relevant permission/lifecycle transitions. Do not use a
user-visible synthetic “canary” event merely to test permission. If trust is
absent or revoked, stop input admission, release what can be released, and show
actionable guidance.

macOS 15 and later also apply Local Network privacy independently of
Accessibility. Package the required `NSLocalNetworkUsageDescription` and
Bonjour service declarations, initiate permission-sensitive network operations
from an appropriate foreground lifecycle, handle denial or an initially failed
operation with retry guidance, and distinguish Local Network failures from
Accessibility failures. There is no assumption of a universal permission-status
query.

Maintain a stable bundle identifier, build UUID, and Apple-issued signing
identity for release and permission-lifecycle validation. Signed/notarized
release artifacts are required before claiming reliable permission persistence;
development and CI behavior must not be generalized to release behavior.

### macOS behavior

Support:

- Command/Option/Control/Shift semantics.
- Natural/traditional scroll settings.
- Multiple monitors.
- Retina/high-density displays.
- Common keyboard shortcuts.
- Menu bar/background operation.
- User pointer acceleration and mixed-density multi-display transitions.

Unicode event support must be tested against actual applications because applications may process keyboard events differently.

Do not promise unsupported control of system authorization dialogs.

---

## 24. Linux X11 Backend

Use `x11rb` and XTEST or an equivalent direct X11 input mechanism.

Do not require `xdotool`.

Implement:

- Cursor movement.
- Mouse buttons.
- Vertical scrolling.
- Horizontal scrolling where supported.
- Keyboard events.
- Modifier state.
- Character/key mapping.

The guaranteed X11 keyboard surface is physical key down/up plus text that maps
through the active XKB layout. Query XTEST availability, derive mapping/state
from the active X server, and observe layout, group, modifier, and keymap
changes. Never mutate the global X keyboard map to manufacture characters.

Account for X11 limitations involving:

- Keyboard layouts.
- Unicode insertion.
- Wheel-button emulation.
- Fractional scroll input.

XTEST scroll injection uses discrete wheel button presses. Accumulate normalized
fractional vertical and horizontal deltas independently, emit balanced Button
4/5 and Button 6/7 press/release pairs only when a threshold is crossed, and
retain the remainder. Test both axes, both directions, large bursts, cancellation,
connection loss, and applications that do not respond to horizontal buttons.

Arbitrary Unicode insertion is capability-dependent and is not guaranteed.
Preflight Compose and Send text against the active mapping where possible;
report precise unsupported characters or capability failure instead of claiming
success. Do not silently replace the user's `CLIPBOARD`/`PRIMARY` selection,
send a paste shortcut, or alter the global keymap. Any future clipboard-based
fallback requires explicit architectural approval and a privacy-preserving
restore/failure design.

---

## 25. Linux Wayland Backend

Wayland must have a separate implementation.

Prefer compositor-approved remote-input mechanisms.

Primary target architecture:

XDG RemoteDesktop portal → ConnectToEIS → EI/libei protocol → compositor.

Potential Rust dependencies:

- `ashpd`.
- `reis` or another suitable maintained EI implementation.

The portal/EI architecture is required, but no Rust EI library is selected
unconditionally. First run a time-bounded compatibility validation of `ashpd`
and the current `reis` against the named supported GNOME/KDE configurations,
including portal versioning, grants, single-use restore-token rotation, EI
handshake, device pause/resume/removal, keymap, ordering, and cleanup. Keep the
implementation behind an internal Wayland adapter. If the Rust implementation
is insufficient, an isolated `libei` FFI option requires an ADR; do not replace
the portal architecture with privilege escalation or `/dev/uinput` by default.

Do not attempt to bypass Wayland's security architecture.

### Wayland session management

Implement:

- Portal permission requests.
- Session lifecycle.
- Device capability negotiation.
- EI connection management.
- Pointer/keyboard authorization.
- Session termination.
- Permission-loss handling.
- Optional EI text capability discovery and lifecycle.

### Compatibility

Test at minimum:

- GNOME Wayland.
- KDE Plasma Wayland.

Do not equate successful compilation with confirmed compositor support.

Where a compositor does not provide the required portal/EI functionality, report the limitation.

The application must not silently fall back to unsafe privilege escalation or unrelated virtual-input mechanisms.

When the compositor advertises the EI text capability and the selected adapter
implements it correctly, use `ei_text.utf8` for UTF-8 text insertion. Chunk text
on Unicode code-point boundaries within the negotiated EI protocol limit
(currently no more than 254 UTF-8 payload bytes plus the terminating NUL and no
more than 254 characters), and separate text from keyboard actions into ordered
frames where the EI protocol leaves mixed ordering undefined. If text is absent,
paused, removed, or rejected, fall back only to capability-reported
layout-mappable physical keys or a clear unsupported result. Do not claim EI
categorically supports or categorically lacks Unicode; the compositor-advertised
device capability is authoritative.

---

## 26. Native Text Insertion

Arbitrary Unicode text is not the same as keyboard scan-code injection.

Implement platform-specific text insertion separately from physical key events.

Document the actual capabilities of each backend.

Use appropriate strategies per platform rather than relying on one universal algorithm.

Expected capability model:

- Windows: UTF-16 `KEYEVENTF_UNICODE` where the focused application accepts it,
  including correct surrogate pairs.
- macOS: CoreGraphics Unicode event strings where accepted by the target
  application, with tested fallbacks limited to approved physical-key mappings.
- X11: active-layout-mappable text; arbitrary Unicode is not guaranteed.
- Wayland: EI UTF-8 text only when advertised and validated; otherwise
  active-layout-mappable key input or an unsupported result.

Text insertion must handle:

- UTF-8 protocol input.
- Native string conversion.
- Multicodepoint characters.
- Long text.
- Focus changes.
- Unsupported applications.
- Error reporting.

Never expose raw user text in diagnostics.

No backend may silently use or overwrite a clipboard to simulate text. A future
clipboard-assisted design is a separate, explicit user-facing feature requiring
privacy review and architectural approval.

---

## 27. Desktop Application

Use Tauri 2 for the desktop configuration interface.

The native Rust core should remain independent of Tauri.

Only one desktop-agent instance may own the installation identity, settings
store, listeners, mDNS name, tray state, or input session at a time. A second
launch must activate the existing instance or exit safely; it must not race for
ports, corrupt storage, or create a second input owner.

### 27.1 Features

The desktop application must provide:

- System tray/menu-bar access.
- Connection status.
- QR pairing screen.
- Paired-device management.
- Network-interface selection.
- Input permission status.
- Active session controls.
- App settings.
- Diagnostics.
- Start-at-login option.
- Quit/stop functionality.

### 27.2 Startup behavior

Support optional start-at-login.

Do not enable it silently without user consent.

The agent should continue operating while the settings window is closed, provided it is running in its tray/background mode.

First-run LAN listening follows section 17.3. Start-at-login does not imply
auto-listen: automatic non-loopback listening remains a distinct opt-in setting.

### 27.3 Quit behavior

On full quit:

1. Stop accepting clients.
2. Release all injected input state.
3. Terminate sessions.
4. Unregister discovery advertisements.
5. Close network listeners.
6. Save valid settings.
7. Exit cleanly.

### 27.4 Operating-system lifecycle

Observe suspend, resume, session lock/unlock, logout, network-address change,
and graceful process/OS shutdown notifications where the platform provides
them. Before suspend, lock, logout, or shutdown when notification timing permits:
stop admitting input, release held state, and terminate or suspend ownership.
After resume/unlock, rediscover interfaces and permissions and require a fresh
session epoch; never restore held keys or buttons. Unobservable or uncatchable
termination remains subject to section 16.2.

---

## 28. Application Settings

### Desktop settings

- Installation/computer name.
- Server port.
- Active network interfaces.
- Start at login.
- Auto-start listening.
- Paired devices.
- Device revocation.
- Control ownership.
- Permission diagnostics.
- Log level.
- Reset application settings.

### Phone settings

- Pointer sensitivity.
- Pointer acceleration.
- Vertical strip position.
- Vertical strip width.
- Horizontal strip height.
- Vertical sensitivity.
- Horizontal sensitivity.
- Vertical scroll direction.
- Horizontal scroll direction.
- Scroll inertia.
- Tap-to-click.
- Double-tap-to-drag.
- Long-press-to-drag.
- Multitouch gestures.
- Theme.
- Control layout.
- Haptic preferences where available.
- Keep Screen Awake.

Settings should have sensible defaults and be easy to reset.

Auto-start listening defaults off until the first interface approval and must
be independently opt-in. The Keep Screen Awake preference defaults off and
remains a mobile setting with clear battery impact.

Persist desktop settings, approved interface identities, installation identity,
and paired-device credential digests in versioned schemas under the platform's
application-data directory. Use atomic temp-write, flush/sync as appropriate,
and replace/rename semantics; restrict file permissions to the owning user where
supported. Validate migrations. Quarantine corrupt settings and recover with
safe defaults; corrupt credential state fails closed and requires re-pairing.
Never write partial or plaintext secrets to recovery, backup, or log files.

---

## 29. Clipboard and Additional Input Features

Clipboard functionality is secondary to reliable mouse/keyboard input, but the architecture should permit it.

### Explicit text transfer

The mobile interface should provide a text field where the user can paste or compose text and send it to the computer.

This must work without relying on the secure-context-only asynchronous Clipboard API.

### Desktop clipboard read

If implemented, reading the desktop clipboard must require explicit user action and a permission/capability check.

Do not continuously synchronize clipboards in the background.

### Shortcut deck

Provide a configurable shortcut panel with user-defined labels and key combinations.

Do not implement arbitrary shell-command execution as a shortcut feature.

### Multiple displays

Relative cursor movement should naturally work across the computer's display configuration.

Do not require display capture or absolute display coordinates.

---

## 30. Runtime Performance

The application should have low idle overhead.

### Frontend

- Avoid unnecessary React re-renders during finger movement.
- Store high-frequency gesture data in refs or dedicated engine state rather than global reactive state.
- Avoid rebuilding component trees for every cursor delta.
- Avoid large UI libraries used only for minor controls.

### Backend

- Keep networking asynchronous.
- Serialize input dispatch safely.
- Bound all queues.
- Coalesce high-frequency pointer traffic.
- Keep authentication and protocol parsing lightweight.
- Avoid unnecessary disk writes.

### Performance diagnostics

Record safe aggregate metrics:

- Connection RTT.
- Input messages per second.
- Motion coalescing rate.
- WebSocket outbound buffer.
- Native injection duration.
- Failed input operations.
- Connection/reconnection counts.

Never log actual typed text or key contents.

Performance targets must be verified on hardware before claiming specific end-to-end latency figures.

---

## 31. Repository Structure

Use a Rust Cargo workspace and a pnpm workspace.

Recommended structure:

- `README.md` — project overview.
- `SPEC.md` — this authoritative product specification.
- `AGENTS.md` — concise instructions for coding agents.
- `Cargo.toml` — Rust workspace.
- `package.json` — JavaScript workspace.
- `pnpm-workspace.yaml` — workspace configuration.
- `proto/` — authoritative Protocol Buffers schemas.
- `apps/mobile-web/` — phone/tablet web interface.
- `apps/desktop/` — Tauri application.
- `crates/agent-core/` — application lifecycle and orchestration.
- `crates/network/` — HTTP/WebSocket transport.
- `crates/discovery/` — mDNS/DNS-SD.
- `crates/pairing/` — authentication and paired devices.
- `crates/protocol/` — Rust protocol implementation.
- `crates/input-core/` — shared input abstractions and state.
- `crates/input-windows/` — Windows input backend.
- `crates/input-macos/` — macOS input backend.
- `crates/input-x11/` — X11 input backend.
- `crates/input-wayland/` — Wayland input backend.
- `tests/` — integration and system tests.
- `docs/` — component-specific documentation.
- `.github/workflows/` — CI and release workflows.

The precise crate boundaries may be refined through an accepted ADR when a
concrete dependency, build-target, FFI-isolation, or testability analysis shows
a benefit.

Avoid crates that exist only to satisfy an arbitrary directory structure.

---

## 32. Recommended Dependencies

### Rust

Preferred libraries, subject to compatibility and verification at implementation time:

- `tokio`
- `axum`
- `tower`
- `tower-http`
- `prost`
- `serde`
- `thiserror`
- `tracing`
- `tracing-subscriber`
- `mdns-sd`
- `rand`
- `sha2`
- `qrcode`
- `windows`
- appropriate Apple CoreGraphics bindings
- `x11rb`
- `ashpd`
- `reis`, if suitable for the target Wayland implementation

### Frontend

- React.
- TypeScript.
- Vite.
- `@bufbuild/protobuf`.
- Zustand, only for shared application state.
- Zod where runtime validation adds value.

### Testing

- Rust built-in test framework.
- Vitest.
- React Testing Library.
- Playwright.
- Appropriate native integration-test harnesses.

### Formatting/linting

- `cargo fmt`.
- Clippy.
- ESLint.
- Prettier.

### Dependency policy

Use maintained stable versions.

Verify current APIs and compatibility before implementation.

Avoid pinning dependencies based solely on an old architectural document.

Commit reproducible lockfiles.

Prefer mature platform APIs over convenience wrappers when the wrapper creates long-term capability restrictions.

Do not use Enigo as the mandatory foundation of the production input architecture.

---

## 33. Test Architecture

Testing is mandatory and must be considered during design.

### 33.1 Mock input backend

Implement `MockInputBackend`.

It must record semantic input actions without touching the real computer.

Use it for the majority of integration and protocol tests.

### 33.2 Protocol tests

Test:

- Rust → TypeScript serialization.
- TypeScript → Rust serialization.
- Protocol versioning.
- Invalid messages.
- Oversized messages.
- Unknown enum values.
- Unsupported commands.
- Ordered batches and flush-before-state-change behavior.
- Per-epoch sequence validation and stale-session rejection.
- Application-layer heartbeat timeout independent of socket activity.
- Queue saturation and emergency release priority.
- Rate-limit, connection-limit, and malformed-message cleanup.

### 33.3 Gesture tests

Test:

- Tap.
- Double tap.
- Long press.
- Drag.
- Double-tap drag.
- Two-finger tap.
- Three-finger tap.
- Gesture cancellation.
- Pointer capture loss.
- Region boundary transitions.
- Orientation changes.
- Browser suspension.
- Left/right edge-navigation avoidance with each vertical-strip position.
- Safe-area and visual-viewport changes.
- Both drag methods enabled separately and together.
- Tap latency with double-tap drag disabled and enabled.

### 33.4 Scroll tests

Test:

- Vertical scrolling.
- Horizontal scrolling.
- Independent axis directions.
- Sensitivity.
- Fractional movement.
- Momentum.
- Momentum cancellation.
- Repeated short swipes.
- High-speed swipes.
- Interaction with the neutral corner.
- Interior edge gutter and minimum usable strip target.
- X11 fractional accumulation into balanced discrete wheel buttons.

### 33.5 Keyboard tests

Test:

- Normal text.
- Accented text.
- Emoji.
- Multiline text.
- Paste/compose-and-send.
- Backspace.
- Modifier combinations.
- Sticky modifiers.
- Modifier lock.
- Input method composition.
- Connection loss while a key is held.
- Synchronous proxy-input focus from a user gesture.
- Software-keyboard `visualViewport` resize/scroll behavior.
- Unmappable X11 characters and capability-aware Wayland EI text.
- Windows supplementary characters encoded as UTF-16 surrogate pairs.

### 33.6 Networking tests

Test:

- Normal pairing.
- Invalid pairing token.
- Expired token.
- Reused token.
- Device revocation.
- Cross-origin attempts.
- Invalid authentication.
- Reconnection.
- WebSocket heartbeat timeout.
- Multiple connected devices.
- Control ownership transfer.
- IP-address change simulation.
- Hostname resolution failure.
- Server restart.
- First-run interface approval before any non-loopback listener.
- Auto-listen opt-in and removed/renamed approved interfaces.
- On-link admission and rejection across IPv4, IPv6, VPN, and routed peers.
- Exact Host/Origin validation, including IPv6 and trailing-dot aliases.
- Fragment pairing flow with no token in request targets, history, or logs.
- Separate `.local` and IP cookie/pairing scopes.
- Bounded HTTP bodies, WebSocket frames, connections, pending approvals, and
  per-peer/global rates.
- Queue flood, malformed-frame, slow-request, and cleanup behavior.

### 33.7 Native tests

Native injection tests must be opt-in because they manipulate the testing computer.

Validate platform behavior on real operating systems.

Native validation must explicitly cover pointer acceleration, user pointer
speed, mixed-DPI and mixed-density multiple monitors, display-boundary crossing,
both scroll axes, keyboard layouts, Unicode capability boundaries, permission
revocation, session lock, sleep/resume, and release ordering.

Do not let destructive desktop-input tests run automatically on ordinary developer workstations.

---

## 34. Browser and Device Testing

Automated browser tests must cover multiple screen sizes and engines.

At minimum:

- Mobile Chromium.
- WebKit browser testing.
- Portrait.
- Landscape.
- Small phones.
- Large phones.
- Tablets.

Real-device testing is also required.

At minimum:

- A current iPhone with Safari.
- An Android phone with Chrome.
- A current iPad with Safari.
- An Android tablet with Chrome.

Browser simulation alone is not sufficient for:

- Native software keyboards.
- iOS viewport behavior.
- Actual touch cancellation.
- Device safe areas.
- Local hostname resolution.
- Network permission behavior.
- Home-screen shortcut behavior.
- Edge-navigation/system-back gesture conflicts.
- Synchronous software-keyboard activation.
- Visual viewport behavior while the keyboard is open.
- Screen Wake Lock eligibility, revocation, and browser-dependent video fallback.

Any untested real-device capability must be marked as unverified.

---

## 35. Cross-Platform Validation

### Windows

Test:

- Windows 11 x86-64 as Tier 1.
- Each specifically claimed serviced Windows 10 LTSC/IoT/ESU configuration;
  ordinary unsupported Windows 10 only as clearly labeled best-effort evidence.
- Mouse movement.
- Horizontal and vertical scrolling.
- Normal keyboard shortcuts.
- Unicode text.
- Multiple monitors.
- Normal vs elevated target applications.
- Firewall onboarding.
- Start at login.
- User pointer acceleration/speed and mixed-DPI multi-monitor transitions.
- Supplementary Unicode/surrogate pairs in representative applications.

### macOS

Test:

- Apple Silicon.
- Intel where supported.
- Accessibility authorization.
- Pointer movement.
- Scrolling.
- Unicode input.
- Command-based shortcuts.
- Background/menu-bar mode.
- Sleep/wake reconnection.
- Local Network allow/deny and retry behavior on macOS 15 or later.
- Stable signed application identity across install/upgrade and Accessibility
  and Local Network permission lifecycle.
- Pointer acceleration and mixed-density multi-display transitions.

### Linux

Test:

- X11.
- GNOME Wayland.
- KDE Wayland.
- Portal authorization.
- Available pointer/keyboard capabilities.
- Logout/relogin behavior.
- Missing portal functionality.
- X11 fractional wheel accumulation and unmappable-text reporting.
- Wayland EI text present, absent, paused, removed, and length/chunk boundaries.
- Compositor restart and restore-token rotation.

Name exact distribution, desktop/compositor, display-server, portal, EI/XTEST,
architecture, and package versions in each support claim. Document unsupported
desktop environments explicitly; compilation is not validation.

---

## 36. Continuous Integration

Configure a cross-platform CI matrix.

### Rust checks

- Formatting.
- Clippy.
- Unit tests.
- Integration tests.
- Release build.

### Frontend checks

- Type checking.
- Linting.
- Unit tests.
- Production build.
- Browser tests.

### Protocol checks

- Schema validation.
- Generated-code consistency.
- Cross-language contract tests.

### Packaging checks

Build desktop artifacts on their supported platforms.

A successful build on one operating system does not imply another operating system is supported.

CI must report which platform tests were actually run and which require manual hardware testing.

---

## 37. Distribution and Packaging

### Windows

Provide a normal installer.

Support:

- Installation.
- Uninstallation.
- Start menu access.
- Private-network firewall setup.
- Start-at-login.
- Signed production binaries.

Public production releases must be signed. Installer and firewall behavior must
preserve an unelevated runtime and must not imply control of higher-integrity
applications.

### macOS

Provide a standard macOS application package and distribution image.

Support:

- Accessibility onboarding.
- Apple Silicon.
- Supported Intel machines.
- Menu-bar behavior.
- Developer ID signing/notarization for public release.
- Stable bundle/signing identity and required Local Network privacy/Bonjour
  metadata.

### Linux

Provide practical native packages such as:

- `.deb`
- `.rpm`
- AppImage where practical

Do not choose a sandboxed package as the only Linux distribution method without verifying that it supports the required input portals and permissions.

### Mobile

No native installation required.

The phone opens the locally hosted web page.

The page should be bookmarkable.

---

## 38. Diagnostics and Troubleshooting

Implement a diagnostics screen.

It should display:

- Agent version.
- Protocol version.
- Operating system.
- Selected input backend.
- Input capabilities.
- Permission status.
- Network interfaces.
- Local hostname.
- Listening address/port.
- mDNS advertisement status.
- WebSocket status.
- Paired devices.
- Active controller.
- Aggregate performance metrics.
- Last non-sensitive error.

Provide simple, user-facing explanations for:

- Phone and PC on different networks.
- Guest Wi-Fi isolation.
- mDNS resolution failure.
- Firewall blocking.
- Computer asleep.
- Input permission missing.
- Unsupported Wayland compositor.
- Expired/revoked pairing.
- Protocol mismatch.

Do not expose sensitive credential or typing information through diagnostics.

---

## 39. Development Principles

### Architecture consistency

Keep the following separate:

- UI.
- Gesture processing.
- Scroll processing.
- Network transport.
- Protocol.
- Authentication.
- Session state.
- Input processing.
- Platform input backends.
- Desktop shell.

### No unnecessary abstraction

Introduce an abstraction when it enforces a meaningful boundary or permits testing/platform variation.

Do not invent generic framework layers without a demonstrated need.

### Correctness over superficial completeness

A feature that compiles but does not work on real supported platforms is incomplete.

### Platform limitations

If a requirement cannot be implemented reliably because of OS or browser restrictions:

1. Explain the limitation.
2. Identify the affected platform.
3. Verify the actual technical constraint.
4. Implement the best supported behavior.
5. Add tests or manual verification instructions.
6. Document the remaining gap.

Never silently simulate success.

---

## 40. Instructions for Codex and Antigravity

This repository is expected to be implemented primarily through AI coding agents.

The agents must operate as disciplined software engineers, not as unconstrained code generators.

### 40.1 Authoritative documents

`SPEC.md` defines product requirements.

`AGENTS.md` defines concise repository-wide engineering rules.

Component documents in `docs/` define implementation details consistent with the specification.

If these documents contradict one another, identify and resolve the conflict before making architectural changes.

### 40.2 Initial architecture review

Before writing application code, conduct a technical review of this specification.

Verify:

- Local HTTP/WebSocket browser compatibility.
- DHCP/mDNS behavior.
- Pairing and cookie limitations.
- Cross-origin security.
- Protocol correctness.
- Pointer/scroll design.
- Keyboard/IME behavior.
- Session cleanup.
- Native input APIs.
- Tauri integration.
- Linux Wayland support.
- Cross-platform packaging.
- Testing feasibility.

Flag genuine contradictions or technically infeasible requirements.

Do not rewrite the architecture merely because another framework is familiar.

Produce a clear list of proposed changes and the reasons for them.

Obtain approval for material architectural deviations.

The initial review decisions are accepted and recorded in
`docs/architecture-decisions.md`. Future work must not reopen those decisions
without new technical evidence and a replacement ADR. Remaining validation
risks are tracked in `docs/open-risks.md`; a risk marked “requires validation”
must not be converted into a support claim by assumption.

### 40.3 Planning

After the architecture is accepted, create a dependency-ordered engineering task list.

Track task status in the repository.

Do not replace the final architecture with an MVP architecture.

It is acceptable to have incomplete components while building. It is not acceptable to treat temporary implementations as the finished design.

### 40.4 Implementation order

Recommended dependency order:

1. Repository/workspace setup.
2. Architecture and engineering documentation.
3. Protocol schema and code generation.
4. Input core and mock backend.
5. Session ownership/input state.
6. Authentication and pairing.
7. HTTP/WebSocket networking.
8. Static web asset embedding.
9. Mobile UI shell.
10. Touchpad gesture engine.
11. Vertical/horizontal scrolling engine.
12. Mouse-button controls.
13. Keyboard and text-entry subsystem.
14. Windows input backend.
15. macOS input backend.
16. Linux X11 backend.
17. Linux Wayland backend.
18. Tauri desktop settings/tray.
19. Network discovery and dynamic-IP handling.
20. Full integration testing.
21. Packaging and release automation.
22. Real-device and cross-platform hardening.
23. Performance tuning and final review.

This order may be adjusted to account for actual dependencies, but architectural boundaries must remain intact.

### 40.5 Parallel agent work

Multiple agents may work in parallel on independent components.

Use separate branches or Git worktrees when appropriate.

Avoid concurrent edits to shared protocol definitions, shared core interfaces, or build configuration without coordination.

Integrate components only after their applicable tests pass.

### 40.6 Model usage

Use the strongest available reasoning/coding models for:

- Architecture review.
- Security design.
- Protocol design.
- Session-state correctness.
- Native OS APIs.
- Wayland.
- Difficult concurrency problems.
- Final review.

Use efficient coding models for:

- Isolated UI components.
- Test expansion.
- Routine integration.
- Documentation.
- Mechanical refactoring.

For the currently available Antigravity models, Claude Opus 4.6 Thinking is the preferred initial architecture and difficult-backend reviewer; Claude Sonnet 4.6 Thinking is suitable for routine implementation.

For Codex, choose the strongest currently available appropriate reasoning model for architecture, and a high-capability coding model for implementation. Do not hardcode the project workflow to one model name or assume model availability remains unchanged.

Independent reviews across model families are encouraged for high-risk components.

### 40.7 Verification obligations

After a task, the implementing agent must:

1. Run relevant formatting.
2. Run relevant lint/static checks.
3. Run unit tests.
4. Run affected integration tests.
5. Build affected components.
6. Fix failures where possible.
7. Update documentation.
8. Report what actually passed.
9. Report untested or unsupported behavior.

Never suppress a failing test merely to achieve a green build.

Never claim manual hardware testing was performed unless it actually was.

### 40.8 Autonomous execution

Agents may continue implementing independent, approved tasks without requesting confirmation at every step.

They must stop and request clarification when:

- A major architectural requirement conflicts with verified platform behavior.
- A security assumption is invalid.
- A destructive or privileged operation requires user approval.
- A dependency would materially change the architecture.
- A design ambiguity could cause substantial rework.

Otherwise, proceed through the task list and verify completed work.

---

## 41. Acceptance Criteria

The project is ready for a full release only when the following are satisfied.

### Installation

- A user can install the desktop application without installing developer tools.
- The application can run from the desktop tray/menu bar.
- The application can start at login when enabled.

### Connectivity

- A phone can load the mobile UI from the computer over a private LAN.
- No internet connection is required.
- No central server is required.
- Pairing works through a QR code.
- Device credentials persist appropriately.
- Credentials can be revoked.
- The application handles ordinary dynamic IP changes.
- Hostname failures have a usable fallback.
- Reconnection behaves safely.
- No non-loopback listener starts before first-run interface approval.
- Automatic listening occurs only after explicit opt-in.
- Listeners bind only approved addresses and reject peers outside the default
  selected-interface/on-link admission policy.
- `.local` remains preferred where it resolves and an IP QR fallback is usable.
- Changing hostname/IP pairing scope is explained and safely re-pairs.

### Mouse

- Relative pointer movement works.
- Left/right/middle click works.
- Dragging works.
- Disconnection does not leave buttons held.
- Touchpad sensitivity is configurable.
- Coalesced motion is flushed before clicks and other state changes.
- Pointer behavior has been tested with OS acceleration, mixed DPI/density, and
  multiple monitors on each claimed platform.

### Scrolling

- Vertical scrolling uses a dedicated side strip.
- The strip can be positioned on either side.
- Horizontal scrolling uses a dedicated bottom strip.
- Axes remain independent.
- Sensitivity and direction are configurable.
- Momentum works when enabled.
- Unsupported backend behavior is reported accurately.
- Edge-navigation insets preserve usable left/right vertical strips and the
  neutral corner remains inactive.
- X11 scroll fractions accumulate into balanced discrete wheel-button events.

### Keyboard

- Text entry works within documented platform capabilities.
- Special keys work.
- Modifiers work.
- Sticky modifiers work.
- Shortcuts work within OS restrictions.
- Compose and Send works.
- Held keys are released after disconnect.
- Unicode/IME limitations are documented and tested.
- Mobile keyboard activation is synchronous with the user gesture and remains
  usable through visual viewport changes.
- Windows supplementary Unicode uses valid UTF-16 surrogate pairs.
- X11 guarantees physical keys and layout-mappable text without clipboard or
  global-keymap mutation.
- Wayland uses EI UTF-8 text only when advertised and validated, with honest
  fallback otherwise.

### Security

- Unpaired clients cannot inject input.
- The desktop user approves new pairings.
- Revocation immediately disables control.
- Only the authorized active controller injects input.
- Input contents and credentials are never logged.
- Dangerous network messages are rejected.
- The user can stop remote input immediately.
- Host and Origin allowlists reject cross-site and rebinding-style browser
  access attempts.
- Tokens use fragment-to-POST handling and do not appear in request/log history.
- HTTP, WebSocket, authentication, pairing, and dispatch resources are bounded
  and rate-limited without starving emergency release.

### Quality

- Automated tests pass.
- Builds complete on targeted platforms.
- Real mobile browsers have been tested.
- Native input functionality has been validated on actual supported desktop OS configurations.
- Known limitations are documented.
- No central service or runtime internet dependency has been introduced.
- Resource consumption and interaction latency have been measured.
- Application-layer JavaScript heartbeats release ownership when page execution
  is suspended even if the underlying socket remains open.
- Every tested observable/graceful loss path ends with neutral input state;
  uncatchable termination limits are documented.
- Single-instance behavior and atomic, versioned storage survive restart,
  interrupted writes, and corrupt state safely.
- Keep Screen Awake accurately reports native availability and any tested
  best-effort fallback without claiming a guarantee over HTTP.

---

## 42. Known Technical Risks

These risks must remain visible during development.

### Risk A: Local HTTP security

HTTP does not encrypt remote input traffic.

Local-only deployment reduces internet exposure but does not guarantee privacy against local network attackers.

### Risk B: Mobile browser limitations

Software keyboard, visual viewport, edge navigation, local-network access,
`.local` resolution, clipboard, backgrounding, keep-awake fallbacks, and
secure-context behavior vary across browsers.

Real-device verification is mandatory.

### Risk C: Wayland compatibility

The RemoteDesktop portal and EI/libei mechanisms depend on compositor support and user authorization.

Some desktop environments may not provide full requested functionality.

The current Rust EI implementation must be validated before selection. EI text
is optional and depends on the compositor-advertised capability.

### Risk D: Unicode text injection

Not every application treats synthetic Unicode input identically.

A robust compose-and-send fallback is important.

On X11 even Compose and Send may reject characters not representable by the
active layout. No silent clipboard mutation is permitted.

### Risk E: Privileged/secure desktop interaction

Normal agents cannot universally interact with elevated or protected system interfaces.

No privilege bypasses should be implemented.

### Risk F: LAN discovery

`.local` name resolution can fail because of network configuration or platform behavior.

An IP-based connection fallback is required.

### Risk G: Browser background suspension

Mobile browsers may pause scripts when the user switches apps or locks the phone.

Control state must be maintained authoritatively and safely on the PC.

### Risk H: Uncatchable termination

Forced process termination, kernel/OS failure, power loss, or a failed native
injection facility can prevent cleanup code from running. The system minimizes
the risk through short application-heartbeat expiry, centralized state,
best-effort panic/drop cleanup, and startup neutralization, but does not claim an
impossible absolute guarantee.

### Risk I: Pointer behavior across desktop configurations

Operating-system acceleration, user pointer settings, DPI scaling, display
density, and mixed-monitor layouts affect injected relative motion. Platform
tuning and real-hardware validation are required; identical physical behavior
cannot be assumed.

---

## 43. Technical References

The following references are preferred sources for implementation decisions.

**Web platform**

- MDN secure contexts: https://developer.mozilla.org/en-US/docs/Web/Security/Defenses/Secure_Contexts
- MDN WebSocket: https://developer.mozilla.org/en-US/docs/Web/API/WebSocket
- MDN Pointer Events: https://developer.mozilla.org/en-US/docs/Web/API/Pointer_events
- MDN pointer capture: https://developer.mozilla.org/en-US/docs/Web/API/Element/setPointerCapture
- MDN beforeinput: https://developer.mozilla.org/en-US/docs/Web/API/Element/beforeinput_event
- MDN Visual Viewport: https://developer.mozilla.org/en-US/docs/Web/API/VisualViewport
- MDN Screen Wake Lock: https://developer.mozilla.org/en-US/docs/Web/API/Screen_Wake_Lock_API
- MDN secure-context feature restrictions: https://developer.mozilla.org/en-US/docs/Web/Security/Defenses/Secure_Contexts/features_restricted_to_secure_contexts
- Chrome local-network restrictions: https://developer.chrome.com/release-notes/142

**Rust and desktop**

- Axum: https://docs.rs/axum/
- Tokio: https://tokio.rs/
- Tauri 2: https://v2.tauri.app/
- mDNS Rust crate: https://docs.rs/mdns-sd/
- Protocol Buffers for TypeScript: https://protobufes.com/
- Protocol Buffers for Rust: https://docs.rs/prost/

**Networking**

- Multicast DNS RFC 6762: https://www.rfc-editor.org/rfc/rfc6762
- DNS-SD RFC 6763: https://www.rfc-editor.org/rfc/rfc6763
- HTTP cookies RFC 6265: https://www.rfc-editor.org/rfc/rfc6265
- WebSocket RFC 6455: https://www.rfc-editor.org/rfc/rfc6455
- Android `.local` resolver: https://source.android.com/docs/core/ota/modular-system/dns-resolver

**Windows**

- Microsoft SendInput: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput
- Microsoft mouse input: https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-mouseinput
- Microsoft keyboard input: https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-keybdinput
- Microsoft Windows 10 lifecycle: https://learn.microsoft.com/en-us/lifecycle/announcements/windows-10-end-of-support

**macOS**

- Apple CoreGraphics: https://developer.apple.com/documentation/coregraphics
- Apple local-network privacy: https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy

**Linux**

- XDG RemoteDesktop portal: https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html
- libei: https://libinput.pages.freedesktop.org/libei/
- EI text interface: https://libinput.pages.freedesktop.org/libei/interfaces/ei_text/
- XTEST protocol: https://www.x.org/releases/current/doc/xextproto/xtest.pdf
- libxkbcommon X11 integration: https://xkbcommon.org/doc/current/group__x11.html
- X11 Rust bindings: https://docs.rs/x11rb/

Agent implementers should consult current API documentation and compatibility information rather than assuming the contents of this specification guarantee a dependency's current behavior.

---

## 44. Final Product Requirement

The finished application must feel like a native wireless mouse and keyboard even though its controller is a mobile web page.

The desktop computer hosts the entire mobile interface, accepts authenticated connections exclusively over the local network, and converts remote input into native operating-system events.

The phone's primary interface must provide a large touchpad, a configurable left/right vertical-scroll strip, and a separate horizontal-scroll strip along the bottom.

There must be no central server, no cloud account, and no mandatory internet connection.

The implementation must prioritize correctness, user experience, maintainability, and honest handling of platform restrictions.

**This document defines the intended final product. All implementation, testing, and release work must be consistent with it.**
