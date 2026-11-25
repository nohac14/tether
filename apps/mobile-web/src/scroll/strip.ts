import { type TouchSample, validSample } from "../gesture/touchpad";

export type ScrollAxis = "vertical" | "horizontal";
export type ScrollDirection = "natural" | "traditional";
export type Momentum = "off" | "low" | "medium" | "high";

export interface StripOptions {
  readonly direction: ScrollDirection;
  readonly sensitivity: number;
  readonly momentum: Momentum;
}

export interface ScrollAction {
  kind: "scroll";
  axis: ScrollAxis;
  delta: number;
}

export const DEFAULT_STRIP: StripOptions = {
  direction: "natural",
  sensitivity: 1,
  momentum: "medium",
};
export const SUSPENSION_MS = 100;
const PIXELS_PER_UNIT = 48;
const MAX_STEP = 160;
const MAX_VELOCITY = 2.5;
const MIN_VELOCITY = 0.04;
const VELOCITY_WINDOW_MS = 80;
const MIN_SAMPLE_MS = 8;
const MAX_SAMPLES = 8;
const RELEASE_WINDOW_MS = 60;

const profiles = {
  low: { decay: 85, duration: 250 },
  medium: { decay: 140, duration: 480 },
  high: { decay: 215, duration: 700 },
} as const;

interface SwipeSample {
  position: number;
  time: number;
}

interface Coast {
  start: number;
  time: number;
  velocity: number;
  decay: number;
  duration: number;
}

export class ScrollStrip {
  private touch: { id: number; last: SwipeSample } | undefined;
  private recent: SwipeSample[] = [];
  private lastMotion: number | undefined;
  private motionDirection = 0;
  private coast: Coast | undefined;
  private readonly gain: number;
  private readonly profile: (typeof profiles)[keyof typeof profiles] | undefined;

  constructor(
    private readonly axis: ScrollAxis,
    options: StripOptions = DEFAULT_STRIP,
  ) {
    if (
      !Number.isFinite(options.sensitivity) ||
      options.sensitivity < 0.125 ||
      options.sensitivity > 8
    ) {
      throw new RangeError("Invalid scroll sensitivity");
    }
    const axisSign = axis === "vertical" ? 1 : -1;
    const directionSign = options.direction === "natural" ? 1 : -1;
    this.gain = (axisSign * directionSign * options.sensitivity) / PIXELS_PER_UNIT;
    this.profile = options.momentum === "off" ? undefined : profiles[options.momentum];
  }

  get needsFrame(): boolean {
    return this.coast !== undefined;
  }

  down(sample: TouchSample): boolean {
    this.stopMomentum();
    if (!validSample(sample) || this.touch) return false;
    const first = this.position(sample);
    this.touch = { id: sample.id, last: first };
    this.resetVelocity(first);
    return true;
  }

  move(sample: TouchSample): ScrollAction | undefined {
    const touch = this.touch;
    if (touch?.id !== sample.id) return;
    if (!validSample(sample) || sample.time < touch.last.time) {
      this.cancel();
      return;
    }
    const current = this.position(sample);
    const previous = touch.last;
    const delta = current.position - previous.position;
    touch.last = current;
    if (Math.abs(delta) > MAX_STEP || current.time - previous.time > SUSPENSION_MS) {
      this.resetVelocity(current);
      return;
    }
    if (delta === 0) return;
    if (
      Math.sign(delta) !== this.motionDirection ||
      current.time - previous.time > VELOCITY_WINDOW_MS
    ) {
      this.resetVelocity(previous);
    }
    this.motionDirection = Math.sign(delta);
    this.lastMotion = current.time;
    this.recent.push(current);
    while (
      this.recent.length > MAX_SAMPLES ||
      (this.recent.length > 1 &&
        current.time - (this.recent[0]?.time ?? current.time) > VELOCITY_WINDOW_MS)
    ) {
      this.recent.shift();
    }
    return this.action(delta);
  }

  up(sample: TouchSample): ScrollAction | undefined {
    if (this.touch?.id !== sample.id) return;
    const action = this.move(sample);
    this.touch = undefined;
    const profile = this.profile;
    const first = this.recent[0];
    const last = this.recent.at(-1);
    if (
      profile &&
      first &&
      last &&
      this.lastMotion !== undefined &&
      sample.time - this.lastMotion <= RELEASE_WINDOW_MS
    ) {
      const estimate =
        (last.position - first.position) / Math.max(MIN_SAMPLE_MS, last.time - first.time);
      const velocity = Math.max(-MAX_VELOCITY, Math.min(MAX_VELOCITY, estimate));
      if (Math.abs(velocity) >= MIN_VELOCITY) {
        this.coast = { start: sample.time, time: sample.time, velocity, ...profile };
      }
    }
    this.resetVelocity();
    return action;
  }

  advance(time: number): ScrollAction | undefined {
    const coast = this.coast;
    if (!coast) return;
    if (!Number.isFinite(time) || time < coast.time || time - coast.time > SUSPENSION_MS) {
      this.stopMomentum();
      return;
    }
    const end = Math.min(time, coast.start + coast.duration);
    const elapsed = end - coast.time;
    const fraction = -Math.expm1(-elapsed / coast.decay);
    const travel = coast.velocity * coast.decay * fraction;
    coast.time = end;
    coast.velocity *= 1 - fraction;
    if (time - coast.start >= coast.duration || Math.abs(coast.velocity) < MIN_VELOCITY) {
      this.stopMomentum();
    }
    return this.action(travel);
  }

  stopMomentum(): void {
    this.coast = undefined;
  }

  cancel(): void {
    this.touch = undefined;
    this.resetVelocity();
    this.stopMomentum();
  }

  private position(sample: TouchSample): SwipeSample {
    return { position: this.axis === "vertical" ? sample.y : sample.x, time: sample.time };
  }

  private resetVelocity(first?: SwipeSample): void {
    this.recent = first ? [first] : [];
    this.lastMotion = undefined;
    this.motionDirection = 0;
  }

  private action(pixels: number): ScrollAction | undefined {
    return pixels === 0
      ? undefined
      : { kind: "scroll", axis: this.axis, delta: pixels * this.gain };
  }
}
