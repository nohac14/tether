const SECRET = /^[0-9a-f]{64}$/;
const PREFIX = "#pair=";
const MAX_BODY_BYTES = 4096;
const POLL_MS = 2_000;
const PAIR_MS = 120_000;
const REQUEST_MS = 10_000;

type PairingStatus = "pairing" | "paired" | "pairing-unavailable";
type PairingBrowser = Pick<
  Window,
  "location" | "history" | "fetch" | "setTimeout" | "clearTimeout"
>;

/** Handles a QR link when the browser reuses an existing mobile tab. */
export function watchPairing(
  status: (state: PairingStatus) => void,
  browser: Window = window,
): () => void {
  let revision = 0;
  let stop: (() => void) | undefined;
  const open = () => {
    if (!browser.location.hash.startsWith(PREFIX)) return;
    revision += 1;
    const current = revision;
    stop?.();
    stop = beginPairing((value) => {
      if (revision === current) status(value);
    }, browser);
  };
  const pause = () => {
    stop?.();
  };
  browser.addEventListener("hashchange", open);
  browser.addEventListener("pagehide", pause);
  open();
  return () => {
    revision += 1;
    browser.removeEventListener("hashchange", open);
    browser.removeEventListener("pagehide", pause);
    pause();
  };
}

/** Scrubs the QR before the first request and retains proofs only in this page. */
export function beginPairing(
  status: (state: PairingStatus) => void,
  browser: PairingBrowser = window,
): (() => void) | undefined {
  const hash = browser.location.hash;
  if (!hash.startsWith(PREFIX)) return undefined;
  const token = hash.slice(PREFIX.length);
  try {
    browser.history.replaceState(
      null,
      "",
      `${browser.location.pathname}${browser.location.search}`,
    );
  } catch {
    status("pairing-unavailable");
    return undefined;
  }
  if (!SECRET.test(token)) {
    status("pairing-unavailable");
    return undefined;
  }

  const session = new AbortController();
  const deadline = browser.setTimeout(() => {
    session.abort();
  }, PAIR_MS);
  status("pairing");
  void exchange(browser, session.signal, token)
    .then((success) => {
      status(success && !session.signal.aborted ? "paired" : "pairing-unavailable");
    })
    .catch(() => {
      status("pairing-unavailable");
    })
    .finally(() => {
      browser.clearTimeout(deadline);
    });
  return () => {
    session.abort();
  };
}

async function exchange(
  browser: PairingBrowser,
  signal: AbortSignal,
  token: string,
): Promise<boolean> {
  const completion = await post(
    browser,
    signal,
    "/pairing/request",
    { token, deviceName: "Phone" },
    async (response) => {
      if (response.status !== 202 || response.headers.get("content-type") !== "application/json")
        return undefined;
      return readCompletion(response);
    },
  );
  if (!completion) return false;
  for (let attempt = 0; attempt < PAIR_MS / POLL_MS; attempt += 1) {
    const result = await post(
      browser,
      signal,
      "/pairing/complete",
      { completion },
      (response) => response.status,
    );
    if (result === 200) return true;
    if (result !== 202 && result !== 429) return false;
    await delay(browser, signal);
  }
  return false;
}

async function post<T>(
  browser: PairingBrowser,
  session: AbortSignal,
  path: "/pairing/request" | "/pairing/complete",
  payload: Record<string, string>,
  read: (response: Response) => T | Promise<T>,
): Promise<T> {
  session.throwIfAborted();
  const request = new AbortController();
  const abort = () => {
    request.abort();
  };
  const timer = browser.setTimeout(abort, REQUEST_MS);
  session.addEventListener("abort", abort, { once: true });
  try {
    const response = await browser.fetch(path, {
      method: "POST",
      credentials: "same-origin",
      cache: "no-store",
      redirect: "error",
      referrerPolicy: "no-referrer",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
      signal: request.signal,
    });
    try {
      return await read(response);
    } finally {
      if (response.body && !response.bodyUsed) void response.body.cancel().catch(() => undefined);
    }
  } finally {
    session.removeEventListener("abort", abort);
    browser.clearTimeout(timer);
  }
}

async function readCompletion(response: Response): Promise<string | undefined> {
  const reader = response.body?.getReader();
  if (!reader) return undefined;
  const bytes = new Uint8Array(MAX_BODY_BYTES);
  let count = 0;
  try {
    for (;;) {
      const { value, done } = await reader.read();
      if (done) break;
      if (value.length === 0 || value.length > bytes.length - count) return undefined;
      bytes.set(value, count);
      count += value.length;
    }
    const result: unknown = JSON.parse(
      new TextDecoder("utf-8", { fatal: true }).decode(bytes.subarray(0, count)),
    );
    if (
      typeof result === "object" &&
      result !== null &&
      "completion" in result &&
      Object.keys(result).length === 1 &&
      typeof result.completion === "string" &&
      SECRET.test(result.completion)
    ) {
      return result.completion;
    }
    return undefined;
  } finally {
    void reader.cancel().catch(() => undefined);
  }
}

function delay(browser: PairingBrowser, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    signal.throwIfAborted();
    const timer = browser.setTimeout(() => {
      signal.removeEventListener("abort", abort);
      resolve();
    }, POLL_MS);
    const abort = () => {
      browser.clearTimeout(timer);
      reject(new Error("Pairing ended"));
    };
    signal.addEventListener("abort", abort, { once: true });
  });
}
