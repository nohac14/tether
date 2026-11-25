export interface ViewportBox {
  width: number;
  height: number;
  offsetTop: number;
  offsetLeft: number;
}

export function chooseViewport(
  visual: ViewportBox | null,
  width: number,
  height: number,
): ViewportBox {
  if (
    visual &&
    Number.isFinite(visual.width) &&
    visual.width > 0 &&
    Number.isFinite(visual.height) &&
    visual.height > 0 &&
    Number.isFinite(visual.offsetLeft) &&
    Number.isFinite(visual.offsetTop)
  ) {
    return {
      width: visual.width,
      height: visual.height,
      offsetLeft: Math.max(0, visual.offsetLeft),
      offsetTop: Math.max(0, visual.offsetTop),
    };
  }
  return {
    width: positiveDimension(width),
    height: positiveDimension(height),
    offsetLeft: 0,
    offsetTop: 0,
  };
}

function positiveDimension(value: number): number {
  return Number.isFinite(value) ? Math.max(1, value) : 1;
}

export function monitorViewport(root: HTMLElement): () => void {
  const visual = window.visualViewport;
  let scheduled: number | undefined;
  const render = () => {
    const box = chooseViewport(visual, window.innerWidth, window.innerHeight);
    root.style.setProperty("--viewport-width", `${String(box.width)}px`);
    root.style.setProperty("--viewport-height", `${String(box.height)}px`);
    root.style.setProperty("--viewport-top", `${String(box.offsetTop)}px`);
    root.style.setProperty("--viewport-left", `${String(box.offsetLeft)}px`);
  };
  const schedule = () => {
    scheduled ??= window.requestAnimationFrame(() => {
      scheduled = undefined;
      render();
    });
  };
  render();
  visual?.addEventListener("resize", schedule);
  visual?.addEventListener("scroll", schedule, { passive: true });
  window.addEventListener("resize", schedule);
  window.addEventListener("pageshow", schedule);
  return () => {
    visual?.removeEventListener("resize", schedule);
    visual?.removeEventListener("scroll", schedule);
    window.removeEventListener("resize", schedule);
    window.removeEventListener("pageshow", schedule);
    if (scheduled !== undefined) window.cancelAnimationFrame(scheduled);
  };
}
