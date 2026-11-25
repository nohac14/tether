# Mobile interface

The mobile interface builds into the desktop's embedded Vite bundle. The
initial screen displays a disconnected status, the touchpad, independent
vertical and horizontal scroll strips, a visibly neutral intersection,
and a keyboard control. Clicking uses touchpad gestures. The touchpad
and vertical scroll strip extend to the horizontal strip at the bottom
of the usable viewport.
Both strips use the same text arrow, with a quarter turn for the vertical
strip. Active touch surfaces dim slightly in both themes.
The input controls stay disabled until the authenticated session layer
provides real capability and ownership state.

The touch surface is a two-column and two-row grid. Switching the vertical
strip side also switches the neutral corner. The horizontal strip occupies
the touchpad column, and the corner carries no interaction. Insets come
from the system safe areas plus a provisional 12–24 CSS pixel gutter on
both edges. Scroll targets and controls start at 44 CSS pixels. The surface
uses remaining height and can scroll within an unusually small viewport to
preserve access to the controls.

The shell samples `visualViewport` size and offsets as the browser changes
the visible area. It coalesces resize and scroll updates to one animation
frame, uses the layout viewport when the visual API is unavailable, and
refreshes after a page is restored. It uses CSSOM properties under the
same stylesheet CSP and external assets as the static server.

Appearance preferences are a bounded version-one record local to the
browser origin. The two supported settings are vertical strip side and
system, light or dark theme. Unsupported or malformed stored versions
fall back to defaults while leaving the stored record intact. Storage
failure keeps settings usable for the open page. Separate input settings control
both drag methods and each strip's direction, sensitivity, and momentum.
Changes to scroll sides or gesture settings reset the active surface. The
[touchpad](mobile-gesture-engine.md), [scroll strips](scroll-engine.md), and
[ordered client queue](pointer-processing.md) define how input flows once a
session owns control.

The Keyboard button synchronously focuses the persistent, accessible live
textarea when the active controller has a text or backspace capability.
The [text-entry contract](mobile-text-entry.md) covers composition, local
drafting, and ownership cleanup.

The [browser connection contract](mobile-connection.md) covers connection
status, reconnect, and the Take control action for standby devices.

See the [HTTP and mobile-shell checkpoint](../manual-tests/2026-11-20-http-and-mobile-shell.md)
for the Windows and browser evidence. Physical Safari and Android testing
must establish suitable device-specific edge gutters and viewport behavior.
