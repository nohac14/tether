import { isScrollSide, isTheme, openAppearance } from "../settings/appearance";
import { bindInputSettings } from "../settings/input";
import { mountKeyboard, type KeyboardBinding } from "../keyboard/editor";
import { attachControlSurface, type SurfaceAction } from "./control-surface";
import { monitorViewport } from "./viewport";

export type ConnectionStatus =
  | "disconnected"
  | "pairing"
  | "paired"
  | "pairing-unavailable"
  | "connecting"
  | "reconnecting"
  | "computer-unavailable"
  | "network-unavailable"
  | "control-unavailable"
  | "standby"
  | "controlling"
  | "incompatible";

const statusText: Record<ConnectionStatus, string> = {
  disconnected: "Disconnected",
  pairing: "Waiting for approval on the PC",
  paired: "Pairing approved",
  "pairing-unavailable": "Scan a new pairing QR code",
  connecting: "Connecting",
  reconnecting: "Reconnecting",
  "computer-unavailable": "Can’t connect to the PC. Scan a new pairing QR code if needed.",
  "network-unavailable": "Network unavailable",
  "control-unavailable": "Connected · Input paused on the PC",
  standby: "Connected · Standby",
  controlling: "Connected · In control",
  incompatible: "Incompatible desktop",
};

export function mountShell(root: HTMLElement): {
  showStatus(status: ConnectionStatus): void;
  setSurfaceSink(sink?: (action: SurfaceAction) => void): void;
  setKeyboardSink(sink?: KeyboardBinding): void;
  setControlRequest(request?: () => void, label?: "Take control" | "Retry"): void;
  dispose(): void;
} {
  const status = root.querySelector("#status");
  const requestButton = root.querySelector<HTMLButtonElement>("#control-request");
  const keyboard = mountKeyboard(root);
  const surface = root.querySelector<HTMLElement>(".surface");
  const side = root.querySelector<HTMLSelectElement>("#strip-side");
  const theme = root.querySelector<HTMLSelectElement>("#theme");
  const storage = browserStorage();
  const store = openAppearance(storage);
  let connection: ConnectionStatus = "disconnected";
  let sink: ((action: SurfaceAction) => void) | undefined;
  let controller: ReturnType<typeof attachControlSurface> | undefined;
  let disposed = false;
  let requestControl: (() => void) | undefined;
  const request = () => {
    requestControl?.();
  };
  requestButton?.addEventListener("click", request);
  const stopSurface = () => {
    const previous = controller;
    controller = undefined;
    surface?.setAttribute("aria-disabled", "true");
    previous?.dispose();
  };
  const refreshSurface = () => {
    stopSurface();
    if (!disposed && connection === "controlling" && surface && sink && !controller) {
      controller = attachControlSurface(surface, sink, input.current());
      surface.setAttribute("aria-disabled", "false");
    }
  };
  const input = bindInputSettings(
    root.querySelector<HTMLFormElement>(".settings-panel"),
    storage,
    refreshSurface,
  );
  let preferences = store.initial;
  const showAppearance = () => {
    root.dataset.scrollSide = preferences.scrollSide;
    root.dataset.theme = preferences.theme;
    if (side) side.value = preferences.scrollSide;
    if (theme) theme.value = preferences.theme;
  };
  showAppearance();
  const updateSide = () => {
    if (!isScrollSide(side?.value)) return;
    controller?.reset();
    preferences = { ...preferences, scrollSide: side.value };
    showAppearance();
    store.save(preferences);
  };
  const updateTheme = () => {
    if (!isTheme(theme?.value)) return;
    preferences = { ...preferences, theme: theme.value };
    showAppearance();
    store.save(preferences);
  };
  side?.addEventListener("change", updateSide);
  theme?.addEventListener("change", updateTheme);
  const stopViewport = monitorViewport(root);
  return {
    showStatus(next) {
      if (disposed) return;
      connection = next;
      if (status) status.textContent = statusText[next];
      if (next !== "controlling") {
        sink = undefined;
        stopSurface();
        keyboard.setOwner();
      }
    },
    setSurfaceSink(next) {
      if (disposed || connection !== "controlling" || sink === next) return;
      sink = next;
      refreshSurface();
    },
    setKeyboardSink(next) {
      if (!disposed && connection === "controlling") keyboard.setOwner(next);
    },
    setControlRequest(next, label = "Take control") {
      if (disposed) return;
      requestControl = next;
      if (requestButton) {
        requestButton.hidden = next === undefined;
        requestButton.textContent = label;
      }
    },
    dispose() {
      disposed = true;
      sink = undefined;
      stopSurface();
      keyboard.dispose();
      input.dispose();
      side?.removeEventListener("change", updateSide);
      theme?.removeEventListener("change", updateTheme);
      requestButton?.removeEventListener("click", request);
      stopViewport();
    },
  };
}

function browserStorage(): Storage | undefined {
  try {
    return window.localStorage;
  } catch {
    return undefined;
  }
}
