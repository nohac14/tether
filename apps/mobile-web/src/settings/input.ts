import type { SurfaceOptions } from "../app/control-surface";
import { DEFAULT_DRAG, type DragOptions } from "../gesture/touchpad";
import { DEFAULT_STRIP, type StripOptions } from "../scroll/strip";

interface InputPreferences extends Required<SurfaceOptions> {
  readonly version: 1;
}

const key = "tether.mobile.input";
const defaults: InputPreferences = {
  version: 1,
  drag: DEFAULT_DRAG,
  vertical: DEFAULT_STRIP,
  horizontal: DEFAULT_STRIP,
};

function dragOptions(value: unknown): DragOptions | undefined {
  if (
    typeof value !== "object" ||
    value === null ||
    !("longPress" in value) ||
    !("doubleTap" in value) ||
    typeof value.longPress !== "boolean" ||
    typeof value.doubleTap !== "boolean"
  ) {
    return undefined;
  }
  return { longPress: value.longPress, doubleTap: value.doubleTap };
}

function stripOptions(value: unknown): StripOptions | undefined {
  if (
    typeof value !== "object" ||
    value === null ||
    !("direction" in value) ||
    !("sensitivity" in value) ||
    !("momentum" in value)
  )
    return undefined;
  const { direction, sensitivity, momentum } = value;
  if (
    (direction !== "natural" && direction !== "traditional") ||
    typeof sensitivity !== "number" ||
    !Number.isFinite(sensitivity) ||
    sensitivity < 0.125 ||
    sensitivity > 8 ||
    (momentum !== "off" && momentum !== "low" && momentum !== "medium" && momentum !== "high")
  )
    return undefined;
  return { direction, sensitivity, momentum };
}

function decode(raw: string): InputPreferences | undefined {
  if (raw.length > 1024) return undefined;
  const value: unknown = JSON.parse(raw);
  if (
    typeof value !== "object" ||
    value === null ||
    !("version" in value) ||
    value.version !== 1 ||
    !("drag" in value) ||
    !("vertical" in value) ||
    !("horizontal" in value)
  )
    return undefined;
  const drag = dragOptions(value.drag);
  const vertical = stripOptions(value.vertical);
  const horizontal = stripOptions(value.horizontal);
  return drag && vertical && horizontal ? { version: 1, drag, vertical, horizontal } : undefined;
}

export function bindInputSettings(
  form: HTMLFormElement | null,
  storage: Pick<Storage, "getItem" | "setItem"> | undefined,
  change: (options: SurfaceOptions) => void,
): { current(): SurfaceOptions; dispose(): void } {
  let preferences = defaults;
  let writable: typeof storage;
  try {
    const raw = storage?.getItem(key) ?? null;
    const decoded = raw === null ? defaults : decode(raw);
    if (decoded) {
      preferences = decoded;
      writable = storage;
    }
  } catch {
    // Keep defaults with unreadable or unavailable storage.
  }

  const longPress = form?.querySelector<HTMLInputElement>("[name=long-press]");
  const doubleTap = form?.querySelector<HTMLInputElement>("[name=double-tap]");
  const setValue = (name: string, value: string | number) => {
    const control = form?.querySelector<HTMLInputElement | HTMLSelectElement>(`[name="${name}"]`);
    if (control) control.value = String(value);
  };
  if (longPress) longPress.checked = preferences.drag.longPress;
  if (doubleTap) doubleTap.checked = preferences.drag.doubleTap;
  for (const axis of ["vertical", "horizontal"] as const) {
    const { direction, sensitivity, momentum } = preferences[axis];
    setValue(`${axis}-direction`, direction);
    setValue(`${axis}-sensitivity`, sensitivity);
    setValue(`${axis}-momentum`, momentum);
  }

  const update = () => {
    if (!form) return;
    const data = new FormData(form);
    const getStrip = (axis: "vertical" | "horizontal") =>
      stripOptions({
        direction: data.get(`${axis}-direction`),
        sensitivity: Number(data.get(`${axis}-sensitivity`)),
        momentum: data.get(`${axis}-momentum`),
      });
    const vertical = getStrip("vertical");
    const horizontal = getStrip("horizontal");
    if (!vertical || !horizontal) return;
    preferences = {
      version: 1,
      drag: { longPress: data.has("long-press"), doubleTap: data.has("double-tap") },
      vertical,
      horizontal,
    };
    try {
      writable?.setItem(key, JSON.stringify(preferences));
    } catch {
      // The current selection still works for this page.
    }
    change(preferences);
  };
  const preventSubmit = (event: SubmitEvent) => {
    event.preventDefault();
  };
  form?.addEventListener("change", update);
  form?.addEventListener("submit", preventSubmit);
  return {
    current: () => preferences,
    dispose() {
      form?.removeEventListener("change", update);
      form?.removeEventListener("submit", preventSubmit);
    },
  };
}
