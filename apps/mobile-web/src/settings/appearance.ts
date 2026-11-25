export type Theme = "system" | "light" | "dark";
export type ScrollSide = "left" | "right";

export interface Appearance {
  readonly version: 1;
  readonly theme: Theme;
  readonly scrollSide: ScrollSide;
}

const STORAGE_KEY = "tether.mobile.appearance";
const MAX_STORED_CHARS = 512;
const defaults: Appearance = Object.freeze({ version: 1, theme: "system", scrollSide: "right" });

export function isTheme(value: unknown): value is Theme {
  return value === "system" || value === "light" || value === "dark";
}

export function isScrollSide(value: unknown): value is ScrollSide {
  return value === "left" || value === "right";
}

function decode(raw: string): Appearance | undefined {
  if (raw.length > MAX_STORED_CHARS) return undefined;
  const parsed: unknown = JSON.parse(raw);
  if (
    !parsed ||
    typeof parsed !== "object" ||
    !("version" in parsed) ||
    parsed.version !== 1 ||
    !("theme" in parsed) ||
    !isTheme(parsed.theme) ||
    !("scrollSide" in parsed) ||
    !isScrollSide(parsed.scrollSide)
  ) {
    return undefined;
  }
  return { version: 1, theme: parsed.theme, scrollSide: parsed.scrollSide };
}

export function openAppearance(storage: Pick<Storage, "getItem" | "setItem"> | undefined): {
  initial: Appearance;
  save(preferences: Appearance): void;
} {
  let initial = defaults;
  let writable: typeof storage;
  try {
    const raw = storage?.getItem(STORAGE_KEY) ?? null;
    const decoded = raw === null ? defaults : decode(raw);
    if (decoded) {
      initial = decoded;
      writable = storage;
    }
  } catch {
    // Storage may be blocked or contain an unreadable record.
  }
  return {
    initial,
    save(preferences) {
      try {
        writable?.setItem(STORAGE_KEY, JSON.stringify(preferences));
      } catch {
        // The current selection still works for this page.
      }
    },
  };
}
