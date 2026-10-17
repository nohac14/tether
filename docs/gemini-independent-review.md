# Independent Architecture and Technical Feasibility Review: Tether

**Document Evaluated:** `SPEC.md` (Tether — Complete Product and Engineering Specification)  
**Review Type:** Independent Architecture, Security, Platform API, and Feasibility Review  
**Date of Review:** October 2026  
**Audience:** System Architects, Core Maintainers, Independent Reviewers, and Implementation Teams

---

## 1. Executive Summary & Architecture Feasibility Verdict

The proposed specification (`SPEC.md`) outlines a self-hosted, local-network remote input utility (phone-as-touchpad/keyboard) combining a Rust/Tauri 2 desktop host (Axum, Tokio, Prost) with a zero-install mobile web client (React, Vite, `@bufbuild/protobuf`).

### Feasibility Verdict: **Feasible with Required Architectural Revisions**

The core concept is sound and well-grounded in production engineering principles. However, the specification contains several **critical technical inaccuracies regarding platform APIs**, **infeasible web assumptions on non-secure (HTTP) origins**, and **fragile native backend expectations** (specifically on Linux X11, Wayland, and mobile touch browsers). Left uncorrected, these issues will cause user-visible failures during implementation (e.g., phone screens sleeping every 30 seconds, broken text typing on Linux/Wayland, broken edge scrolling on iOS/Android, and dropped or out-of-order mouse clicks).

### High-Level Summary of Findings

| Category | Critical | High | Medium | Low / Note | Total |
| --- | :---: | :---: | :---: | :---: | :---: |
| **Technical Errors & Infeasible Requirements** | 2 | 3 | 0 | 0 | **5** |
| **Architectural Weaknesses & Protocol Risks** | 1 | 3 | 1 | 0 | **5** |
| **Missing Requirements & Edge Cases** | 0 | 3 | 2 | 0 | **5** |
| **Unnecessary Complexity** | 0 | 0 | 2 | 1 | **3** |
| **Security & Reliability Concerns** | 1 | 2 | 1 | 0 | **4** |
| **Total** | **4** | **11** | **6** | **1** | **22** |

---

## 2. Technical Errors & Infeasible Requirements

### Finding TE-01: Screen Wake Lock API is Infeasible over Plain HTTP

- **Severity:** `Critical`
- **Specification Section:** §6, §16.3, §18.6 ("Because the LAN web application normally uses HTTP rather than HTTPS...")
- **Technical Justification:**
  The W3C Screen Wake Lock API (`navigator.wakeLock.request('screen')`) is strictly restricted to **Secure Contexts** (`HTTPS` or `localhost`). In an insecure context (`http://<lan-ip>:<port>`), `navigator.wakeLock` is either `undefined` or rejects with a `SecurityError` across all modern browser engines (WebKit/Safari and Chromium).
  Without keeping the screen awake, mobile devices will dim and enter sleep mode after 30 to 60 seconds of idle reading or watching the desktop screen. When the phone sleeps, mobile OSes immediately throttle or suspend the JavaScript runtime, causing WebSocket heartbeat timeouts and dropping the connection.
- **Recommended Correction:**
  1. Explicitly document that native `navigator.wakeLock` is unavailable over plain HTTP.
  2. Implement a proven insecure-context fallback: a continuous, muted, inline looping `<video playsinline loop muted>` element or a tiny WebGL/Canvas micro-render cycle (e.g., the NoSleep.js technique) activated on the first user interaction.
  3. Include a visibility state observer (`document.visibilitychange`) to cleanly trigger session state preservation and wake-up synchronization when the user un-minimizes the browser.

---

### Finding TE-02: Linux X11 (XTEST) Cannot Inject Fractional Scrolling or Arbitrary Unicode

- **Severity:** `Critical`
- **Specification Section:** §24 ("Use x11rb and XTEST... Fractional scroll input... Character/key mapping")
- **Technical Justification:**
  1. **Scrolling:** The X11 XTEST extension (`XTestFakeButtonEvent`) only injects legacy discrete button events: Button 4 (Wheel Up), Button 5 (Wheel Down), Button 6 (Wheel Left), and Button 7 (Wheel Right). XTEST has no protocol mechanism to inject continuous, high-resolution, or fractional valuator scroll deltas (which require XInput 2 XI2 / XSendEvent, but XI2 does not support synthetic slave device injection from arbitrary client connections without root or dedicated virtual device drivers). Expecting fractional scrolling from XTEST is technically impossible.
  2. **Unicode Text:** XTEST can only fake `KeyPress` and `KeyRelease` for an 8-bit `KeyCode` (values 8–255). A key event only produces a character if that KeyCode is currently mapped to a `KeySym` in the user's active X11 keymap. For arbitrary Unicode characters (accented glyphs, CJK, emojis), XTEST cannot inject them unless the application dynamically mutates the user's active X11 keymap via `XChangeKeyboardMapping` (an unstable practice that can cause desktop flicker and race conditions with other X clients).
- **Recommended Correction:**
  1. **Scrolling:** Acknowledge that the X11 XTEST backend must quantize accumulated floating-point deltas into discrete integer button clicks (Buttons 4–7) with a backend accumulator; fractional deltas cannot be streamed directly to the OS.
  2. **Text:** Explicitly define an X11 text insertion fallback strategy: for characters present in the active keymap, use XTEST key events; for all arbitrary Unicode strings, inject via the X11 `CLIPBOARD`/`PRIMARY` selection buffer followed by a synthetic `Ctrl+V` (or `Shift+Insert`) key sequence, or require Compose and Send via clipboard.

---

### Finding TE-03: Linux Wayland (libei / EIS) Lacks Unicode String Insertion Protocol

- **Severity:** `High`
- **Specification Section:** §25 ("XDG RemoteDesktop portal → ConnectToEIS → EI/libei protocol... ashpd, reis")
- **Technical Justification:**
  The Emulated Input (`libei` / `reis`) protocol defines hardware-level emulated input devices (`ei_pointer`, `ei_scroll`, `ei_button`, `ei_keyboard`). The `ei_keyboard` interface accepts only Linux EVDEV scancodes (`KEY_A`, `KEY_ENTER`, etc.) mapped through XKB keymaps. **There is no protocol message in `libei` for arbitrary Unicode text or string insertion.**
  Furthermore, the XDG RemoteDesktop portal session creates an isolated virtual seat; it does not grant direct access to the Wayland text-input or input-method protocols (`zwp_text_input_v3`).
- **Recommended Correction:**
  Update the Wayland backend specification to state:
  1. Physical key actions map to EVDEV scancodes via XKB.
  2. Arbitrary text insertion on Wayland cannot be injected as direct keystrokes via `libei`. It must use either:
     - The XDG Data Control / Wayland clipboard portal to paste content into the active focus.
     - A clear UI capability indicator stating that direct Unicode typing is constrained to characters resolvable by the desktop's active XKB layout.

---

### Finding TE-04: Android Chrome Fails Unicast mDNS (`.local`) Hostname Resolution

- **Severity:** `High`
- **Specification Section:** §5.2, §18.1, §18.2 ("Connection address: <http://tether-a8f3.local:PORT>", "Use mDNS to publish the hostname")
- **Technical Justification:**
  Historically, Android's Bionic libc stub resolver lacked an mDNS Name Service Switch (NSS) module. While Android 12+ introduced an mDNS resolver for NSD APIs, Chrome on Android does not consistently resolve single-label or multi-label `.local` domain names typed into the address bar or opened via QR code scanner intents. In many Android builds and network setups, entering `tether-a8f3.local` triggers an external Google Search rather than a local DNS lookup, or returns `DNS_PROBE_FINISHED_NXDOMAIN`.
- **Recommended Correction:**
  The QR code must not default solely to the `.local` hostname. It must encode the **LAN IPv4 address URL** (`http://192.168.x.x:PORT/?t=token`) as the primary target or provide a dual-payload / immediate redirect mechanism, with `.local` treated as an auxiliary fallback rather than the primary entry point.

---

### Finding TE-05: Windows `SendInput` Requires Surrogate Pairs for Characters Outside the BMP

- **Severity:** `High`
- **Specification Section:** §22, §26 ("Unicode text input... UTF-8 protocol input... Multicodepoint characters")
- **Technical Justification:**
  When using `SendInput` with `KEYEVENTF_UNICODE`, the Win32 `KEYBDINPUT.wScan` field is a 16-bit unsigned integer (`WORD`). Any Unicode character outside the Basic Multilingual Plane (BMP, code points > `U+FFFF`, such as emojis and supplementary symbols) cannot fit into a single `wScan`. If a 32-bit codepoint is cast directly to `wScan`, the high bits are truncated, injecting corrupted glyphs.
- **Recommended Correction:**
  The Windows input backend must convert incoming UTF-8 strings into UTF-16 code units. For any code point outside the BMP, the backend must emit two consecutive `INPUT` structures containing the High Surrogate (`0xD800..0xDBFF`) and Low Surrogate (`0xDC00..0xDFFF`) with `KEYEVENTF_UNICODE`.

---

## 3. Architectural Weaknesses & Protocol Risks

### Finding AW-01: Insecure LAN Credentials Vulnerable to Passive Sniffing and Keystroke Injection

- **Severity:** `Critical`
- **Specification Section:** §19.3, §20.2 ("Plain HTTP and ws:// do not encrypt traffic... Store only a cryptographic hash of the credential on the PC")
- **Technical Justification:**
  While the specification admits HTTP does not encrypt traffic, the combination of:
  1. Plain HTTP transmission of the persistent cookie / session token, and
  2. Unencrypted WebSocket (`ws://`) input injection,
  creates an immediate vulnerability on any shared local network (e.g., home networks with guest devices, shared apartments, or student housing). Any host running `tcpdump`/Wireshark or executing an ARP spoofing attack can sniff the session cookie and connect to `ws://<pc>:port/control` to inject arbitrary keystrokes (e.g., `Win+R -> powershell -c <payload>` or `Command+Space -> Terminal`).
- **Recommended Correction:**
  Implement application-layer authentication and payload sealing over the WebSocket:
  - Derive an ephemeral session key during the QR pairing handshake (the QR code can transfer a 256-bit symmetric pre-shared key that never traverses the network in plaintext).
  - Use lightweight application-layer encryption (e.g., ChaCha20-Poly1305 via pure JS / WebAssembly `@noble/ciphers` or TweetNaCl on the client and `chacha20poly1305` in Rust) for the WebSocket binary stream. This achieves confidentiality and integrity on the LAN without requiring public CA infrastructure, TLS certificate warnings, or private CA installation on the mobile device.

---

### Finding AW-02: Mobile Edge Swipe Interference on Dedicated Scroll Strips

- **Severity:** `High`
- **Specification Section:** §6.2, §8.1 ("Side strip: vertical scrolling... Default: right side of touchpad. Alternative: left side")
- **Technical Justification:**
  On modern iOS Safari, a swipe starting from the left edge triggers navigation history back, and swiping from the right edge triggers navigation forward. On modern Android (gesture navigation), swiping inward from *either* the left or right screen edge triggers the system Back action.
  CSS `touch-action: none` and `overscroll-behavior: none` **do not override** native iOS Safari history swipe gestures or Android system navigation back gestures when touches originate within ~16–20 CSS pixels of the physical viewport edge. A scroll strip placed flush against the glass edge will result in frequent accidental page transitions or app exits.
- **Recommended Correction:**
  1. Introduce an explicit inset / gutter (`safe-area-inset` + a configurable touch deadzone of at least 16–24px) between the screen edge and the scroll strip touch target.
  2. Provide a visual handle centered inside the strip so users naturally place their thumb away from the screen perimeter.

---

### Finding AW-03: Event Ordering Hazard Between Coalesced Motion and Discrete Inputs

- **Severity:** `High`
- **Specification Section:** §11.2, §12, §15.2 ("Pointer movement may be accumulated and transmitted once per animation frame... Do not send every browser pointer event...")
- **Technical Justification:**
  If pointer delta events are coalesced and buffered until `requestAnimationFrame` (up to 16.6ms at 60Hz), but mouse button clicks, taps, or key events are dispatched immediately to avoid perceived latency, a race condition occurs. A click event generated at timestamp $T_1$ could arrive at the server *before* the coalesced motion vector representing movement up to $T_1$. The desktop OS would then register the click at the old cursor position rather than the new one.
- **Recommended Correction:**
  All input events (motion deltas, button transitions, key presses) must pass through a single, strictly ordered client-side dispatch pipeline. If an atomic click or button down/up occurs, any pending uncommitted motion deltas must be flushed immediately in the *same* network frame preceding the click message.

---

### Finding AW-04: Software Keyboard Invocation & Viewport Resizing on Mobile Browsers

- **Severity:** `High`
- **Specification Section:** §6.1, §14.1 ("Keyboard toggle... Handle beforeinput, input...")
- **Technical Justification:**
  1. **Activation:** Mobile Safari and Android Chrome will only summon the virtual keyboard if an editable DOM element (`<input>` or `<textarea>`) receives focus within the synchronous execution stack of an explicit user gesture (`touchend` or `click`). Calling `.focus()` asynchronously (e.g., after an async state change or network callback) is suppressed by the browser.
  2. **Viewport Disruption:** When the software keyboard opens, mobile browsers shift the visual viewport (`window.visualViewport`), triggering page scrolling, layout compression, and touch offset errors. If the viewport meta tag lacks `interactive-widget=resizes-content` (or if fixed layout boundaries are not adjusted), the touchpad canvas will be compressed or pushed off-screen.
- **Recommended Correction:**
  1. Maintain a permanent off-screen proxy `<input>` element with `autocapitalize="none"`, `autocomplete="off"`, `autocorrect="off"`, and `spellcheck="false"`. The "Keyboard Toggle" button must synchronously focus this element.
  2. Implement a dedicated `visualViewport` resize listener on the web client to dynamically recalculate and anchor the touch canvas dimensions without page scroll jump.

---

### Finding AW-05: Transport-Level vs Application-Level Heartbeat Ambiguity

- **Severity:** `Medium`
- **Specification Section:** §16.3 ("Heartbeat roughly every 1 second while active... Server-side timeouts remain authoritative")
- **Technical Justification:**
  WebSocket TCP-level Ping/Pong frames (RFC 6455) are handled entirely within the browser's native C++ network stack; the JavaScript application layer cannot send or intercept them. When a mobile tab is backgrounded or minimized, the browser's network stack may continue answering TCP and WebSocket pings even though the JavaScript thread is frozen. Relying on transport pings will cause the desktop agent to believe the mobile client is active, leaving held keys or mouse buttons active indefinitely.
- **Recommended Correction:**
  Explicitly mandate **Application-Layer Protobuf Heartbeats** (`HeartbeatPing` / `HeartbeatPong`) generated exclusively by the client's JavaScript runtime loop. If the JavaScript runtime halts due to backgrounding, the application heartbeats cease immediately, triggering the server-side 4-second safety `release_all()` sweep.

---

## 4. Missing Requirements & Edge Cases

### Finding MR-01: Windows UIPI / Elevated Window Inaccessibility (`uiAccess`)

- **Severity:** `High`
- **Specification Section:** §22 ("Respect User Interface Privilege Isolation... Normal unelevated Tether sessions must not claim to control elevated applications...")
- **Technical Justification:**
  Under Windows User Interface Privilege Isolation (UIPI), an unelevated process calling `SendInput` cannot send mouse clicks or keystrokes to any window running with higher integrity (e.g., Task Manager, elevated terminals, installers, administrative tools). While SPEC non-goals acknowledge secure desktop prompts (UAC), users will frequently encounter silent input failure when interacting with elevated applications on their desktop.
- **Recommended Correction:**
  Document the exact technical requirements for optional elevated control:
  - To interact with elevated windows without running the entire application as Administrator, the Windows executable manifest must specify `<requestedExecutionLevel level="asInvoker" uiAccess="true"/>`.
  - Note the OS prerequisite: `uiAccess="true"` requires the binary to be digitally signed by a trusted root certificate and installed inside a secure folder (`%ProgramFiles%` or `%SystemRoot%`). Provide a clear troubleshooting status in the desktop UI when UIPI blocks input injection.

---

### Finding MR-02: macOS TCC Accessibility Permissions Lifecycle & Bundle Identity

- **Severity:** `High`
- **Specification Section:** §23 ("Provide Accessibility permission onboarding... Display permission state and a button opening the appropriate system settings")
- **Technical Justification:**
  On macOS (Ventura, Sonoma, Sequoia), Accessibility permissions (`AXIsProcessTrustedWithOptions`) are bound to the code signature and bundle identifier of the application. During development, rebuilding the binary without a stable ad-hoc or Developer ID signature invalidates macOS TCC permissions, causing `CGEventPost` to silently fail without re-prompting. Furthermore, calling `CGEventPost` when untrusted does not return an error code; it simply drops events.
- **Recommended Correction:**
  1. Explicitly check `AXIsProcessTrusted()` before posting events, and poll permission state at startup.
  2. Implement an active permission validation probe: post a canary event or verify trust before allowing incoming remote sessions.
  3. Ensure the macOS build pipeline applies a stable local signing identity even in development builds.

---

### Finding MR-03: Dynamic Port Selection & Firewall Collisions

- **Severity:** `High`
- **Specification Section:** §17.2, §18.3, §28 ("Server port... Active network interfaces... Firewall handling")
- **Technical Justification:**
  The specification does not define a standard default port or dynamic port reservation policy. If Tether defaults to a hardcoded port (e.g., `8080`) that is occupied by another local service (e.g., dev servers, proxies), the server fails to bind. If it selects an ephemeral dynamic port (port `0`), the QR code and bookmark URLs become invalid every time the desktop app restarts.
- **Recommended Correction:**
  1. Specify a registered/standardized default application port (e.g., `51820` or a dedicated port in the dynamic range like `49200`) with automatic fallback to incremental offsets (`+1`, `+2`) if occupied.
  2. On Windows, register a specific Windows Defender Firewall rule for the executable path rather than an open port rule, ensuring seamless binding across interface changes.

---

### Finding MR-04: Desktop Multi-Display Coordinate Space and Acceleration Asymmetry

- **Severity:** `Medium`
- **Specification Section:** §11.1, §29 ("Platform-specific cursor scaling correction... Relative cursor movement should naturally work across the computer's display configuration")
- **Technical Justification:**
  Relative cursor deltas injected via OS APIs are subjected to native OS pointer ballistics and acceleration curves.
  - On macOS, `CGEventCreateMouseEvent` using relative delta fields (`kCGMouseEventDeltaX`) interacts with macOS's aggressive pointer acceleration curve, making small finger movements feel sluggish and fast movements overshoot.
  - On Windows, `MOUSEEVENTF_MOVE` deltas pass through the Windows "Enhance pointer precision" curve unless raw normalized coordinates are calculated.
  - On multi-monitor setups with mixed DPI scaling (e.g., 4K at 150% + 1080p at 100%), relative inputs can exhibit severe speed variations as the cursor crosses display boundaries.
- **Recommended Correction:**
  1. Specify platform-tuned acceleration transfer functions in the client or input core to linearize raw touch deltas before passing them to the OS.
  2. On macOS, consider providing an option to query the display layout via `CGGetActiveDisplayList` to ensure seamless cursor tracking across disparate display bounds.

---

### Finding MR-05: Wayland Compositor Coverage Beyond GNOME/KDE (wlroots / Hyprland)

- **Severity:** `Medium`
- **Specification Section:** §25 ("Test at minimum: GNOME Wayland, KDE Plasma Wayland... ashpd, reis")
- **Technical Justification:**
  SPEC 25 restricts its target to compositors supporting the XDG RemoteDesktop portal + EIS (`libei`). While GNOME 44+ and KDE Plasma 6+ support this, popular tiling Wayland compositors (Hyprland, Sway, Wayfire) based on wlroots do not implement the XDG RemoteDesktop portal with EIS. They rely on standard Wayland protocols (`wlr-virtual-pointer-unstable-v1` and `virtual-keyboard-unstable-v1`). Users on Sway/Hyprland will experience total input failure.
- **Recommended Correction:**
  Clarify in the platform support matrix that wlroots-based compositors are explicitly unsupported in v1, or provide an optional Linux backend extension utilizing the `/dev/uinput` subsystem (requiring a standard `udev` rule) as a universal Linux fallback.

---

## 5. Unnecessary Complexity

### Finding UC-01: Neutral Corner Geometric Boundary vs Axis Lock State Machine

- **Severity:** `Medium`
- **Specification Section:** §6.2, §9.2 ("The bottom/side intersection is a small neutral corner and should not trigger either scrolling axis")
- **Technical Justification:**
  Attempting to carve out a physical "neutral corner" in CSS geometry creates dead zones where touch events fail to register or cause jittery transitions when dragging across boundaries. Standard mobile gesture engines do not use spatial exclusion zones; they use **directional axis locking**.
- **Recommended Correction:**
  Eliminate the physical neutral corner. Instead, implement touch-slop and axis locking in the gesture state machine: when a touch begins, measure the initial displacement vector $(\Delta x, \Delta y)$. If $|\Delta x| > |\Delta y| + \text{threshold}$, lock to the horizontal axis; if $|\Delta y| > |\Delta x| + \text{threshold}$, lock to the vertical axis.

---

### Finding UC-02: Dual Drag Trigger Mechanisms (Long-Press vs Double-Tap-and-Drag)

- **Severity:** `Medium`
- **Specification Section:** §7.2, §28 ("Support both: Long-press then move. Double-tap-and-drag behavior. These should be independently configurable")
- **Technical Justification:**
  Supporting both long-press-to-drag and double-tap-to-drag concurrently within a web touch surface adds significant timer latency and ambiguous gesture states. Specifically, double-tap detection requires delaying single-tap click dispatch by 200–300ms to verify if a second tap occurs. This introduces noticeable perceived latency on every standard click.
- **Recommended Correction:**
  Default to an immediate-response tap-to-click model with dedicated on-screen mouse buttons (Left/Right) for complex drag-and-drop operations. Make double-tap drag an opt-in setting with an explicit warning that enabling it adds tap latency.

---

### Finding UC-03: Excessive Workspace Crate Granularity

- **Severity:** `Low`
- **Specification Section:** §31 (Defining 10 separate crates: `agent-core`, `network`, `discovery`, `pairing`, `protocol`, `input-core`, `input-windows`, `input-macos`, `input-x11`, `input-wayland`)
- **Technical Justification:**
  For an application with a lean scope, maintaining 10 independent Rust crates creates high compilation overhead, boilerplate traits, and complex cyclic dependency hazards during early development.
- **Recommended Correction:**
  Consolidate into 5 logical crates:
  1. `tether-protocol` (Protobuf definitions and generated Prost types)
  2. `tether-input` (Input core abstraction + platform backends isolated via `cfg(target_os)`)
  3. `tether-server` (Axum HTTP/WS, pairing, discovery)
  4. `tether-core` (Orchestration, session state machine, config)
  5. `tether-desktop` (Tauri 2 desktop shell, tray, UI)

---

## 6. Security and Reliability Concerns

### Finding SR-01: Cross-Site WebSocket Hijacking (CSWSH) and DNS Rebinding

- **Severity:** `High`
- **Specification Section:** §19.6 ("Validate the WebSocket Origin... Reject cross-origin browser requests by default")
- **Technical Justification:**
  While SPEC 19.6 correctly mentions validating the `Origin` header, standard browser behavior must be carefully guarded:
  1. If an attacker on the same network performs a DNS rebinding attack (mapping a malicious domain to `192.168.1.x`), the browser sends `Origin: http://attacker.com` and `Host: attacker.com`.
  2. If a local user opens a malicious website on their mobile phone, the website can attempt WebSocket connections to `ws://192.168.1.x:port`.
- **Recommended Correction:**
  In Axum, enforce strict dual validation:
  1. Validate the `Host` header against the machine's actual bound IP addresses and `.local` hostname.
  2. Validate the `Origin` header to match the server's own bound origin.
  3. Require the pairing/session token to be passed inside the initial Protobuf `ClientHello` payload rather than relying solely on HTTP cookies. This guarantees that a malicious web page executing a cross-origin WebSocket connection cannot hijack ambient credentials.

---

### Finding SR-02: Input Message Queue Flooding and DoS

- **Severity:** `Medium`
- **Specification Section:** §11.3, §15.5 ("Never allow unbounded input queues... Maximum sizes")
- **Technical Justification:**
  A compromised or buggy mobile client could stream thousands of synthetic motion or key events per second over the WebSocket, overwhelming the Tokio channel and exhausting OS input queues or causing high CPU consumption.
- **Recommended Correction:**
  Enforce a hard rate limiter on the Axum WebSocket stream:
  - Drop or coalesce motion deltas exceeding 120 packets/sec.
  - Rate-limit text insertion messages to a realistic maximum (e.g., 50 messages/sec or bounded buffer size of 4KB per message).
  - Immediately close sessions that violate protocol frame limits.

---

## 7. Comprehensive Platform Compatibility Matrix

The table below summarizes verified platform feasibility and constraints:

| Platform / Subsystem | Target API / Mechanism | Current Feasibility | Primary Constraint / Requirement |
| --- | --- | :---: | --- |
| **Mobile Safari (iOS)** | Pointer Events, WebSocket | **Verified** | `navigator.wakeLock` blocked on HTTP (requires video loop); edge swipes require touch margin. |
| **Mobile Chrome (Android)** | Pointer Events, WebSocket | **Verified** | `.local` mDNS resolution unreliable (requires IP fallback in QR); `interactive-widget` required for keyboard. |
| **Windows Desktop** | Win32 `SendInput` | **Verified** | UIPI blocks elevated windows; emojis require UTF-16 surrogate pairs in `wScan`. |
| **macOS Desktop** | CoreGraphics `CGEventPost` | **Verified** | Requires Accessibility TCC permissions; relative pointer acceleration requires tuning. |
| **Linux X11** | `x11rb` / XTEST | **Conditional** | No smooth scrolling (discrete buttons 4–7 only); arbitrary Unicode requires clipboard injection. |
| **Linux Wayland (GNOME/KDE)** | XDG RemoteDesktop + `libei` | **Conditional** | GNOME 44+ / KDE 6+ only; `libei` lacks Unicode string injection; prompt on session start. |
| **Linux Wayland (wlroots)** | Wayland protocols | **Unsupported** | Lacks XDG RemoteDesktop + EIS support; requires `/dev/uinput` fallback. |

---

## 8. Specific Changes to Improve the Final Implementation

To ensure a robust, production-ready implementation, adopt the following targeted changes:

1. **Protocol Architecture:**
   - Require the persistent device session token in the Protobuf `ClientHello` rather than relying solely on browser cookies.
   - Implement an application-layer encrypted envelope (ChaCha20-Poly1305) initialized via the QR pairing secret to guarantee input privacy on unencrypted LANs.
2. **Mobile Web Client:**
   - Add a continuous video-loop keepalive element to prevent mobile screen sleep over plain HTTP.
   - Implement an inset touch margin (minimum 20px) to prevent iOS/Android edge-swipe navigation conflicts.
   - Use an off-screen proxy input element with synchronous click focus to reliably trigger mobile software keyboards.
3. **Desktop Native Backends:**
   - **Windows:** Implement UTF-16 surrogate pair splitting in `SendInput` for code points > `U+FFFF`.
   - **macOS:** Add startup validation for `AXIsProcessTrusted()` with direct deep-linking to macOS Accessibility settings.
   - **Linux X11:** Implement an integer accumulator for discrete Wheel Buttons (4–7) and use clipboard synthesis for arbitrary Unicode text.
   - **Linux Wayland:** Restrict direct typing to XKB keycodes and document clipboard fallback for complex Unicode strings.
4. **Networking & Discovery:**
   - Format the QR code with the explicit IPv4 address (`http://192.168.x.x:PORT/?t=...`) as primary, with mDNS `.local` as secondary.
   - Establish a default port (e.g., `49200`) with deterministic incremental fallback.

---

## 9. Conclusion

The Tether specification (`SPEC.md`) provides an exceptionally structured and thorough foundation for a local-only remote input application. Its emphasis on safety (`release_all`), production-first design, and dependency discipline is exemplary.

Incorporating the corrections detailed in this review—particularly regarding HTTP Wake Lock alternatives, X11/Wayland input limitations, mobile edge-gesture margins, and LAN transport hardening—will eliminate critical runtime failure modes and ensure a dependable, high-performance product.
