# Canonical input mapping and backend conformance contract

**Task:** NB-001
**Status:** Implemented platform-neutral contract and inert suite
**Authority:** `SPEC.md` sections 14, 15, and 22–26; `input-backend.md`

This contract fixes the canonical input identities and the test obligations
shared by every native backend. It does not select or validate a native API.
Windows, macOS, X11, and Wayland feasibility, native event translation, and
physical-device evidence remain in their platform tasks.

## Canonical physical keys

Protocol version one accepts only USB HID Keyboard/Keypad usage page `0x07` and
the explicit ranges below:

| Inclusive usages | Class | Deliberate boundary |
|---|---|---|
| `0x04..=0x65` | Standard keyboard | Excludes error usages `0x01..=0x03` and Keyboard Power `0x66` |
| `0x67..=0xA4` | Extended keyboard | Stops before reserved `0xA5..=0xAF` |
| `0xB0..=0xDD` | Extended keypad | Stops before reserved `0xDE..=0xDF` |
| `0xE0..=0xE7` | Modifiers | Excludes values beyond Right GUI |

The ranges are one shared source for domain validation and conformance tests.
Reserved, error, consumer-page, vendor-defined, and wider values fail before
dispatch. A native backend maps each advertised usage explicitly or declines
physical-key capability; it never guesses from a browser label, Unicode scalar,
or keycap legend.

The version-one list was established against HID Usage Tables 1.5 as specified
by the protocol contract and rechecked on 2026-10-12 against the current
[USB-IF HID Usage Tables 1.7](https://usb.org/document-library/hid-usage-tables-17).
USB-IF explicitly notes that defining a usage does not imply host support. New
table revisions therefore do not silently expand Tether's wire allowlist.

## Logical command table

Logical commands translate to deterministic physical-key chords in press order;
release order is the exact reverse. These are reviewed conventional plans, not
claims that every focused application accepts them:

| Command | Windows | macOS | Linux desktop/application convention |
|---|---|---|---|
| Copy/Cut/Paste | Ctrl+C/X/V | Command+C/X/V | Ctrl+C/X/V |
| Undo | Ctrl+Z | Command+Z | Ctrl+Z |
| Redo | Ctrl+Y | Command+Shift+Z | Ctrl+Shift+Z |
| Select all/Find/Save/Close tab | Ctrl+A/F/S/W | Command+A/F/S/W | Ctrl+A/F/S/W |
| Switch application | Alt+Tab | Command+Tab | Alt+Tab |
| Task manager/platform equivalent | Ctrl+Shift+Escape | Option+Command+Escape | Unsupported: no desktop-independent equivalent |

The Windows mappings were checked against current
[Microsoft keyboard-shortcut guidance](https://support.microsoft.com/en-us/windows/keyboard-shortcuts-in-windows-dcc61a57-8ff0-cffe-9796-cb9706c75eec)
and the documented
[Task Manager chord](https://learn.microsoft.com/en-us/windows-hardware/customize/desktop/unattend/microsoft-windows-embedded-keyboardfilterservice-shiftcontrolescape).
The macOS mappings were checked against current
[Apple keyboard-shortcut guidance](https://support.apple.com/en-us/102650).
Linux application shortcuts are conventions rather than a cross-desktop OS
guarantee. Each Linux backend must validate and advertise only mappings that
work in its named supported configuration.

Paste means invoking the conventional paste shortcut. It does not permit Tether
to read, replace, inspect, or log clipboard contents. Custom shortcut decks
remain reviewed physical-key transitions and cannot encode commands or scripts.
Media commands stay separate because their native representation is platform
specific and frequently uses consumer controls rather than keyboard-page HID.

## Capability preflight

`BackendStatus::validate_action` and `execute_checked` provide the shared final
preflight immediately before backend execution. They reject when:

- the backend is uninitialized, unavailable, or faulted;
- required permission is not currently granted/not-required; or
- the action-specific capability is absent.

Button transitions require the named button. Atomic clicks additionally require
atomic-click capability. Scroll axes are independent. Text insertion requires
at least one active text mode. Version-one text-edit commands require physical
key capability. Logical and media actions require their exact advertised enum.
The rejection carries only a closed safe category.

This cached check cannot close the race with native permission/device loss. The
backend must still validate platform preconditions, stop admission on uncertain
state changes, and trigger emergency release through the dispatcher.

## Shared conformance suite

The `backend-conformance` Cargo feature exposes `run_backend_conformance`.
Platform crates must run it against their production mapper and held-state
implementation wired to an inert event sink. Default automated tests must not
inject workstation input. Guarded native injection remains a separate explicit
platform gate.

For every capability snapshot, the suite verifies:

1. backend kind and status/capability consistency;
2. unsupported representative actions stop before `InputBackend::execute`;
3. supported pointer, both scroll axes, text/edit, logical, media, button, and
   atomic-click paths execute through the same mapping boundary;
4. every allowlisted HID usage either executes a balanced down/up or returns an
   explicit `Unsupported` mapping result when physical keys are generally
   available; the common baseline key used for release testing must work;
5. every advertised button executes ordinary down/up;
6. held key/button state is neutralized by `release_all`; and
7. a repeated release is confirmed neutral and idempotent.

A platform test may advertise a conservative subset, but it must not omit a
mapping from the suite while advertising that mapping in runtime capabilities.
Capability/status changes and partial native failures require additional
platform-specific tests.

## Current evidence and limits

The suite passes against `MockInputBackend`, including more than 400 canonical
action checks, unsupported-action non-execution, held-state cleanup, and repeated
release. Mapping boundary, exhaustiveness, reverse release order, and redacted
chord diagnostics have unit coverage.

No Windows, macOS, X11, or Wayland native mapper exists yet, and no native input
or physical-device test was run for NB-001. Passing this shared suite is a
prerequisite, not evidence of platform support.
