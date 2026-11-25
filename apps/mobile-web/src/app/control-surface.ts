import { Touchpad, type DragOptions, type PadAction, type TouchSample } from "../gesture/touchpad";
import {
  ScrollStrip,
  SUSPENSION_MS,
  type ScrollAction,
  type ScrollAxis,
  type StripOptions,
} from "../scroll/strip";

type Region = "pad" | ScrollAxis;
export type SurfaceAction = PadAction | ScrollAction;

export interface SurfaceOptions {
  drag?: DragOptions;
  vertical?: StripOptions;
  horizontal?: StripOptions;
}

// Attach only while the authenticated session owns input. The sink accepts actions synchronously.
export function attachControlSurface(
  surface: HTMLElement,
  emit: (action: SurfaceAction) => void,
  options: SurfaceOptions = {},
  browser: Window = window,
): { reset(): void; dispose(): void } {
  const pad = new Touchpad(options.drag);
  const strips = {
    vertical: new ScrollStrip("vertical", options.vertical),
    horizontal: new ScrollStrip("horizontal", options.horizontal),
  };
  const captured = new Map<number, { element: HTMLElement; region: Region }>();
  const listeners = new AbortController();
  const document = browser.document;
  let frame: number | undefined;
  let lastFrame: number | undefined;
  let attached = true;
  let revision = 0;

  const emitPad = (actions: PadAction[]) => {
    const current = revision;
    for (const action of actions) {
      if (!attached || revision !== current) break;
      emit(action);
    }
  };
  const emitScroll = (action: ScrollAction | undefined) => {
    if (attached && action) emit(action);
  };
  const stopMomentum = () => {
    strips.vertical.stopMomentum();
    strips.horizontal.stopMomentum();
  };
  const reset = () => {
    revision += 1;
    const release = pad.cancel();
    strips.vertical.cancel();
    strips.horizontal.cancel();
    const pointers = [...captured];
    captured.clear();
    if (frame !== undefined) browser.cancelAnimationFrame(frame);
    frame = undefined;
    lastFrame = undefined;
    for (const [id, { element }] of pointers) {
      try {
        if (element.hasPointerCapture(id)) element.releasePointerCapture(id);
      } catch {
        // The browser may already have retired this pointer during cancellation.
      }
    }
    emitPad(release);
  };
  const checkSuspension = (time: number) => {
    if (lastFrame === undefined || time - lastFrame <= SUSPENSION_MS) return false;
    reset();
    return true;
  };
  const schedule = (time: number) => {
    if (!attached || frame !== undefined) return;
    if (
      captured.size === 0 &&
      !pad.needsFrame &&
      !strips.vertical.needsFrame &&
      !strips.horizontal.needsFrame
    ) {
      lastFrame = undefined;
      return;
    }
    lastFrame ??= time;
    frame = browser.requestAnimationFrame(() => {
      frame = undefined;
      const now = browser.performance.now();
      if (checkSuspension(now)) return;
      lastFrame = now;
      emitPad(pad.advance(now));
      emitScroll(strips.vertical.advance(now));
      emitScroll(strips.horizontal.advance(now));
      schedule(now);
    });
  };
  const sample = (event: PointerEvent, time: number): TouchSample => ({
    id: event.pointerId,
    x: event.clientX,
    y: event.clientY,
    time,
  });
  const down = (event: PointerEvent, element: HTMLElement, region: Region) => {
    if (!attached || document.hidden || event.button !== 0 || captured.has(event.pointerId)) return;
    const now = browser.performance.now();
    if (checkSuspension(now)) return;
    stopMomentum();
    if (captured.size >= 5) {
      reset();
      return;
    }
    // If capture fails, later up and cancel cannot be relied on.
    try {
      element.setPointerCapture(event.pointerId);
    } catch {
      reset();
      return;
    }
    const point = sample(event, now);
    if (region !== "pad" && !strips[region].down(point)) {
      element.releasePointerCapture(event.pointerId);
      return;
    }
    captured.set(event.pointerId, { element, region });
    if (region === "pad") emitPad(pad.down(point));
    else emitPad(pad.finishTap());
    schedule(now);
  };
  const pointer = (event: PointerEvent, end: boolean) => {
    const now = browser.performance.now();
    if (checkSuspension(now)) return;
    const capture = captured.get(event.pointerId);
    if (!capture) return;
    const { region } = capture;
    const point = sample(event, now);
    if (end) captured.delete(event.pointerId);
    if (region === "pad") emitPad(end ? pad.up(point) : pad.move(point));
    else emitScroll(end ? strips[region].up(point) : strips[region].move(point));
    schedule(now);
  };
  const lost = (event: PointerEvent) => {
    if (captured.has(event.pointerId)) reset();
  };
  const signal = { signal: listeners.signal };
  for (const region of ["pad", "vertical", "horizontal"] as const) {
    const element = surface.querySelector<HTMLElement>(`[data-region="${region}"]`);
    element?.addEventListener(
      "pointerdown",
      (event) => {
        down(event, element, region);
      },
      signal,
    );
    element?.addEventListener(
      "contextmenu",
      (event) => {
        event.preventDefault();
      },
      signal,
    );
  }
  surface
    .querySelector('[data-region="neutral"]')
    ?.addEventListener("pointerdown", stopMomentum, signal);
  surface.addEventListener(
    "pointermove",
    (event) => {
      pointer(event, false);
    },
    signal,
  );
  surface.addEventListener(
    "pointerup",
    (event) => {
      pointer(event, true);
    },
    signal,
  );
  surface.addEventListener("pointercancel", lost, signal);
  surface.addEventListener("lostpointercapture", lost, signal);
  document.addEventListener(
    "visibilitychange",
    () => {
      if (document.hidden) reset();
    },
    signal,
  );
  browser.addEventListener("pagehide", reset, signal);
  browser.addEventListener("blur", reset, signal);
  browser.addEventListener("resize", reset, signal);
  browser.addEventListener("orientationchange", reset, signal);
  const screen: Partial<Screen> = browser.screen;
  screen.orientation?.addEventListener("change", reset, signal);
  browser.visualViewport?.addEventListener("resize", reset, signal);
  return {
    reset,
    dispose() {
      if (!attached) return;
      listeners.abort();
      // Release synchronously before closing this sink. A later mount starts neutral.
      reset();
      attached = false;
    },
  };
}
