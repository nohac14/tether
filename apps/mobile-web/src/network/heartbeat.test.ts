import { create, fromBinary, toBinary } from "@bufbuild/protobuf";
import { describe, expect, it } from "vitest";
import {
  HeartbeatPingSchema,
  HeartbeatPongSchema,
} from "../protocol/generated/tether/control/v1/control_pb";
import { ApplicationHeartbeat } from "./heartbeat";

const defaultPolicy = { heartbeatIntervalMillis: 1_000, heartbeatTimeoutMillis: 4_000 };

function setup(policy = defaultPolicy) {
  let now = 0;
  let nextTimer = 0;
  const scheduled = new Map<number, { task: () => void; time: number }>();
  const sent: bigint[] = [];
  const stopped: string[] = [];
  const transport = { blocked: false, failed: false };
  const heartbeat = new ApplicationHeartbeat(
    {
      now: () => now,
      schedule(task, delay) {
        nextTimer += 1;
        scheduled.set(nextTimer, { task, time: now + delay });
        return nextTimer;
      },
      cancel(id) {
        scheduled.delete(id);
      },
    },
    {
      send(ping) {
        if (transport.failed) throw new Error("Socket closed");
        if (transport.blocked) return false;
        sent.push(
          fromBinary(HeartbeatPingSchema, toBinary(HeartbeatPingSchema, ping)).heartbeatIdentifier,
        );
        return true;
      },
      stop(reason) {
        stopped.push(reason);
      },
    },
    policy,
  );
  const advance = (time: number) => {
    now = time;
    for (const [id, timer] of [...scheduled]) {
      if (timer.time <= now) {
        scheduled.delete(id);
        timer.task();
      }
    }
  };
  const pong = (identifier: bigint, time: number) => {
    now = time;
    return heartbeat.acceptPong(create(HeartbeatPongSchema, { heartbeatIdentifier: identifier }));
  };
  return { heartbeat, transport, advance, pong, sent, stopped, scheduled };
}

describe("browser application heartbeat", () => {
  it("uses increasing protobuf identifiers and estimates local RTT from correlated pongs", () => {
    const session = setup();
    session.advance(999);
    expect(session.sent).toEqual([]);
    session.advance(1_000);
    expect(session.pong(1n, 1_047)).toBe(47);
    expect(session.pong(1n, 1_250)).toBeUndefined();
    session.advance(2_000);
    session.advance(3_000);
    expect(session.sent).toEqual([1n, 2n, 3n]);
    expect(session.pong(3n, 3_020)).toBe(20);
    expect(session.pong(2n, 3_025)).toBeUndefined();
    expect(session.scheduled.size).toBe(1);
  });

  it("does not send a catch-up burst after a delayed callback", () => {
    const session = setup();
    session.advance(2_700);
    session.advance(3_000);
    expect(session.sent).toEqual([1n]);
    expect(session.pong(1n, 3_650)).toBe(950);
    session.advance(3_700);
    expect(session.sent).toEqual([1n, 2n]);
    expect(session.stopped).toEqual([]);
  });

  it("times out after event-loop suspension before emitting another ping", () => {
    const session = setup();
    session.advance(1_000);
    session.advance(4_000);
    expect(session.sent).toEqual([1n]);
    expect(session.stopped).toEqual(["timeout"]);
    expect(session.pong(1n, 4_001)).toBeUndefined();
    expect(session.scheduled.size).toBe(0);
  });

  it("ignores unknown identifiers and gives the deadline priority over a pong at equality", () => {
    const session = setup();
    session.advance(1_000);
    expect(session.pong(42n, 3_999)).toBeUndefined();
    expect(session.pong(1n, 4_000)).toBeUndefined();
    expect(session.sent).toEqual([1n]);
    expect(session.stopped).toEqual(["timeout"]);
  });

  it("starts reconnect with a fresh timer and no pending pongs", () => {
    const old = setup();
    old.advance(1_000);
    old.heartbeat.dispose();
    const current = setup();
    expect(current.pong(1n, 15)).toBeUndefined();
    current.advance(1_000);
    expect(current.pong(1n, 1_025)).toBe(25);
    expect(old.pong(1n, 1_025)).toBeUndefined();
    expect(old.scheduled.size).toBe(0);
    expect(old.stopped).toEqual([]);
  });

  it.each(["blocked", "failed"] as const)("closes a %s writer exactly once", (fault) => {
    const session = setup();
    session.transport[fault] = true;
    session.advance(1_000);
    session.advance(8_000);
    expect(session.sent).toEqual([]);
    expect(session.stopped).toEqual(["transport"]);
    expect(session.scheduled.size).toBe(0);
  });

  it("supports negotiated lower intervals and rejects invalid policies and clocks", () => {
    const fast = setup({ heartbeatIntervalMillis: 250, heartbeatTimeoutMillis: 1_000 });
    fast.advance(250);
    fast.advance(500);
    expect(fast.sent).toEqual([1n, 2n]);
    expect(() => setup({ heartbeatIntervalMillis: 1_000, heartbeatTimeoutMillis: 1_999 })).toThrow(
      RangeError,
    );
    expect(fast.pong(1n, 499)).toBeUndefined();
    expect(fast.stopped).toEqual(["clock"]);
    const overflow = setup();
    overflow.advance(Number.MAX_SAFE_INTEGER);
    expect(overflow.stopped).toEqual(["clock"]);
  });
});
