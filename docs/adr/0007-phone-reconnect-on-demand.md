# ADR 0007 — Phone reconnect on demand

On a fresh phone page launch, Tether tries the saved pairing once. After a
disconnect or a failed attempt, the user taps **Reconnect** to start a new
connection. A suspended page offers the button when the user returns. The
owner requested this behavior after testing a Windows restart with Safari
open and seeing repeated reconnect attempts.

The connection and heartbeat deadlines still apply. Closing a connection
clears local input state and the desktop retires the old session. A Home
Screen bookmark stores the phone page URL. Any saved pairing comes from
the browser's persistent cookie for the same host.
