import { describe, expect, it } from "vitest";
import { openAppearance } from "./appearance";

describe("mobile appearance preferences", () => {
  it("defaults to the right strip and follows the system theme", () => {
    expect(openAppearance(undefined).initial).toEqual({
      version: 1,
      scrollSide: "right",
      theme: "system",
    });
  });

  it("reads and writes one bounded appearance record", () => {
    const values = new Map<string, string>();
    const storage = {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => values.set(key, value),
    };
    openAppearance(storage).save({ version: 1, scrollSide: "left", theme: "dark" });
    expect(openAppearance(storage).initial).toEqual({
      version: 1,
      scrollSide: "left",
      theme: "dark",
    });
    expect(values.size).toBe(1);
  });

  it.each([
    "{",
    '{"version":2,"scrollSide":"left","theme":"dark"}',
    '{"version":1,"scrollSide":"top","theme":"bright"}',
    "a".repeat(513),
  ])("falls back without replacing an unrecognized stored record", (raw) => {
    let calls = 0;
    const store = openAppearance({
      getItem: () => raw,
      setItem: () => {
        calls += 1;
      },
    });
    expect(store.initial.theme).toBe("system");
    store.save({ version: 1, scrollSide: "left", theme: "light" });
    expect(calls).toBe(0);
  });

  it("can render when storage is denied or exhausted", () => {
    const denied = openAppearance({
      getItem: () => {
        throw new Error("denied");
      },
      setItem: () => {
        throw new Error("unexpected write");
      },
    });
    expect(denied.initial.scrollSide).toBe("right");
    const exhausted = openAppearance({
      getItem: () => null,
      setItem: () => {
        throw new Error("quota");
      },
    });
    expect(() => {
      exhausted.save({ version: 1, scrollSide: "left", theme: "dark" });
    }).not.toThrow();
  });
});
