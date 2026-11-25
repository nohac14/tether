# Mobile text entry

Live typing uses its own bounded textarea buffer. It emits semantic text and
edit operations through the ordered input queue. The local buffer represents
recent phone edits and carries no authority over the desktop application's
selection or text. Each control epoch binds a fresh editor instance to its
ordered queue. Blur clears the live buffer. A local draft can survive blur
within the same epoch. Ownership loss, page hide, or disposal clears it.

The editable must exist before the Keyboard button is used. A tap calls
`focus()` directly in the gesture stack, checks the active element, and leaves
the keyboard closed on failure. A visual viewport change recomputes the touch
surface through the existing viewport adapter. Live typing requests that the
browser disable capitalization, autocomplete, autocorrect and spellchecking.

A live append must be an observed append to the prior local value and fit the
negotiated UTF-8 input budget. Backward character and word deletions map to
the number of confirmed trailing printable ASCII characters removed, up to
64 per event. The editor accepts a trailing range selected by the keyboard
when it exactly matches the removal. For an input-only grouped deletion it
checks the surviving prefix and the final caret. The queue accounts for each
repeat against the negotiated batch limit. When the live buffer is empty, a
local zero-width marker gives the keyboard a deletion target. Canceling a
backward edit or observing the marker disappear sends one Backspace. The
marker stays on the phone. Erasing a local draft resumes live typing for the
next edit, and Resume live typing explicitly discards a draft at any point.

While composing, the buffer emits no action. At composition end, the confirmed
DOM value must equal the baseline plus the final composition text. If the DOM
has yet to commit, a matching follow-up input can finish it exactly once.
Cancellation and late input after loss of control cannot publish composition
text. The engine rejects isolated surrogate code units. Replacements, selection
edits, and deletions with ambiguous grapheme behavior move to local drafting
without emitting a desktop edit. Local drafting currently keeps the textarea
editable and pauses live sending. MW-008 owns the transition from that draft to
explicit confirmation and chunked sending.

Tests exercise the live buffer and the generated operation ordering. The
[Windows browser checkpoint](../manual-tests/2026-11-21-mobile-text-entry.md)
records headless focus and composition evidence. The software-keyboard
and viewport checks on physical iOS and Android remain open under R-005.
