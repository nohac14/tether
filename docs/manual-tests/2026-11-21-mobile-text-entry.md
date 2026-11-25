# Windows mobile text-entry checkpoint

An independent localhost fixture loaded the mobile shell in headless Edge
158.0.4370.33. It supplied mock control ownership and decoded generated input
batches locally. A trusted DevTools touch gesture focused the persistent
textarea. A forced focus refusal showed the retry message. Live ASCII text,
populated and empty backspaces, and a Unicode composition commit each produced
one operation. Preedit and a simulated dictation replacement produced no
operations. Revoking mock ownership blurred, disabled, and cleared the editor.

The same run checked touch, scroll and release order, plus both scroll-strip
layouts across three viewport sizes and during resize. Twelve live-buffer
tests cover bounded Unicode, invalid UTF-16, deletion, unsupported
replacements, and cancelled, deferred or stale composition events. The
ordered-queue regression decodes both text operation forms and verifies that
motion and scroll ahead of them are flushed. Windows Vitest passed all 204
tests across 17 suites. TypeScript and ESLint checks on the changed browser
code passed.

The x64 release build linked the CRT statically. A copy containing only
`tether-desktop.exe` passed `--verify-mobile-bundle` with System32 on PATH.
The executable still has only its development entry point. This run
verified the embedded assets.

The browser checks use synthetic ownership and an isolated browser profile.
Physical iOS and Android software keyboards, real pairing, authenticated
WebSocket control, and native Windows input remain unverified.

## Held Backspace follow-up

The owner reported live typing on an iPhone paired to the Windows test
build. Holding Backspace could remove several characters in one browser
event, after which the app stopped forwarding edits. The revised live
buffer accepts verified backward ASCII suffix deletions and sends their
exact bounded repeat count. It preserves local handling for selections,
unconfirmed ranges and ambiguous Unicode deletions.

The headless browser fixture simulated backward word and character
deletions after a focused editor received text. It decoded repeat counts
of six and four, then verified that a regular Backspace still sent one
edit. The 215-test browser suite passed, including negotiated batch-limit
coverage. A physical iPhone retest of held Backspace remains pending.

## Accelerated Backspace recovery on 2026-11-25

After the owner reproduced the stall on iPhone, buffer tests exercised a
selected trailing word, a grouped input-only deletion, and subsequent edits.
The Edge fixture decoded single Backspaces for deletion of the empty-field
marker and a subsequent tap. It also verified Unicode composition and live
typing after explicitly discarding a draft. All 217 Vitest tests passed.
The owner subsequently reported that held Backspace appeared to work on their
iPhone. The specific browser event sequence on that device was not captured.
