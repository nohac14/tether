# Mobile touchpad gestures

The touchpad tracks individual pointer IDs and their starting and most recent
positions. Only contacts that began on the pad can join its gesture. The browser
adapter captures each accepted pointer to its starting region. Moving across a
strip or the neutral corner keeps the original region. The side setting moves the
vertical region and neutral corner together and resets active gestures first.

| Decision | Provisional threshold |
| --- | --- |
| Tap duration and movement | Up within 200 ms and below 8 CSS px from the start |
| Second or third finger joins | Within 100 ms of the first down, before motion or a lift |
| Long-press drag | First movement crossing 8 CSS px after holding for at least 350 ms |
| Double-tap drag | Second down within 24 CSS px and motion crosses 8 CSS px before the first tap's 250 ms deadline |
| Malformed or anomalous movement | Cancel on invalid/backward samples or one step over 128 CSS px |

A tap candidate becomes a motion gesture when it crosses the movement threshold
before a drag qualifies. That gesture stays motion-only until it ends. Drag
starts with a left-button down followed by motion. It ends with exactly one
left-button up. A stationary long press never holds the button. A second finger
during motion or drag rejects the mixed gesture and releases a held button.
Two- and three-finger taps emit right and middle clicks after every contact
lifts. Motion, a late finger, or a fourth contact cancels their click candidate.

Long-press drag defaults to on. Double-tap drag defaults to off. Each qualifying
tap immediately emits a full click at release in the default mode. A pair of
quick clicks uses the desktop's ordinary double-click timing. The opt-in mode
buffers one tap for up to 250 ms of recognizer time, with delivery on the first
animation frame at or after the deadline. A qualifying second tap that ends
before the deadline emits one double click. Starting the drag before the deadline
consumes the saved tap. Starting outside the double-tap radius or on a scroll
strip commits the older click before that other gesture. The settings disclose
the recognizer delay. Physical interaction latency and operating-system double
click recognition remain downstream validation.

Pointer cancellation or unexpected capture loss, hidden page, page hide, blur,
orientation and viewport changes, a frame gap over 100 ms, settings replacement,
and control loss discard pending gestures and release any drag. Pointer capture
failure cancels the surface. There are at most five captured pointers across the
surface and three tracked fingers on the pad. Every controller replacement starts
neutral. The session owns authoritative release on transport loss.

The shell requires both controlling status and a current input sink before it
attaches listeners. The application still launches disconnected. Input settings
are a separate version-one browser-origin record, bounded to 1,024 characters.
