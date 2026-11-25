import { create } from "@bufbuild/protobuf";
import {
  type HeartbeatPing,
  HeartbeatPingSchema,
  type HeartbeatPong,
  type ServerHello,
} from "../protocol/generated/tether/control/v1/control_pb";

type HeartbeatPolicy = Pick<ServerHello, "heartbeatIntervalMillis" | "heartbeatTimeoutMillis">;
type StopReason = "timeout" | "transport" | "clock" | "counter-exhausted";

export interface HeartbeatClock {
  now(): number;
  schedule(task: () => void, delayMillis: number): number;
  cancel(timer: number): void;
}

interface HeartbeatTransport {
  send(ping: HeartbeatPing): boolean;
  stop(reason: StopReason): void;
}

const MAX_IDENTIFIER = (1n << 64n) - 1n;
const MAX_OUTSTANDING = 16;
const MAX_TIME = Number.MAX_SAFE_INTEGER - 4_000;

/** One scheduler per negotiated connection. The shared writer owns envelope counters. */
export class ApplicationHeartbeat {
  private readonly interval: number;
  private readonly timeout: number;
  private readonly pending: { identifier: bigint; sentAt: number }[] = [];
  private identifier = 1n;
  private timer: number | undefined;
  private active = true;
  private lastObserved: number;
  private lastPong: number;
  private nextPing: number;

  constructor(
    private readonly clock: HeartbeatClock,
    private readonly transport: HeartbeatTransport,
    policy: HeartbeatPolicy,
  ) {
    this.interval = policy.heartbeatIntervalMillis;
    this.timeout = policy.heartbeatTimeoutMillis;
    if (
      !Number.isInteger(this.interval) ||
      this.interval < 250 ||
      this.interval > 1_000 ||
      !Number.isInteger(this.timeout) ||
      this.timeout < 1_000 ||
      this.timeout > 4_000 ||
      this.timeout < this.interval * 2
    )
      throw new RangeError("Invalid heartbeat policy");
    const now = clock.now();
    if (!Number.isFinite(now) || now < 0 || now > MAX_TIME)
      throw new RangeError("Invalid heartbeat clock");
    this.lastObserved = now;
    this.lastPong = now;
    this.nextPing = now + this.interval;
    this.arm(now);
  }

  /** Returns local RTT only for a timely pong that matches a sent identifier. */
  acceptPong(pong: HeartbeatPong): number | undefined {
    const now = this.checkTime();
    if (now === undefined) return;
    const index = this.pending.findIndex(
      ({ identifier }) => identifier === pong.heartbeatIdentifier,
    );
    const sample = this.pending[index];
    if (!sample || now - sample.sentAt >= this.timeout) return;
    this.pending.splice(0, index + 1);
    this.lastPong = now;
    return now - sample.sentAt;
  }

  dispose(): void {
    this.active = false;
    this.pending.length = 0;
    if (this.timer !== undefined) this.clock.cancel(this.timer);
    this.timer = undefined;
  }

  private arm(now: number): void {
    if (!this.active) return;
    const due = Math.min(this.nextPing, this.lastPong + this.timeout);
    this.timer = this.clock.schedule(
      () => {
        this.tick();
      },
      Math.max(0, Math.ceil(due - now)),
    );
  }

  private tick(): void {
    this.timer = undefined;
    const now = this.checkTime();
    if (now === undefined) return;
    if (now >= this.nextPing) {
      const oldest = this.pending.findIndex(({ sentAt }) => now - sentAt < this.timeout);
      this.pending.splice(0, oldest < 0 ? this.pending.length : oldest);
      if (this.identifier > MAX_IDENTIFIER || this.pending.length >= MAX_OUTSTANDING) {
        this.fail("counter-exhausted");
        return;
      }
      const identifier = this.identifier;
      this.identifier += 1n;
      this.pending.push({ identifier, sentAt: now });
      this.nextPing = now + this.interval;
      let sent = false;
      try {
        sent = this.transport.send(
          create(HeartbeatPingSchema, { heartbeatIdentifier: identifier }),
        );
      } catch {
        // Socket state may change before the write.
      }
      if (!sent) {
        this.fail("transport");
        return;
      }
    }
    this.arm(now);
  }

  private checkTime(): number | undefined {
    if (!this.active) return undefined;
    const now = this.clock.now();
    if (!Number.isFinite(now) || now < this.lastObserved || now > MAX_TIME) {
      this.fail("clock");
      return undefined;
    }
    this.lastObserved = now;
    if (now - this.lastPong >= this.timeout) {
      this.fail("timeout");
      return undefined;
    }
    return now;
  }

  private fail(reason: StopReason): void {
    if (!this.active) return;
    this.dispose();
    this.transport.stop(reason);
  }
}
