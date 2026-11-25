import { describe, expect, it } from "vitest";
import { attachControlSurface, type SurfaceAction, type SurfaceOptions } from "./control-surface";

class RegionElement extends EventTarget {
  readonly ids = new Set<number>();
  failCapture = false;

  setPointerCapture(id: number): void {
    if (this.failCapture) throw new Error("Capture refused");
    this.ids.add(id);
  }

  hasPointerCapture(id: number): boolean {
    return this.ids.has(id);
  }

  releasePointerCapture(id: number): void {
    this.ids.delete(id);
  }
}

type Region = "pad" | "vertical" | "horizontal" | "neutral";

function fixture(options?: SurfaceOptions) {
  let time = 0;
  let nextId = 0;
  const scheduled = new Map<number, FrameRequestCallback>();
  const regions = {
    pad: new RegionElement(),
    vertical: new RegionElement(),
    horizontal: new RegionElement(),
    neutral: new RegionElement(),
  };
  const surface = Object.assign(new EventTarget(), {
    querySelector: (selector: string) => {
      for (const region of ["pad", "vertical", "horizontal", "neutral"] as const) {
        if (selector === `[data-region="${region}"]`) return regions[region];
      }
      return null;
    },
  });
  const document = Object.assign(new EventTarget(), { hidden: false });
  const orientation = new EventTarget();
  const browser = Object.assign(new EventTarget(), {
    document,
    performance: { now: () => time },
    screen: { orientation },
    visualViewport: new EventTarget(),
    requestAnimationFrame(cb: FrameRequestCallback) {
      nextId += 1;
      scheduled.set(nextId, cb);
      return nextId;
    },
    cancelAnimationFrame: (id: number) => {
      scheduled.delete(id);
    },
  });
  const actions: SurfaceAction[] = [];
  const controller = attachControlSurface(
    surface as unknown as HTMLElement,
    (action) => {
      actions.push(action);
    },
    options,
    browser as unknown as Window,
  );
  const pointer = (
    target: EventTarget,
    type: string,
    id: number,
    x: number,
    y: number,
    button = 0,
  ) => {
    const fields = { pointerId: id, clientX: x, clientY: y, button };
    target.dispatchEvent(Object.assign(new Event(type, { cancelable: true }), fields));
  };
  const frame = (now: number) => {
    time = now;
    const callbacks = [...scheduled.values()];
    scheduled.clear();
    for (const callback of callbacks) callback(now);
  };
  return {
    actions,
    browser,
    controller,
    document,
    orientation,
    scheduled,
    regions,
    down: (region: Region, id = 1, x = 0, y = 0, button = 0) => {
      pointer(regions[region], "pointerdown", id, x, y, button);
    },
    pointer: (type: string, id = 1, x = 0, y = 0) => {
      pointer(surface, type, id, x, y);
    },
    frame,
  };
}

const press = { kind: "button", button: "left", down: true };
const move = { kind: "move", dx: 15, dy: 0 };
const release = { ...press, down: false };

function startDrag(f: ReturnType<typeof fixture>): void {
  f.down("pad");
  for (let time = 16; time <= 352; time += 16) f.frame(time);
  f.pointer("pointermove", 1, 15);
  expect(f.actions).toEqual([press, move]);
}

describe("pointer event surface", () => {
  it("latches the starting region across edges and the neutral corner", () => {
    const f = fixture();
    f.down("neutral", 3);
    f.pointer("pointermove", 3, 120, 125);
    f.pointer("pointerup", 3, 120, 125);
    expect(f.regions.neutral.ids.size).toBe(0);
    f.down("vertical");
    f.down("horizontal", 2);
    expect(f.regions.vertical.ids.has(1)).toBe(true);
    expect(f.regions.horizontal.ids.has(2)).toBe(true);
    f.frame(16);
    f.pointer("pointermove", 2, 48, 70);
    f.pointer("pointermove", 1, 90, 12);
    expect(f.actions).toEqual([
      { kind: "scroll", axis: "horizontal", delta: -1 },
      { kind: "scroll", axis: "vertical", delta: 0.25 },
    ]);
    f.controller.dispose();
  });

  it.each(["pointercancel", "lostpointercapture"])(
    "releases an active drag exactly once on %s",
    (event) => {
      const f = fixture();
      startDrag(f);
      f.pointer(event, 9);
      expect(f.actions).toEqual([press, move]);
      f.pointer(event);
      f.pointer(event);
      f.pointer("pointerup", 1, 15);
      expect(f.regions.pad.ids.size).toBe(0);
      expect(f.actions).toEqual([press, move, release]);
      expect(f.scheduled.size).toBe(0);
      f.controller.dispose();
    },
  );

  it.each([
    (f: ReturnType<typeof fixture>) => f.orientation.dispatchEvent(new Event("change")),
    (f: ReturnType<typeof fixture>) => f.browser.visualViewport.dispatchEvent(new Event("resize")),
    (f: ReturnType<typeof fixture>) => f.browser.dispatchEvent(new Event("pagehide")),
    (f: ReturnType<typeof fixture>) => {
      f.document.hidden = true;
      f.document.dispatchEvent(new Event("visibilitychange"));
    },
    (f: ReturnType<typeof fixture>) => {
      f.frame(500);
    },
    (f: ReturnType<typeof fixture>) => {
      f.controller.dispose();
    },
  ])(
    "neutralizes held input and drops stale motion on a lifecycle interruption %#",
    (interrupt) => {
      const f = fixture();
      startDrag(f);
      interrupt(f);
      f.pointer("pointermove", 1, 30);
      f.pointer("pointerup", 1, 30);
      expect(f.actions).toEqual([press, move, release]);
      expect(f.scheduled.size).toBe(0);
      f.controller.dispose();
    },
  );

  it("cancels both coasts on new input and consumes the opted-in first tap at a strip boundary", () => {
    const f = fixture({ drag: { longPress: true, doubleTap: true } });
    f.down("pad");
    f.frame(16);
    f.pointer("pointerup");
    expect(f.actions).toEqual([]);
    f.down("vertical", 2);
    f.down("horizontal", 3);
    expect(f.actions).toEqual([{ kind: "click", button: "left", count: 1 }]);
    f.frame(32);
    f.pointer("pointermove", 2, 0, 24);
    f.pointer("pointermove", 3, 48, 0);
    f.pointer("pointerup", 2, 0, 24);
    f.pointer("pointerup", 3, 48, 0);
    f.frame(48);
    expect(f.actions).toHaveLength(5);
    f.down("neutral", 4);
    f.frame(64);
    expect(f.actions).toHaveLength(5);
    expect(f.scheduled.size).toBe(0);
    f.controller.dispose();
  });

  it("does not create an uncaptured gesture and clears existing holds when capture fails", () => {
    const f = fixture();
    f.down("pad", 4, 0, 0, 2);
    expect(f.regions.pad.ids.size).toBe(0);
    startDrag(f);
    f.regions.horizontal.failCapture = true;
    f.down("horizontal", 2);
    f.pointer("pointerup", 2);
    f.pointer("pointerup", 1, 15);
    expect(f.actions).toEqual([press, move, release]);
    f.controller.dispose();
    f.down("pad", 3);
    expect(f.regions.pad.ids.size).toBe(0);
  });
});
