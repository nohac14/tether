import { mountShell } from "./app/shell";
import { ClientLeavingReason } from "./protocol/generated/tether/control/v1/control_pb";
import { controlUrl, MobileControlSession, SUBPROTOCOL } from "./network/control-session";
import { watchPairing } from "./network/pairing";

const root = document.querySelector<HTMLElement>("#root");
if (root) {
  const shell = mountShell(root);
  const control = new MobileControlSession(controlUrl(location.origin), shell, {
    now: () => performance.now(),
    schedule: (callback, delay) => window.setTimeout(callback, delay),
    cancel: (timer) => {
      window.clearTimeout(timer);
    },
    requestFrame: (callback) => window.requestAnimationFrame(callback),
    cancelFrame: (frame) => {
      window.cancelAnimationFrame(frame);
    },
    online: () => navigator.onLine,
    open(url, events) {
      const socket = new WebSocket(url, SUBPROTOCOL);
      socket.binaryType = "arraybuffer";
      socket.onopen = () => {
        events.opened(socket.protocol);
      };
      socket.onmessage = (event: MessageEvent<unknown>) => {
        events.received(event.data);
      };
      socket.onclose = (event) => {
        events.closed(event.code);
      };
      return {
        bufferedAmount: () => socket.bufferedAmount,
        send(bytes) {
          if (socket.readyState !== WebSocket.OPEN) return false;
          try {
            socket.send(bytes);
            return true;
          } catch {
            return false;
          }
        },
        close() {
          socket.onopen = null;
          socket.onmessage = null;
          socket.onclose = null;
          socket.close();
        },
      };
    },
  });
  let pairingBlocksControl = location.hash.startsWith("#pair=");
  let started = false;
  const startControl = () => {
    if (!started && !pairingBlocksControl && !document.hidden) {
      started = true;
      control.start();
    }
  };
  const pauseControl = () => {
    control.pause(ClientLeavingReason.PAGE_HIDE);
    if (started && !pairingBlocksControl) control.offerReconnect("disconnected");
  };
  const visibility = () => {
    if (document.hidden) pauseControl();
    else startControl();
  };
  const offline = () => {
    control.pause();
    if (!pairingBlocksControl) control.offerReconnect("network-unavailable");
  };
  const disposePairing = watchPairing(
    (status) => {
      if (status === "reconnected") {
        pairingBlocksControl = false;
        started = true;
        return;
      }
      pairingBlocksControl = status !== "paired";
      if (pairingBlocksControl) control.pause();
      else started = false;
      shell.showStatus(status);
      startControl();
    },
    window,
    (signal) => control.tryExisting(signal),
  );
  startControl();
  document.addEventListener("visibilitychange", visibility);
  window.addEventListener("pageshow", startControl);
  window.addEventListener("offline", offline);
  window.addEventListener("pagehide", (event) => {
    pauseControl();
    if (!event.persisted) {
      disposePairing();
      document.removeEventListener("visibilitychange", visibility);
      window.removeEventListener("pageshow", startControl);
      window.removeEventListener("offline", offline);
      shell.dispose();
    }
  });
}
