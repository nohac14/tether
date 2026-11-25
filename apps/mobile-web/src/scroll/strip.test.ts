import { describe, expect, it } from "vitest";
import type { TouchSample } from "../gesture/touchpad";
import { DEFAULT_STRIP, ScrollStrip, SUSPENSION_MS, type Momentum } from "./strip";

const at = (time: number, y = 0, x = 0, id = 1): TouchSample => ({ id, x, y, time });

function fastSwipe(strip: ScrollStrip): void {
  strip.down(at(0));
  strip.move(at(16, 50));
  strip.move(at(32, 100));
  strip.up(at(36, 100));
}

function distanceAfterRelease(momentum: Momentum, interval: number): number {
  const strip = new ScrollStrip("vertical", { ...DEFAULT_STRIP, momentum });
  fastSwipe(strip);
  let sum = 0;
  for (let time = 36 + interval; time < 820; time += interval) {
    sum += strip.advance(time)?.delta ?? 0;
  }
  expect(strip.needsFrame).toBe(false);
  return sum;
}

describe("dedicated scroll strips", () => {
  it("uses only the starting axis, with independent settings and fractional deltas", () => {
    const vertical = new ScrollStrip("vertical", {
      direction: "natural",
      sensitivity: 0.5,
      momentum: "off",
    });
    const horizontal = new ScrollStrip("horizontal", {
      direction: "traditional",
      sensitivity: 2,
      momentum: "off",
    });
    expect(vertical.down(at(0, 5, 6))).toBe(true);
    expect(horizontal.down(at(0, 5, 6, 2))).toBe(true);
    expect(vertical.move(at(10, 5, 80))).toBeUndefined();
    expect(horizontal.move(at(10, 50, 6, 2))).toBeUndefined();
    expect(vertical.move(at(20, 5.5, 100))).toEqual({
      kind: "scroll",
      axis: "vertical",
      delta: 0.25 / 48,
    });
    expect(horizontal.move(at(20, 110, 18, 2))).toEqual({
      kind: "scroll",
      axis: "horizontal",
      delta: 0.5,
    });
    expect(vertical.move(at(30, 5.25, 100))).toEqual({
      kind: "scroll",
      axis: "vertical",
      delta: -0.125 / 48,
    });
    expect(horizontal.move(at(30, 110, 6, 2))).toEqual({
      kind: "scroll",
      axis: "horizontal",
      delta: -0.5,
    });
  });

  it.each([
    ["vertical", "natural", 1],
    ["vertical", "traditional", -1],
    ["horizontal", "natural", -1],
    ["horizontal", "traditional", 1],
  ] as const)("maps a positive %s sample in %s direction", (axis, direction, expected) => {
    const strip = new ScrollStrip(axis, { ...DEFAULT_STRIP, direction });
    strip.down(at(0));
    expect(strip.move(at(16, 48, 48))?.delta).toBe(expected);
  });

  it("accepts repeated short swipes at unrelated starting positions without a jump", () => {
    const strip = new ScrollStrip("vertical", { ...DEFAULT_STRIP, momentum: "off" });
    for (let i = 0; i < 500; i += 1) {
      const time = i * 25;
      const start = i % 2 ? -700 : 700;
      expect(strip.down(at(time, start))).toBe(true);
      expect(strip.move(at(time + 10, start + 3))?.delta).toBe(0.0625);
      expect(strip.up(at(time + 15, start + 3))).toBeUndefined();
    }
  });

  it("decays based on time and bounds each momentum profile", () => {
    const off = distanceAfterRelease("off", 16);
    const low = distanceAfterRelease("low", 16);
    const medium = distanceAfterRelease("medium", 16);
    const high = distanceAfterRelease("high", 16);
    expect(off).toBe(0);
    expect(low).toBeGreaterThan(off);
    expect(medium).toBeGreaterThan(low);
    expect(high).toBeGreaterThan(medium);
    expect(high).toBeLessThan((2.5 * 215) / 48);
    expect(distanceAfterRelease("high", 8)).toBeCloseTo(high, 8);
    expect(distanceAfterRelease("high", 25)).toBeCloseTo(high, 8);
    const strip = new ScrollStrip("vertical");
    fastSwipe(strip);
    const first = strip.advance(52)?.delta ?? 0;
    const second = strip.advance(68)?.delta ?? 0;
    expect(first).toBeGreaterThan(second);
    expect(second).toBeGreaterThan(0);
  });

  it("stops momentum on a new gesture, cancellation or a suspended frame", () => {
    for (const stop of [
      (strip: ScrollStrip) => strip.down(at(50, 600, 0, 8)),
      (strip: ScrollStrip) => {
        strip.cancel();
      },
      (strip: ScrollStrip) => strip.advance(36 + SUSPENSION_MS + 1),
      (strip: ScrollStrip) => strip.advance(35),
    ]) {
      const strip = new ScrollStrip("vertical");
      fastSwipe(strip);
      expect(strip.needsFrame).toBe(true);
      stop(strip);
      expect(strip.needsFrame).toBe(false);
      expect(strip.advance(200)).toBeUndefined();
    }
  });

  it("does not coast after a stationary release, a malformed event, or a dropped jump", () => {
    for (const stop of [at(100, 22), at(40, Number.NaN), at(40, 500), at(19, 26)]) {
      const strip = new ScrollStrip("vertical");
      strip.down(at(0));
      strip.move(at(20, 22));
      expect(strip.up(stop)).toBeUndefined();
      expect(strip.advance(120)).toBeUndefined();
    }
  });

  it("reanchors after an anomaly and coasts only in the most recent direction", () => {
    const strip = new ScrollStrip("vertical");
    strip.down(at(0));
    expect(strip.move(at(15, 500))).toBeUndefined();
    expect(strip.move(at(30, 548))?.delta).toBe(1);
    expect(strip.move(at(45, 524))?.delta).toBe(-0.5);
    strip.up(at(50, 524));
    expect(strip.advance(65)?.delta).toBeLessThan(0);
  });

  it("does not accept overlapping pointers on one strip", () => {
    const strip = new ScrollStrip("vertical");
    expect(strip.down(at(0))).toBe(true);
    expect(strip.down(at(10, 500, 10, 2))).toBe(false);
    expect(strip.move(at(20, 520, 10, 2))).toBeUndefined();
    expect(strip.up(at(30, 520, 10, 2))).toBeUndefined();
    expect(strip.move(at(40, 48))?.delta).toBe(1);
    strip.cancel();
    expect(strip.up(at(50, 48))).toBeUndefined();
  });
});
