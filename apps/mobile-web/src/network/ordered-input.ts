import { create, toBinary } from "@bufbuild/protobuf";
import type { SurfaceAction } from "../app/control-surface";
import { MAX_BACKSPACE_REPEAT, type LiveAction } from "../keyboard/live-edit";
import {
  type ClientEnvelope,
  ClientEnvelopeSchema,
  type ClientOperation,
  type ClientOperationBatch,
  ClientOperationBatchSchema,
  ClientOperationSchema,
  MouseButton,
  MouseButtonClickSchema,
  MouseButtonTransitionSchema,
  type ProtocolLimits,
  ScrollAxis,
  TextEditCommand,
  TextEditSchema,
  TextInsertionSchema,
  TransitionState,
} from "../protocol/generated/tether/control/v1/control_pb";

type Barrier = Exclude<ClientOperation["action"], { case: undefined | "pointerMove" | "scroll" }>;
export type InputStopReason = "congestion" | "transport" | "operation-limit";

export interface InputTransport {
  bufferedAmount(): number;
  // An oversized result must leave the connection's sequence and send buffer untouched.
  writeBatch(batch: ClientOperationBatch, maximumBytes: number): "sent" | "oversized" | "closed";
  abortInput(reason: InputStopReason): void;
}

export interface FrameClock {
  now(): number;
  requestFrame(task: () => void): number;
  cancelFrame(id: number): void;
}

export interface PointerGain {
  sensitivity: number;
  acceleration?: { threshold: number; maximum: number };
}

const SOFT_BUFFER_BYTES = 32 * 1024;
const HARD_BUFFER_BYTES = 128 * 1024;
const MAX_PENDING_MS = 80;
const MAX_RUNS = 32;
const SCALE = 1024;
const MAX_SAMPLE_PIXELS = 128;
const clickButton = {
  left: MouseButton.LEFT,
  right: MouseButton.RIGHT,
  middle: MouseButton.MIDDLE,
};
type MotionKind = "pointer" | "horizontal" | "vertical";
interface Run {
  kind: MotionKind;
  x: number;
  y: number;
  clippedX: boolean;
  clippedY: boolean;
}

const clamp = (value: number, limit: number): number => Math.max(-limit, Math.min(limit, value));

const limitsCeiling = {
  maximumEncodedMessageBytes: 65_536,
  maximumOperationsPerBatch: 256,
  maximumTextOperationBytes: 4_096,
  maximumTextBytesPerBatch: 16_384,
  maximumAbsoluteFixedDelta: 8_388_608,
  maximumAtomicClickCount: 2,
};

type ClientInputLimits = Pick<ProtocolLimits, keyof typeof limitsCeiling>;
type BatchHeader = Pick<
  ClientEnvelope,
  "protocolMajor" | "protocolMinor" | "sequence" | "sessionEpoch" | "clientTimestampMicros"
>;

export function encodeInputBatch(
  header: BatchHeader,
  batch: ClientOperationBatch,
  maximumBytes: number,
): Uint8Array | undefined {
  const envelope = create(ClientEnvelopeSchema, {
    ...header,
    payload: { case: "operationBatch", value: batch },
  });
  const bytes = toBinary(ClientEnvelopeSchema, envelope);
  return bytes.length <= maximumBytes ? bytes : undefined;
}

export class OrderedInput {
  private readonly pending: Run[] = [];
  private readonly remainder = {
    pointer: { x: 0, y: 0 },
    vertical: { x: 0, y: 0 },
    horizontal: { x: 0, y: 0 },
  };
  private frame: number | undefined;
  private pendingSince: number | undefined;
  private active = true;
  private readonly textEncoder = new TextEncoder();
  private readonly limits: ClientInputLimits;
  private readonly gain: PointerGain;

  constructor(
    private readonly transport: InputTransport,
    private readonly clock: FrameClock,
    limits: ClientInputLimits,
    gain: PointerGain = { sensitivity: 1 },
  ) {
    for (const key of Object.keys(limitsCeiling) as (keyof ClientInputLimits)[]) {
      if (
        !Number.isSafeInteger(limits[key]) ||
        limits[key] < 1 ||
        limits[key] > limitsCeiling[key]
      ) {
        throw new RangeError("Invalid client input limits");
      }
    }
    this.limits = { ...limits };
    if (
      !Number.isFinite(gain.sensitivity) ||
      gain.sensitivity < 0.125 ||
      gain.sensitivity > 8 ||
      (gain.acceleration &&
        (!Number.isFinite(gain.acceleration.maximum) ||
          gain.acceleration.maximum < 1 ||
          gain.acceleration.maximum > 4 ||
          !Number.isFinite(gain.acceleration.threshold) ||
          gain.acceleration.threshold < 0 ||
          gain.acceleration.threshold >= MAX_SAMPLE_PIXELS))
    ) {
      throw new RangeError("Invalid pointer gain");
    }
    this.gain = gain.acceleration
      ? { sensitivity: gain.sensitivity, acceleration: { ...gain.acceleration } }
      : { sensitivity: gain.sensitivity };
  }

  surface(action: SurfaceAction): void {
    if (!this.active) return;
    switch (action.kind) {
      case "move": {
        if (!Number.isFinite(action.dx) || !Number.isFinite(action.dy)) return;
        const x = clamp(action.dx, MAX_SAMPLE_PIXELS);
        const y = clamp(action.dy, MAX_SAMPLE_PIXELS);
        const curve = this.gain.acceleration;
        const speed = Math.max(Math.abs(x), Math.abs(y));
        const multiplier = curve
          ? 1 +
            ((curve.maximum - 1) * Math.max(0, speed - curve.threshold)) /
              (MAX_SAMPLE_PIXELS - curve.threshold)
          : 1;
        const factor = SCALE * this.gain.sensitivity * multiplier;
        this.accumulate("pointer", x * factor, y * factor);
        break;
      }
      case "scroll":
        if (Number.isFinite(action.delta)) this.accumulate(action.axis, action.delta * SCALE, 0);
        break;
      case "button":
        this.barrier({
          case: "mouseButtonTransition",
          value: create(MouseButtonTransitionSchema, {
            button: clickButton[action.button],
            state: action.down ? TransitionState.DOWN : TransitionState.UP,
          }),
        });
        break;
      case "click":
        this.barrier({
          case: "mouseButtonClick",
          value: create(MouseButtonClickSchema, {
            button: clickButton[action.button],
            clickCount: action.count,
          }),
        });
    }
  }

  keyboard(action: LiveAction): void {
    if (action.kind === "insert") {
      this.barrier({
        case: "textInsertion",
        value: create(TextInsertionSchema, { text: action.text }),
      });
      return;
    }
    if (
      !Number.isSafeInteger(action.repeatCount) ||
      action.repeatCount < 1 ||
      action.repeatCount > MAX_BACKSPACE_REPEAT
    ) {
      this.fail("operation-limit");
      return;
    }
    const limit = this.limits.maximumOperationsPerBatch;
    for (let remaining = action.repeatCount; this.active && remaining > 0; remaining -= limit) {
      this.barrier({
        case: "textEdit",
        value: create(TextEditSchema, {
          command: TextEditCommand.DELETE_BACKWARD,
          repeatCount: Math.min(remaining, limit),
        }),
      });
    }
  }

  barrier(action: Barrier): void {
    if (!this.active) return;
    if (
      action.case === "mouseButtonClick" &&
      action.value.clickCount !== 1 &&
      action.value.clickCount !== 2
    ) {
      this.fail("operation-limit");
      return;
    }
    if (action.case === "textInsertion") {
      const budget = Math.min(
        this.limits.maximumTextOperationBytes,
        this.limits.maximumTextBytesPerBatch,
      );
      const text = action.value.text;
      if (text.length > budget || this.textEncoder.encode(text).byteLength > budget) {
        this.fail("operation-limit");
        return;
      }
    }
    const status = this.bufferStatus();
    if (status !== "ready") {
      if (status === "waiting") this.fail("congestion");
      return;
    }
    const operations = this.drain();
    const limit = this.limits.maximumOperationsPerBatch;
    if (
      action.case === "textEdit" &&
      (operations.length % limit) + action.value.repeatCount > limit
    ) {
      this.write(operations);
      operations.length = 0;
    }
    if (
      action.case === "mouseButtonClick" &&
      action.value.clickCount > this.limits.maximumAtomicClickCount
    ) {
      const single = create(ClientOperationSchema, {
        action: { case: "mouseButtonClick", value: { button: action.value.button, clickCount: 1 } },
      });
      operations.push(single, single);
    } else {
      operations.push(create(ClientOperationSchema, { action }));
    }
    this.clearRemainder();
    this.write(operations);
    if (
      action.case === "requestControl" ||
      action.case === "relinquishControl" ||
      action.case === "releaseAll"
    ) {
      this.dispose();
    }
  }

  dispose(): void {
    this.active = false;
    this.clearPending();
    this.clearRemainder();
  }

  private accumulate(kind: MotionKind, x: number, y: number): void {
    if ((x === 0 && y === 0) || this.bufferStatus() === "stopped") return;
    const limit = this.limits.maximumAbsoluteFixedDelta;
    const last = this.pending.at(-1);
    const limitedX = clamp(x, limit);
    const limitedY = clamp(y, limit);
    if (last?.kind === kind) {
      const nextX = last.x + limitedX;
      const nextY = last.y + limitedY;
      last.x = clamp(nextX, limit);
      last.y = clamp(nextY, limit);
      last.clippedX ||= nextX !== last.x || x !== limitedX;
      last.clippedY ||= nextY !== last.y || y !== limitedY;
    } else if (this.pending.length < MAX_RUNS) {
      this.pending.push({
        kind,
        x: limitedX,
        y: limitedY,
        clippedX: x !== limitedX,
        clippedY: y !== limitedY,
      });
    } else {
      this.fail("congestion");
      return;
    }
    this.pendingSince ??= this.clock.now();
    this.schedule();
  }

  private schedule(): void {
    this.frame ??= this.clock.requestFrame(() => {
      this.frame = undefined;
      switch (this.bufferStatus()) {
        case "ready":
          this.write(this.drain());
          break;
        case "waiting":
          this.schedule();
          break;
        case "stopped":
          break;
      }
    });
  }

  private drain(): ClientOperation[] {
    const operations: ClientOperation[] = [];
    for (const run of this.pending) {
      const remainder = this.remainder[run.kind];
      const limit = this.limits.maximumAbsoluteFixedDelta;
      const x = clamp(run.x + (run.clippedX ? 0 : remainder.x), limit);
      const y = clamp(run.y + (run.clippedY ? 0 : remainder.y), limit);
      const integralX = Math.trunc(x);
      const integralY = Math.trunc(y);
      remainder.x = x - integralX;
      remainder.y = y - integralY;
      if (integralX === 0 && integralY === 0) continue;
      operations.push(
        create(ClientOperationSchema, {
          action:
            run.kind === "pointer"
              ? { case: "pointerMove", value: { deltaX1024: integralX, deltaY1024: integralY } }
              : {
                  case: "scroll",
                  value: {
                    axis: run.kind === "horizontal" ? ScrollAxis.HORIZONTAL : ScrollAxis.VERTICAL,
                    delta1024: integralX,
                  },
                },
        }),
      );
    }
    this.clearPending();
    return operations;
  }

  private write(operations: ClientOperation[]): void {
    const max = this.limits.maximumOperationsPerBatch;
    for (let index = 0; index < operations.length; index += max) {
      if (!this.writeBatch(operations.slice(index, index + max))) return;
    }
  }

  private writeBatch(operations: ClientOperation[]): boolean {
    if (!this.active) return false;
    const status = this.bufferStatus();
    if (status !== "ready") {
      if (status === "waiting") this.fail("congestion");
      return false;
    }
    let result: ReturnType<InputTransport["writeBatch"]>;
    try {
      result = this.transport.writeBatch(
        create(ClientOperationBatchSchema, { operations }),
        this.limits.maximumEncodedMessageBytes,
      );
    } catch {
      this.fail("transport");
      return false;
    }
    if (result === "sent") return true;
    if (result === "closed") this.fail("transport");
    else if (operations.length <= 1) this.fail("operation-limit");
    else {
      const middle = Math.floor(operations.length / 2);
      return (
        this.writeBatch(operations.slice(0, middle)) && this.writeBatch(operations.slice(middle))
      );
    }
    return false;
  }

  private bufferStatus(): "ready" | "waiting" | "stopped" {
    if (!this.active) return "stopped";
    let buffered: number;
    try {
      buffered = this.transport.bufferedAmount();
    } catch {
      this.fail("transport");
      return "stopped";
    }
    if (!Number.isSafeInteger(buffered) || buffered < 0) {
      this.fail("transport");
      return "stopped";
    }
    const expired =
      this.pendingSince !== undefined && this.clock.now() - this.pendingSince >= MAX_PENDING_MS;
    if (buffered >= HARD_BUFFER_BYTES || expired) {
      this.fail("congestion");
      return "stopped";
    }
    return buffered >= SOFT_BUFFER_BYTES ? "waiting" : "ready";
  }

  private clearPending(): void {
    this.pending.length = 0;
    this.pendingSince = undefined;
    if (this.frame !== undefined) this.clock.cancelFrame(this.frame);
    this.frame = undefined;
  }

  private clearRemainder(): void {
    for (const remainder of Object.values(this.remainder)) {
      remainder.x = 0;
      remainder.y = 0;
    }
  }

  private fail(reason: InputStopReason): void {
    if (!this.active) return;
    this.dispose();
    this.transport.abortInput(reason);
  }
}
