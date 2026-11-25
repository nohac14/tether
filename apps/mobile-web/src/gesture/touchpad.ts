export interface TouchSample {
  readonly id: number;
  readonly x: number;
  readonly y: number;
  readonly time: number;
}

export interface DragOptions {
  readonly longPress: boolean;
  readonly doubleTap: boolean;
}

export type PadAction =
  | { kind: "move"; dx: number; dy: number }
  | { kind: "click"; button: "left" | "right" | "middle"; count: 1 | 2 }
  | { kind: "button"; button: "left"; down: boolean };

export const DOUBLE_TAP_MS = 250;
export const DEFAULT_DRAG: DragOptions = { longPress: true, doubleTap: false };
const HOLD_MS = 350;
const TAP_MS = 200;
const JOIN_MS = 100;
const TAP_SLOP = 8;
const DOUBLE_TAP_SLOP = 24;
const MAX_STEP = 128;

interface Finger {
  start: TouchSample;
  last: TouchSample;
}

type Phase = "tap" | "second-tap" | "multi-tap" | "move" | "drag" | "rejected";

export function validSample(sample: TouchSample): boolean {
  return (
    Number.isSafeInteger(sample.id) &&
    Number.isFinite(sample.x) &&
    Number.isFinite(sample.y) &&
    Number.isFinite(sample.time) &&
    sample.time >= 0
  );
}

function click(button: "left" | "right" | "middle" = "left", count: 1 | 2 = 1): PadAction {
  return { kind: "click", button, count };
}

function button(down: boolean): PadAction {
  return { kind: "button", button: "left", down };
}

function distance(a: TouchSample, b: TouchSample): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

export class Touchpad {
  private readonly fingers = new Map<number, Finger>();
  private phase: Phase = "tap";
  private started = 0;
  private latest = 0;
  private fingerCount = 0;
  private pending: TouchSample | undefined;

  constructor(private readonly drag: DragOptions = DEFAULT_DRAG) {}

  get needsFrame(): boolean {
    return this.pending !== undefined;
  }

  advance(time: number): PadAction[] {
    if (!Number.isFinite(time) || time < this.latest) return this.cancel();
    this.latest = time;
    return this.pending && time - this.pending.time >= DOUBLE_TAP_MS ? this.finishTap() : [];
  }

  finishTap(): PadAction[] {
    if (!this.pending) return [];
    this.pending = undefined;
    if (this.phase === "second-tap") this.phase = "tap";
    return [click()];
  }

  down(sample: TouchSample): PadAction[] {
    if (this.fingers.has(sample.id)) return [];
    if (!validSample(sample) || sample.time < this.latest || this.fingers.size === 3) {
      return this.cancel();
    }
    const actions = this.advance(sample.time);
    if (this.fingers.size === 0) {
      this.started = sample.time;
      this.fingerCount = 1;
      if (this.pending && distance(this.pending, sample) <= DOUBLE_TAP_SLOP) {
        this.phase = "second-tap";
      } else {
        actions.push(...this.finishTap());
        this.phase = "tap";
      }
    } else if (
      (this.phase === "tap" || this.phase === "second-tap" || this.phase === "multi-tap") &&
      sample.time - this.started <= JOIN_MS &&
      this.fingers.size === this.fingerCount
    ) {
      actions.push(...this.finishTap());
      this.phase = "multi-tap";
      this.fingerCount += 1;
    } else {
      if (this.phase === "drag") actions.push(button(false));
      actions.push(...this.finishTap());
      this.phase = "rejected";
    }
    this.fingers.set(sample.id, { start: sample, last: sample });
    return actions;
  }

  move(sample: TouchSample): PadAction[] {
    const finger = this.fingers.get(sample.id);
    if (!finger) return [];
    if (!this.validMove(sample, finger)) return this.cancel();
    const actions = this.advance(sample.time);
    const dx = sample.x - finger.last.x;
    const dy = sample.y - finger.last.y;
    finger.last = sample;

    if (this.phase === "rejected") return actions;
    if (this.phase === "move" || this.phase === "drag") {
      if (dx !== 0 || dy !== 0) actions.push({ kind: "move", dx, dy });
      return actions;
    }
    if (distance(sample, finger.start) < TAP_SLOP) return actions;
    if (this.phase === "multi-tap") {
      this.phase = "rejected";
      return actions;
    }
    if (
      this.phase === "second-tap" ||
      (this.drag.longPress && sample.time - this.started >= HOLD_MS)
    ) {
      this.pending = undefined;
      this.phase = "drag";
      actions.push(button(true));
    } else {
      this.phase = "move";
    }
    actions.push({ kind: "move", dx, dy });
    return actions;
  }

  up(sample: TouchSample): PadAction[] {
    const finger = this.fingers.get(sample.id);
    if (!finger) return [];
    if (!this.validMove(sample, finger)) return this.cancel();
    const actions = this.advance(sample.time);
    this.fingers.delete(sample.id);

    if (this.phase === "drag") return [...actions, button(false)];
    if (this.phase === "move" || this.phase === "rejected") return actions;
    if (sample.time - this.started > TAP_MS || distance(sample, finger.start) >= TAP_SLOP) {
      this.phase = "rejected";
      return [...actions, ...this.finishTap()];
    }
    if (this.fingers.size > 0) return actions;
    switch (this.phase) {
      case "multi-tap":
        return [...actions, click(this.fingerCount === 2 ? "right" : "middle")];
      case "second-tap":
        this.pending = undefined;
        return [...actions, click("left", 2)];
      case "tap":
        if (this.drag.doubleTap) {
          this.pending = sample;
          return actions;
        }
        return [...actions, click()];
    }
  }

  cancel(): PadAction[] {
    const wasDragging = this.fingers.size > 0 && this.phase === "drag";
    this.fingers.clear();
    this.pending = undefined;
    this.phase = "tap";
    this.latest = 0;
    return wasDragging ? [button(false)] : [];
  }

  private validMove(sample: TouchSample, finger: Finger): boolean {
    return (
      validSample(sample) && sample.time >= this.latest && distance(sample, finger.last) <= MAX_STEP
    );
  }
}
