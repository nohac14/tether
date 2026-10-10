# Development Instructions

## Authority

- `SPEC.md` is the authoritative product and engineering specification. Read it
  completely before architectural work or implementation that spans components.
- Follow accepted decisions in `docs/architecture-decisions.md`, then applicable
  component documentation and the dependency gates in
  `docs/implementation-plan.md`.
- Track uncertain platform behavior in `docs/open-risks.md`. Never convert an
  unvalidated risk into a support claim.
- Do not silently deviate from `SPEC.md`. A material conflict or architecture
  change requires an ADR, synchronized documentation, and owner approval.
- Do not reopen accepted D-01 through D-05 without new verified technical
  evidence.

## Product boundaries

- Build the complete production architecture, not a disposable MVP.
- Keep the desktop as the local HTTP/WebSocket host. Do not add a cloud service,
  relay, account system, runtime internet dependency, or required native mobile
  application.
- Keep the Tauri shell thin. Business logic belongs in testable Rust components.
- Keep every platform-specific input implementation behind `InputBackend`.
- Keep UI, gesture/text engines, protocol, transport, pairing, session safety,
  persistence, discovery, and native backends separate at meaningful boundaries.
- Use one authoritative Protocol Buffers schema and reproducible generation.
- Preserve the dedicated vertical and horizontal scroll strips and neutral corner.
- Do not add elevation, `uiAccess`, hidden clipboard mutation, global X11 keymap
  mutation, or ad hoc WebSocket payload encryption.

## Input safety

- Route all input through the ordered dispatch pipeline. Flush pending motion and
  scroll before state-changing actions.
- Maintain one active controller and explicit, atomic ownership transitions.
- Every observable disconnect, timeout, overflow, revocation, transfer, suspend,
  lock, shutdown, and fatal backend path must invoke idempotent release-all.
- Reserve an emergency cleanup path that cannot be starved by the normal queue.
- Add best-effort panic/drop/startup cleanup, but do not claim cleanup after
  uncatchable process or OS failure.
- Never restore held keys or buttons after reconnect, resume, or ownership change.

## Security and privacy

- Authenticate and authorize before dispatch. Enforce approved-interface binding,
  on-link peer admission, strict Host/Origin allowlists, and host-scoped cookies.
- Bound every body, header, frame, message, string, connection count, approval
  count, queue, retry loop, deadline, and input rate.
- Never log typed text, input key/button identities or streams, clipboard data,
  credentials, pairing tokens, cookies, raw control frames, or sensitive bodies.
- Give sensitive types redacted diagnostics; do not derive payload-revealing
  `Debug` output.
- Keep device/settings/log administration on narrow Tauri IPC, not LAN HTTP.
- Reject unsupported capabilities honestly; never simulate success.

## Workflow and verification

- Work only on a task whose dependencies are complete in
  `docs/implementation-plan.md`; coordinate changes to schemas, core interfaces,
  manifests, generation, and shared build configuration.
- Implement production-quality code and tests together. Include failure,
  saturation, malformed-input, cancellation, and release-state tests.
- Prefer `MockInputBackend` for automation. Native injection tests are opt-in,
  isolated, and must warn that they manipulate the workstation.
- Run applicable formatting, lint/static analysis, unit tests, integration tests,
  generated-code checks, browser tests, builds, and package checks before marking
  a task done.
- Never suppress, weaken, skip, or reclassify a failure to make a gate pass.
- Do not claim native, signed-package, browser, or real-device validation unless
  it actually ran; record not-run coverage explicitly.
- Preserve lockfiles, pinned toolchains, generated-source reproducibility, user
  changes, and unrelated worktree state.
- Update component docs, support matrices, manual evidence, decisions, and open
  risks with every behavior or architecture change.
