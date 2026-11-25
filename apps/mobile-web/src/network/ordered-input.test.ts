import { create, fromBinary } from "@bufbuild/protobuf";
import { describe, expect, it } from "vitest";
import {
  ClientEnvelopeSchema,
  type ClientOperation,
  PhysicalKeyTransitionSchema,
  ProtocolLimitsSchema,
  ReleaseAllSchema,
  RequestControlSchema,
  ScrollAxis,
  TextEditCommand,
  TextInsertionSchema,
  TransitionState,
} from "../protocol/generated/tether/control/v1/control_pb";
import {
  encodeInputBatch,
  type InputStopReason,
  OrderedInput,
  type PointerGain,
} from "./ordered-input";

const defaults = {
  maximumEncodedMessageBytes: 65_536,
  maximumOperationsPerBatch: 256,
  maximumTextOperationBytes: 4_096,
  maximumTextBytesPerBatch: 16_384,
  maximumAbsoluteFixedDelta: 8_388_608,
  maximumAtomicClickCount: 2,
};

function setup(limits: Partial<typeof defaults> = {}, gain?: PointerGain) {
  const packets: Uint8Array[] = [];
  const stopped: InputStopReason[] = [];
  const scheduled = new Map<number, () => void>();
  let now = 0;
  let next = 0;
  const socket = { buffered: 0, closed: false, broken: false };
  const queue = new OrderedInput(
    {
      bufferedAmount: () => socket.buffered,
      writeBatch(batch, maximumBytes) {
        if (socket.broken) throw new Error("Send failed");
        if (socket.closed) return "closed";
        const bytes = encodeInputBatch(
          {
            protocolMajor: 1,
            protocolMinor: 0,
            sequence: BigInt(packets.length + 1),
            sessionEpoch: 2n,
            clientTimestampMicros: BigInt(now * 1_000 + 1),
          },
          batch,
          maximumBytes,
        );
        if (!bytes) return "oversized";
        packets.push(bytes);
        return "sent";
      },
      abortInput(reason) {
        stopped.push(reason);
      },
    },
    {
      now: () => now,
      requestFrame(task) {
        next += 1;
        scheduled.set(next, task);
        return next;
      },
      cancelFrame(id) {
        scheduled.delete(id);
      },
    },
    create(ProtocolLimitsSchema, { ...defaults, ...limits }),
    gain,
  );
  const frame = (time: number) => {
    now = time;
    const callbacks = [...scheduled.values()];
    scheduled.clear();
    for (const callback of callbacks) callback();
  };
  const envelopes = () => packets.map((bytes) => fromBinary(ClientEnvelopeSchema, bytes));
  const operations = () =>
    envelopes().flatMap(({ payload }) => {
      if (payload.case !== "operationBatch") throw new Error("Expected input batch");
      return payload.value.operations.map(({ action }) => action);
    });
  return { queue, frame, packets, socket, stopped, scheduled, envelopes, operations };
}

function values(ops: ClientOperation["action"][]) {
  return ops.map((a) => {
    if (a.case === "scroll") return [a.case, a.value.axis, a.value.delta1024];
    if (a.case === "pointerMove") return [a.case, a.value.deltaX1024, a.value.deltaY1024];
    if (a.case === "mouseButtonTransition" || a.case === "physicalKeyTransition")
      return [a.case, a.value.state];
    if (a.case === "mouseButtonClick") return [a.case, a.value.clickCount];
    return [a.case];
  });
}

describe("ordered input dispatch", () => {
  it("coalesces 10,000 high-rate samples into bounded frame sends", () => {
    const f = setup();
    for (let frame = 1; frame <= 10; frame += 1) {
      for (let i = 0; i < 1_000; i += 1) f.queue.surface({ kind: "move", dx: 0.125, dy: -0.25 });
      expect(f.scheduled.size).toBe(1);
      f.frame(frame * 8);
    }
    expect(f.packets).toHaveLength(10);
    expect(values(f.operations())).toEqual(
      Array.from({ length: 10 }, () => ["pointerMove", 128_000, -256_000]),
    );
    expect(f.stopped).toEqual([]);
  });

  it("flushes compatible runs in order across mouse, keyboard and release barriers", () => {
    const f = setup();
    f.queue.surface({ kind: "move", dx: 1, dy: 0 });
    f.queue.surface({ kind: "scroll", axis: "vertical", delta: -2 });
    f.queue.surface({ kind: "scroll", axis: "horizontal", delta: 1 });
    f.queue.surface({ kind: "scroll", axis: "vertical", delta: 0.5 });
    f.queue.surface({ kind: "click", button: "left", count: 1 });
    f.queue.surface({ kind: "button", button: "left", down: true });
    f.queue.surface({ kind: "move", dx: -2, dy: 3 });
    f.queue.surface({ kind: "button", button: "left", down: false });
    f.queue.surface({ kind: "move", dx: 2, dy: 4 });
    f.queue.barrier({
      case: "physicalKeyTransition",
      value: create(PhysicalKeyTransitionSchema, {
        key: { usagePage: 7, usageId: 4 },
        state: TransitionState.DOWN,
      }),
    });
    f.queue.surface({ kind: "scroll", axis: "vertical", delta: 1 });
    f.queue.barrier({ case: "releaseAll", value: create(ReleaseAllSchema) });
    f.queue.surface({ kind: "click", button: "left", count: 1 });
    expect(values(f.operations())).toEqual([
      ["pointerMove", 1024, 0],
      ["scroll", ScrollAxis.VERTICAL, -2048],
      ["scroll", ScrollAxis.HORIZONTAL, 1024],
      ["scroll", ScrollAxis.VERTICAL, 512],
      ["mouseButtonClick", 1],
      ["mouseButtonTransition", TransitionState.DOWN],
      ["pointerMove", -2048, 3072],
      ["mouseButtonTransition", TransitionState.UP],
      ["pointerMove", 2048, 4096],
      ["physicalKeyTransition", TransitionState.DOWN],
      ["scroll", ScrollAxis.VERTICAL, 1024],
      ["releaseAll"],
    ]);
    expect(f.scheduled.size).toBe(0);
  });

  it("retains fractions per axis between frames and clears them at a click", () => {
    const f = setup();
    for (const time of [8, 16]) {
      f.queue.surface({ kind: "scroll", axis: "horizontal", delta: 0.75 / 1024 });
      f.queue.surface({ kind: "scroll", axis: "vertical", delta: -0.75 / 1024 });
      f.frame(time);
    }
    expect(values(f.operations())).toEqual([
      ["scroll", ScrollAxis.HORIZONTAL, 1],
      ["scroll", ScrollAxis.VERTICAL, -1],
    ]);
    f.queue.surface({ kind: "click", button: "right", count: 1 });
    f.queue.surface({ kind: "scroll", axis: "vertical", delta: -0.75 / 1024 });
    f.frame(24);
    expect(values(f.operations()).at(-1)).toEqual(["mouseButtonClick", 1]);
  });

  it("flushes pointer and scroll before sending confirmed text or a single backspace", () => {
    const f = setup();
    f.queue.surface({ kind: "move", dx: 1, dy: 2 });
    f.queue.keyboard({ kind: "insert", text: "猫" });
    f.queue.surface({ kind: "scroll", axis: "horizontal", delta: 2 });
    f.queue.keyboard({ kind: "delete-backward", repeatCount: 1 });

    const operations = f.operations();
    expect(values(operations)).toEqual([
      ["pointerMove", 1024, 2048],
      ["textInsertion"],
      ["scroll", ScrollAxis.HORIZONTAL, 2048],
      ["textEdit"],
    ]);
    expect(operations[1]?.value).toMatchObject({ text: "猫" });
    expect(operations[3]?.value).toMatchObject({
      command: TextEditCommand.DELETE_BACKWARD,
      repeatCount: 1,
    });
  });

  it("flushes pointer motion and accounts for expanded backspaces at the negotiated limit", () => {
    const f = setup({ maximumOperationsPerBatch: 3 });
    f.queue.surface({ kind: "move", dx: 1, dy: 2 });
    f.queue.keyboard({ kind: "delete-backward", repeatCount: 7 });
    f.queue.keyboard({ kind: "insert", text: "x" });

    expect(f.packets).toHaveLength(5);
    const operations = f.operations();
    expect(operations.map((operation) => operation.case)).toEqual([
      "pointerMove",
      "textEdit",
      "textEdit",
      "textEdit",
      "textInsertion",
    ]);
    expect(
      operations.flatMap((operation) =>
        operation.case === "textEdit" ? [operation.value.repeatCount] : [],
      ),
    ).toEqual([3, 3, 1]);
    expect(f.stopped).toEqual([]);
  });

  it("applies one bounded acceleration gain to both axes and discards excess", () => {
    const f = setup(
      { maximumAbsoluteFixedDelta: 65_536 },
      { sensitivity: 2, acceleration: { threshold: 16, maximum: 2 } },
    );
    f.queue.surface({ kind: "move", dx: 6, dy: -2 });
    f.frame(10);
    f.queue.surface({ kind: "move", dx: 72, dy: 8 });
    f.frame(20);
    f.queue.surface({ kind: "move", dx: Number.NaN, dy: 5 });
    f.queue.surface({ kind: "move", dx: 1, dy: 0 });
    f.frame(30);
    expect(values(f.operations())).toEqual([
      ["pointerMove", 12_288, -4096],
      ["pointerMove", 65_536, 24_576],
      ["pointerMove", 2048, 0],
    ]);
  });

  it("drops a clipped run's prior fraction even after reversing within that frame", () => {
    const f = setup({ maximumAbsoluteFixedDelta: 32 });
    f.queue.surface({ kind: "scroll", axis: "vertical", delta: 0.75 / 1024 });
    f.frame(10);
    f.queue.surface({ kind: "scroll", axis: "vertical", delta: -1e200 });
    f.queue.surface({ kind: "scroll", axis: "vertical", delta: 2 / 1024 });
    f.frame(20);
    expect(values(f.operations())).toEqual([["scroll", ScrollAxis.VERTICAL, -30]]);
  });

  it("splits at actual encoded limits without skipping sequence or order", () => {
    const f = setup({ maximumOperationsPerBatch: 2, maximumEncodedMessageBytes: 22 });
    f.queue.surface({ kind: "move", dx: 3, dy: 4 });
    f.queue.surface({ kind: "scroll", axis: "vertical", delta: 5 });
    f.queue.surface({ kind: "click", button: "left", count: 1 });
    expect(f.packets).toHaveLength(3);
    expect(f.packets.every((bytes) => bytes.length <= 22)).toBe(true);
    expect(f.envelopes().map((e) => e.sequence)).toEqual([1n, 2n, 3n]);
    expect(values(f.operations())).toEqual([
      ["pointerMove", 3072, 4096],
      ["scroll", ScrollAxis.VERTICAL, 5120],
      ["mouseButtonClick", 1],
    ]);
  });

  it("handles a lower atomic count and prevents input after an ownership barrier", () => {
    const f = setup({ maximumAtomicClickCount: 1 });
    f.queue.surface({ kind: "click", button: "left", count: 2 });
    f.queue.barrier({ case: "requestControl", value: create(RequestControlSchema) });
    f.queue.surface({ kind: "move", dx: 2, dy: 2 });
    f.frame(10);
    expect(values(f.operations())).toEqual([
      ["mouseButtonClick", 1],
      ["mouseButtonClick", 1],
      ["requestControl"],
    ]);
  });

  it("enforces text UTF-8 and exact envelope budgets", () => {
    const f = setup({ maximumTextOperationBytes: 4, maximumTextBytesPerBatch: 4 });
    f.queue.barrier({ case: "textInsertion", value: create(TextInsertionSchema, { text: "🐾" }) });
    expect(f.operations()).toHaveLength(1);
    f.queue.barrier({ case: "textInsertion", value: create(TextInsertionSchema, { text: "🐾a" }) });
    expect(f.operations()).toHaveLength(1);
    expect(f.stopped).toEqual(["operation-limit"]);
    const small = setup({ maximumEncodedMessageBytes: 1 });
    small.queue.surface({ kind: "click", button: "left", count: 1 });
    expect(small.stopped).toEqual(["operation-limit"]);
    expect(small.packets).toHaveLength(0);
  });

  it("coalesces temporary congestion and sends only when the buffer recovers", () => {
    const f = setup();
    f.socket.buffered = 32_768;
    f.queue.surface({ kind: "move", dx: 1, dy: 2 });
    f.frame(16);
    f.queue.surface({ kind: "move", dx: 3, dy: 4 });
    f.frame(32);
    expect(f.packets).toHaveLength(0);
    f.socket.buffered = 0;
    f.frame(48);
    expect(values(f.operations())).toEqual([["pointerMove", 4096, 6144]]);
    expect(f.stopped).toEqual([]);
  });

  it.each(["stall", "suspension", "queue", "hard limit", "held release"] as const)(
    "retires through the reserved path on %s",
    (fault) => {
      const f = setup();
      f.queue.surface({ kind: "button", button: "left", down: true });
      f.queue.surface({ kind: "move", dx: 1, dy: 0 });
      switch (fault) {
        case "stall":
          f.socket.buffered = 32_768;
          f.frame(16);
          f.frame(80);
          break;
        case "suspension":
          f.frame(120);
          break;
        case "queue":
          for (let i = 0; i < 33; i += 1) {
            f.queue.surface({ kind: "scroll", axis: i % 2 ? "horizontal" : "vertical", delta: 1 });
          }
          break;
        case "hard limit":
          f.socket.buffered = 131_072;
          f.frame(8);
          break;
        case "held release":
          f.socket.buffered = 32_768;
          f.queue.surface({ kind: "button", button: "left", down: false });
          break;
      }
      f.queue.surface({ kind: "click", button: "right", count: 1 });
      f.frame(200);
      expect(f.stopped).toEqual(["congestion"]);
      expect(values(f.operations())).toEqual([["mouseButtonTransition", TransitionState.DOWN]]);
      expect(f.scheduled.size).toBe(0);
    },
  );

  it.each(["closed", "broken"] as const)("stops on a %s writer", (fault) => {
    const f = setup();
    f.socket[fault] = true;
    f.queue.surface({ kind: "click", button: "left", count: 1 });
    expect(f.stopped).toEqual(["transport"]);
    expect(f.packets).toHaveLength(0);
  });

  it("rejects invalid limits before the dispatch loop and cancels scheduled frames on disposal", () => {
    expect(() => setup({ maximumOperationsPerBatch: 0 })).toThrow("Invalid client input limits");
    const f = setup();
    f.queue.surface({ kind: "move", dx: 5, dy: 5 });
    f.queue.dispose();
    f.frame(8);
    expect(f.scheduled.size).toBe(0);
    expect(f.packets).toHaveLength(0);
    expect(f.stopped).toHaveLength(0);
  });
});
