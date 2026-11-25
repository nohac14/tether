# Application shell

The shell mounts the touch surface after its session reports `controlling` and
supplies an input sink. Leaving control detaches pointer capture and discards
gesture history. The network layer owns authoritative release on connection loss.
