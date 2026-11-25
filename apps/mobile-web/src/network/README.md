# Network client

`OrderedInput` batches semantic actions and owns the animation-frame motion and
scroll queue. The connection writer owns envelope counters, exact encoded byte
checks, and the reserved connection-abort hook. See the
[ordered mobile input contract](../../../../docs/components/pointer-processing.md).

`ApplicationHeartbeat` belongs to a single negotiated connection. It schedules
Protobuf pings and computes RTT from matched pongs. The shared writer owns
envelope sequencing and closes the connection on heartbeat failure.
