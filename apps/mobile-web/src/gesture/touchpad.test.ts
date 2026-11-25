import { describe, expect, it } from "vitest";
import { DOUBLE_TAP_MS, Touchpad, type TouchSample } from "./touchpad";

const at = (time: number, x = 10, y = 15, id = 1): TouchSample => ({ id, x, y, time });
const left = { kind: "click", button: "left", count: 1 };
const press = { kind: "button", button: "left", down: true };
const release = { ...press, down: false };

describe("touchpad gestures", () => {
  it("delivers both taps on their release with zero recognizer delay by default", () => {
    const pad = new Touchpad();
    expect(pad.down(at(10))).toEqual([]);
    expect(pad.move(at(25, 13))).toEqual([]);
    expect(pad.up(at(40))).toEqual([left]);
    expect(pad.needsFrame).toBe(false);
    pad.down(at(60));
    expect(pad.up(at(95))).toEqual([left]);
    expect(pad.advance(400)).toEqual([]);
  });

  it("delays an opted-in single tap by exactly the decision window", () => {
    const pad = new Touchpad({ longPress: false, doubleTap: true });
    pad.down(at(10));
    expect(pad.up(at(45))).toEqual([]);
    expect(pad.needsFrame).toBe(true);
    expect(pad.advance(45 + DOUBLE_TAP_MS - 1)).toEqual([]);
    expect(pad.advance(45 + DOUBLE_TAP_MS)).toEqual([left]);
    expect(pad.advance(500)).toEqual([]);
  });

  it("emits one atomic double click when two opted-in taps qualify", () => {
    const pad = new Touchpad({ longPress: true, doubleTap: true });
    pad.down(at(10));
    pad.up(at(25));
    pad.down(at(85, 28, 22, 2));
    expect(pad.up(at(140, 28, 22, 2))).toEqual([{ ...left, count: 2 }]);
    expect(pad.advance(400)).toEqual([]);
  });

  it("resolves a pending tap before an unrelated touch or at the first deadline", () => {
    const pad = new Touchpad({ longPress: true, doubleTap: true });
    pad.down(at(0));
    pad.up(at(20));
    expect(pad.down(at(40, 45))).toEqual([left]);
    pad.up(at(60, 45));
    expect(pad.down(at(300, 45))).toEqual([]);
    expect(pad.advance(310)).toEqual([left]);
    expect(pad.move(at(650, 56))).toEqual([press, { kind: "move", dx: 11, dy: 0 }]);
    expect(pad.up(at(700, 56))).toEqual([release]);
  });

  it("holds only when motion begins after the long-press threshold", () => {
    const pad = new Touchpad();
    pad.down(at(0));
    expect(pad.advance(350)).toEqual([]);
    expect(pad.move(at(350, 20))).toEqual([press, { kind: "move", dx: 10, dy: 0 }]);
    expect(pad.move(at(370, 18, 21))).toEqual([{ kind: "move", dx: -2, dy: 6 }]);
    expect(pad.up(at(400, 18, 21))).toEqual([release]);
    expect(pad.cancel()).toEqual([]);
    pad.down(at(500));
    expect(pad.up(at(950))).toEqual([]);
  });

  it.each([false, true])(
    "double-tap drag has exactly one press with long press %s",
    (longPress) => {
      const pad = new Touchpad({ longPress, doubleTap: true });
      pad.down(at(0));
      pad.up(at(20));
      pad.down(at(80));
      expect(pad.move(at(120, 25))).toEqual([press, { kind: "move", dx: 15, dy: 0 }]);
      expect(pad.advance(400)).toEqual([]);
      expect(pad.move(at(420, 28))).toEqual([{ kind: "move", dx: 3, dy: 0 }]);
      expect(pad.up(at(440, 28))).toEqual([release]);
    },
  );

  it("continues unheld motion after an ordinary swipe or with both drag options off", () => {
    for (const pad of [new Touchpad(), new Touchpad({ longPress: false, doubleTap: false })]) {
      pad.down(at(0));
      expect(pad.move(at(349, 20))).toEqual([{ kind: "move", dx: 10, dy: 0 }]);
      expect(pad.move(at(540, 25))).toEqual([{ kind: "move", dx: 5, dy: 0 }]);
      expect(pad.up(at(555, 25))).toEqual([]);
    }
    const pad = new Touchpad({ longPress: false, doubleTap: false });
    pad.down(at(0));
    expect(pad.move(at(500, 20))).toEqual([{ kind: "move", dx: 10, dy: 0 }]);
  });

  it.each([
    [2, "right"],
    [3, "middle"],
  ] as const)("recognizes a %s-finger %s click after all lift", (count, button) => {
    const pad = new Touchpad();
    for (let id = 1; id <= count; id += 1) {
      expect(pad.down(at(id * 10, id * 30, 5, id))).toEqual([]);
    }
    expect(pad.down(at(50, 30, 5, 1))).toEqual([]);
    expect(pad.move(at(55, 31, 9, 1))).toEqual([]);
    for (let id: number = count; id >= 1; id -= 1) {
      expect(pad.up(at(65 + count - id, id * 30, 5, id))).toEqual(
        id === 1 ? [{ kind: "click", button, count: 1 }] : [],
      );
    }
  });

  it("rejects late or moving multitouch, a fourth contact, and foreign pointer moves", () => {
    const pad = new Touchpad();
    pad.down(at(0));
    expect(pad.move(at(15, 80, 15, 7))).toEqual([]);
    pad.down(at(110, 20, 15, 2));
    expect(pad.up(at(120, 20, 15, 2))).toEqual([]);
    expect(pad.up(at(140))).toEqual([]);
    pad.down(at(150));
    pad.down(at(160, 20, 15, 2));
    expect(pad.move(at(170, 35, 15, 2))).toEqual([]);
    expect(pad.up(at(180, 35, 15, 2))).toEqual([]);
    expect(pad.up(at(190))).toEqual([]);
    for (let id = 1; id <= 4; id += 1) pad.down(at(200 + id, 10, 15, id));
    for (let id = 1; id <= 4; id += 1) expect(pad.up(at(220 + id, 10, 15, id))).toEqual([]);
  });

  it("releases once on cancellation, secondary down, and anomalous samples", () => {
    for (const abort of [
      (pad: Touchpad) => pad.cancel(),
      (pad: Touchpad) => pad.down(at(380, 30, 15, 2)),
      (pad: Touchpad) => pad.move(at(380, Number.NaN)),
      (pad: Touchpad) => pad.move(at(380, 999)),
      (pad: Touchpad) => pad.move(at(330, 25)),
    ]) {
      const pad = new Touchpad();
      pad.down(at(0));
      pad.move(at(350, 20));
      expect(abort(pad)).toEqual([release]);
      expect(pad.cancel()).toEqual([]);
      expect(pad.up(at(400, 20))).toEqual([]);
    }
  });

  it("discards pending taps and rejected release-only motion on reset", () => {
    const pad = new Touchpad({ longPress: true, doubleTap: true });
    pad.down(at(0));
    expect(pad.up(at(20, 20))).toEqual([]);
    pad.down(at(50));
    pad.up(at(75));
    expect(pad.cancel()).toEqual([]);
    expect(pad.advance(400)).toEqual([]);
  });
});
