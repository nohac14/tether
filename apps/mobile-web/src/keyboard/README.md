# Keyboard engine

The persistent textarea uses `LiveEditBuffer` to map confirmed suffix appends
and unambiguous backspace edits to the ordered input queue. See the
[text-entry contract](../../../../docs/components/mobile-text-entry.md) for
browser events, ownership cleanup, and the Compose and Send handoff.
