import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { beginPairing } from "./pairing";

const QR = "b".repeat(64);
const COMPLETION = "a".repeat(64);

function setup(hash = `#pair=${QR}`) {
  const states: string[] = [];
  const sequence: string[] = [];
  const fetch = vi.fn<typeof globalThis.fetch>();
  const replaceState = vi.fn(() => {
    sequence.push("scrub");
  });
  const browser = {
    location: { hash, pathname: "/", search: "" },
    history: { replaceState },
    fetch,
    setTimeout: (task: () => void, delay: number) => setTimeout(task, delay),
    clearTimeout: (id: number) => {
      clearTimeout(id);
    },
  } as unknown as NonNullable<Parameters<typeof beginPairing>[1]>;
  const start = (check?: (signal: AbortSignal) => Promise<boolean>) =>
    beginPairing(
      (state) => {
        states.push(state);
      },
      browser,
      check,
    );
  const initial = (body = JSON.stringify({ completion: COMPLETION })) =>
    new Response(body, {
      status: 202,
      headers: { "Content-Type": "application/json" },
    });
  return { states, sequence, start, replaceState, fetch, initial };
}

describe("mobile pairing", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("scrubs the QR before posting it once and waits for the PC to approve", async () => {
    const f = setup();
    f.fetch.mockImplementationOnce(() => {
      f.sequence.push("submit");
      return Promise.resolve(f.initial());
    });
    f.fetch.mockResolvedValueOnce(new Response(null, { status: 202 }));
    f.fetch.mockResolvedValueOnce(new Response(null, { status: 200 }));
    f.start();
    await vi.advanceTimersByTimeAsync(0);
    expect(f.sequence).toEqual(["scrub", "submit"]);
    expect(f.replaceState).toHaveBeenCalledWith(null, "", "/");
    expect(f.states).toEqual(["pairing"]);
    expect(f.fetch.mock.calls[0]).toEqual([
      "/pairing/request",
      expect.objectContaining({
        method: "POST",
        credentials: "same-origin",
        redirect: "error",
        cache: "no-store",
        body: JSON.stringify({ token: QR, deviceName: "Phone" }),
      }),
    ]);
    await vi.advanceTimersByTimeAsync(1_999);
    expect(f.fetch).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(1);
    expect(f.fetch).toHaveBeenCalledTimes(3);
    expect(f.fetch.mock.calls[2]).toEqual([
      "/pairing/complete",
      expect.objectContaining({ body: JSON.stringify({ completion: COMPLETION }) }),
    ]);
    expect(f.states).toEqual(["pairing", "paired"]);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("sends no requests for an absent or invalid fragment or a failed scrub", async () => {
    for (const hash of ["", "#manual", "#pair=bad", `#pair=${QR}&extra=true`]) {
      const f = setup(hash);
      f.start();
      expect(f.fetch).toHaveBeenCalledTimes(0);
      expect(f.states).toEqual(hash.startsWith("#pair=") ? ["pairing-unavailable"] : []);
    }
    const f = setup();
    f.replaceState.mockImplementationOnce(() => {
      throw new Error("History unavailable");
    });
    f.start();
    await vi.advanceTimersByTimeAsync(0);
    expect(f.fetch).toHaveBeenCalledTimes(0);
    expect(f.states).toEqual(["pairing-unavailable"]);
  });

  it("rejects unexpected fields and oversized completion bodies", async () => {
    for (const response of [
      JSON.stringify({ completion: COMPLETION, extra: true }),
      " ".repeat(4097),
    ]) {
      const f = setup();
      f.fetch.mockResolvedValueOnce(f.initial(response));
      f.start();
      await vi.advanceTimersByTimeAsync(0);
      expect(f.fetch).toHaveBeenCalledTimes(1);
      expect(f.states).toEqual(["pairing", "pairing-unavailable"]);
    }
  });

  it("scrubs a rescanned QR and keeps the existing credential without enrolling", async () => {
    const f = setup();
    f.start(() => Promise.resolve(true));
    await vi.advanceTimersByTimeAsync(0);
    expect(f.sequence).toEqual(["scrub"]);
    expect(f.fetch).not.toHaveBeenCalled();
    expect(f.states).toEqual(["reconnected"]);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("stops after denial or page cancellation without resubmitting the QR", async () => {
    for (const cancelled of [false, true]) {
      const f = setup();
      f.fetch.mockResolvedValueOnce(f.initial());
      f.fetch.mockResolvedValueOnce(new Response(null, { status: cancelled ? 202 : 410 }));
      const stop = f.start();
      await vi.advanceTimersByTimeAsync(0);
      if (cancelled) stop?.();
      await vi.advanceTimersByTimeAsync(3_000);
      expect(f.fetch).toHaveBeenCalledTimes(2);
      expect(f.states).toEqual(["pairing", "pairing-unavailable"]);
      expect(vi.getTimerCount()).toBe(0);
    }
  });

  it("aborts an unanswered request at ten seconds", async () => {
    const f = setup();
    f.fetch.mockImplementationOnce(
      (_input, options) =>
        new Promise((_, reject) => {
          options?.signal?.addEventListener(
            "abort",
            () => {
              reject(new Error("Request aborted"));
            },
            { once: true },
          );
        }),
    );
    f.start();
    await vi.advanceTimersByTimeAsync(10_000);
    expect(f.states).toEqual(["pairing", "pairing-unavailable"]);
    expect(f.fetch).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
  });
});
