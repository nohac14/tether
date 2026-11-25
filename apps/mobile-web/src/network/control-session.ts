import { create, toBinary } from "@bufbuild/protobuf";
import type { SurfaceAction } from "../app/control-surface";
import type { ConnectionStatus } from "../app/shell";
import type { KeyboardBinding } from "../keyboard/editor";
import {
  type CapabilitySnapshot,
  ClientCapability,
  type ClientEnvelope,
  ClientEnvelopeSchema,
  ClientHelloSchema,
  type ClientLeavingReason,
  ClientLeavingSchema,
  ClientOperationBatchSchema,
  type ControlState,
  ControlStateKind,
  MouseButton,
  ProtocolErrorCode,
  RequestControlSchema,
} from "../protocol/generated/tether/control/v1/control_pb";
import { ApplicationHeartbeat, type HeartbeatClock } from "./heartbeat";
import { type FrameClock, OrderedInput } from "./ordered-input";
import {
  acceptWelcome,
  decodeServerMessage,
  MAX_COUNTER,
  MAX_MESSAGE_BYTES,
  validCapabilities,
  validControl,
  type Welcome,
} from "./server-messages";

export const SUBPROTOCOL = "tether.control.v1";
const HELLO_TIMEOUT = 5_000;
const SEND_BUFFER_LIMIT = 32 * 1024;
const buttonId = { left: MouseButton.LEFT, right: MouseButton.RIGHT, middle: MouseButton.MIDDLE };

export interface ControlView {
  showStatus(status: ConnectionStatus): void;
  setSurfaceSink(sink?: (action: SurfaceAction) => void): void;
  setKeyboardSink(sink?: KeyboardBinding): void;
  setControlRequest(request?: () => void, label?: "Take control" | "Reconnect"): void;
}

interface ConnectionEvents {
  opened(protocol: string): void;
  received(data: unknown): void;
  closed(code: number): void;
}

interface SocketPort {
  bufferedAmount(): number;
  send(bytes: Uint8Array<ArrayBuffer>): boolean;
  close(): void;
}

export interface ControlEnvironment extends FrameClock, HeartbeatClock {
  open(url: string, events: ConnectionEvents): SocketPort;
  online(): boolean;
}

interface Connection {
  socket?: SocketPort;
  helloTimer?: number;
  welcome?: Welcome;
  control?: ControlState;
  capabilities?: CapabilitySnapshot;
  heartbeat?: ApplicationHeartbeat;
  input?: OrderedInput;
  clientSequence: bigint;
  serverSequence: bigint;
  timestamp: bigint;
  requested: boolean;
}

export function controlUrl(origin: string): string | undefined {
  try {
    const url = new URL("/control", origin);
    if (url.protocol !== "http:") return;
    url.protocol = "ws:";
    return url.href;
  } catch {
    return undefined;
  }
}

/** Owns the only socket, ordered input queue and heartbeat for this page. */
export class MobileControlSession {
  private current: Connection | undefined;
  private pairingCheck?: (connected: boolean) => void;

  constructor(
    private readonly url: string | undefined,
    private readonly view: ControlView,
    private readonly environment: ControlEnvironment,
  ) {}

  start(): void {
    if (this.current) return;
    this.open();
  }

  /** A scanned QR first tries the normal authenticated connection. */
  tryExisting(signal: AbortSignal): Promise<boolean> {
    if (signal.aborted || !this.url || !this.environment.online()) return Promise.resolve(false);
    if (this.current?.welcome) return Promise.resolve(true);
    this.pause();
    return new Promise((resolve) => {
      const cancel = () => {
        this.pause();
      };
      this.pairingCheck = (connected) => {
        delete this.pairingCheck;
        signal.removeEventListener("abort", cancel);
        resolve(connected);
      };
      signal.addEventListener("abort", cancel, { once: true });
      this.start();
    });
  }

  pause(reason?: ClientLeavingReason): void {
    this.pairingCheck?.(false);
    this.view.setControlRequest();
    const current = this.current;
    if (current) {
      if (reason !== undefined && current.welcome) {
        this.send(current, {
          case: "clientLeaving",
          value: create(ClientLeavingSchema, { reason }),
        });
      }
      this.retire(current);
    }
  }

  offerReconnect(reason: "disconnected" | "computer-unavailable" | "network-unavailable"): void {
    this.view.showStatus(reason);
    this.view.setControlRequest(() => {
      this.start();
    }, "Reconnect");
  }

  private open(): void {
    if (!this.url) return;
    if (!this.environment.online()) {
      this.offerReconnect("network-unavailable");
      return;
    }
    const connection: Connection = {
      clientSequence: 0n,
      serverSequence: 0n,
      timestamp: 0n,
      requested: false,
    };
    this.current = connection;
    this.view.setControlRequest();
    this.view.showStatus("connecting");
    connection.helloTimer = this.environment.schedule(() => {
      this.failed(connection);
    }, HELLO_TIMEOUT);
    try {
      connection.socket = this.environment.open(this.url, {
        opened: (protocol) => {
          this.hello(connection, protocol);
        },
        received: (data) => {
          this.receive(connection, data);
        },
        closed: (code) => {
          this.failed(
            connection,
            code === 4001 ? "pairing-unavailable" : code === 4002 ? "incompatible" : undefined,
          );
        },
      });
    } catch {
      this.failed(connection);
    }
  }

  private hello(connection: Connection, protocol: string): void {
    if (this.current !== connection) return;
    if (protocol !== SUBPROTOCOL) {
      this.failed(connection, "incompatible");
      return;
    }
    if (
      !this.send(connection, {
        case: "clientHello",
        value: create(ClientHelloSchema, {
          protocolMajor: 1,
          buildIdentifier: "tether-web/0.1",
          clientDisplayName: "Phone",
          limits: {
            maximumEncodedMessageBytes: MAX_MESSAGE_BYTES,
            maximumOperationsPerBatch: 256,
            maximumTextOperationBytes: 4096,
          },
          capabilities: [
            ClientCapability.ORDERED_OPERATION_BATCHES,
            ClientCapability.APPLICATION_HEARTBEAT,
          ],
        }),
      })
    )
      this.failed(connection);
  }

  private receive(connection: Connection, data: unknown): void {
    if (this.current !== connection) return;
    const welcome = connection.welcome;
    const message = decodeServerMessage(
      data,
      welcome?.limits.maximumEncodedMessageBytes ?? MAX_MESSAGE_BYTES,
      connection.serverSequence,
    );
    if (!message || connection.serverSequence >= MAX_COUNTER) {
      this.failed(connection, "incompatible");
      return;
    }
    connection.serverSequence += 1n;
    if (message.payload.case === "protocolError") {
      const { code } = message.payload.value;
      this.failed(
        connection,
        code === ProtocolErrorCode.AUTHENTICATION_REQUIRED
          ? "pairing-unavailable"
          : code === ProtocolErrorCode.INCOMPATIBLE_VERSION
            ? "incompatible"
            : undefined,
      );
      return;
    }
    if (!welcome) {
      const accepted = acceptWelcome(message);
      if (!accepted) {
        this.failed(connection, "incompatible");
        return;
      }
      this.ready(connection, accepted);
      return;
    }
    const latest = connection.control;
    if (message.payload.case === "controlState") {
      const state = message.payload.value;
      if (
        !latest ||
        !validControl(state) ||
        state.revision <= latest.revision ||
        state.sessionEpoch < latest.sessionEpoch ||
        message.sessionEpoch !== state.sessionEpoch
      ) {
        this.failed(connection, "incompatible");
        return;
      }
      this.applyControl(connection, state);
    } else if (message.sessionEpoch !== latest?.sessionEpoch) {
      this.failed(connection, "incompatible");
    } else if (message.payload.case === "heartbeatPong") {
      connection.heartbeat?.acceptPong(message.payload.value);
    } else if (message.payload.case === "capabilitySnapshot") {
      const next = message.payload.value;
      if (
        !validCapabilities(next, welcome.limits) ||
        next.revision <= (connection.capabilities?.revision ?? 0n)
      ) {
        this.failed(connection, "incompatible");
      } else if (connection.input) {
        this.failed(connection);
      } else {
        connection.capabilities = next;
      }
    } else {
      this.failed(connection, "incompatible");
    }
  }

  private ready(connection: Connection, welcome: Welcome): void {
    if (connection.helloTimer !== undefined) this.environment.cancel(connection.helloTimer);
    delete connection.helloTimer;
    connection.welcome = welcome;
    this.pairingCheck?.(true);
    connection.capabilities = welcome.capabilities;
    connection.heartbeat = new ApplicationHeartbeat(
      this.environment,
      {
        send: (value) => this.send(connection, { case: "heartbeatPing", value }),
        stop: () => {
          this.failed(connection);
        },
      },
      welcome,
    );
    this.applyControl(connection, welcome.controlState);
  }

  private applyControl(connection: Connection, state: ControlState): void {
    const previous = connection.control;
    connection.control = state;
    const controlling = state.state === ControlStateKind.CONTROLLING;
    if (previous?.sessionEpoch !== state.sessionEpoch || !controlling) {
      this.clearInput(connection);
      connection.requested = false;
    }
    const limits = connection.welcome?.limits;
    const caps = connection.capabilities;
    if (controlling && limits && caps) {
      this.view.showStatus("controlling");
      this.view.setControlRequest();
      if (connection.input) return;
      const input = new OrderedInput(
        {
          bufferedAmount: () => connection.socket?.bufferedAmount() ?? SEND_BUFFER_LIMIT,
          writeBatch: (value, maximumBytes) =>
            this.write(connection, { case: "operationBatch", value }, maximumBytes),
          abortInput: () => {
            this.failed(connection);
          },
        },
        this.environment,
        limits,
      );
      connection.input = input;
      this.view.setSurfaceSink((action) => {
        if (surfaceAvailable(caps, action)) input.surface(action);
      });
      this.view.setKeyboardSink({
        textBytes: caps.textModes.length
          ? Math.min(limits.maximumTextOperationBytes, limits.maximumTextBytesPerBatch)
          : 0,
        backspace: caps.physicalKeysAvailable,
        send: (action) => {
          input.keyboard(action);
        },
      });
      return;
    }
    this.view.showStatus(
      state.state === ControlStateKind.SUSPENDED ? "control-unavailable" : "standby",
    );
    this.view.setControlRequest(
      state.state === ControlStateKind.STANDBY || state.state === ControlStateKind.NO_CONTROLLER
        ? () => {
            this.claim(connection);
          }
        : undefined,
    );
    if (!previous && state.state === ControlStateKind.NO_CONTROLLER) this.claim(connection);
  }

  private claim(connection: Connection): void {
    const state = connection.control;
    if (
      this.current !== connection ||
      connection.requested ||
      !state ||
      (state.state !== ControlStateKind.NO_CONTROLLER &&
        state.state !== ControlStateKind.STANDBY) ||
      state.sessionEpoch === 0n
    )
      return;
    connection.requested = true;
    this.view.setControlRequest();
    if (
      !this.send(connection, {
        case: "operationBatch",
        value: create(ClientOperationBatchSchema, {
          operations: [{ action: { case: "requestControl", value: create(RequestControlSchema) } }],
        }),
      })
    )
      this.failed(connection);
  }

  private send(connection: Connection, payload: ClientEnvelope["payload"]): boolean {
    if (!connection.socket || connection.socket.bufferedAmount() >= SEND_BUFFER_LIMIT) return false;
    return (
      this.write(
        connection,
        payload,
        connection.welcome?.limits.maximumEncodedMessageBytes ?? MAX_MESSAGE_BYTES,
      ) === "sent"
    );
  }

  private write(connection: Connection, payload: ClientEnvelope["payload"], maximumBytes: number) {
    const socket = connection.socket;
    const hello = payload.case === "clientHello";
    const clock = Math.floor(this.environment.now() * 1000);
    if (
      this.current !== connection ||
      !socket ||
      connection.clientSequence >= MAX_COUNTER ||
      !Number.isSafeInteger(clock) ||
      clock < 0 ||
      (!hello && !connection.control)
    )
      return "closed";
    const micros = BigInt(clock);
    const timestamp = micros >= connection.timestamp ? micros : connection.timestamp;
    const bytes = toBinary(
      ClientEnvelopeSchema,
      create(ClientEnvelopeSchema, {
        protocolMajor: 1,
        sequence: connection.clientSequence,
        sessionEpoch: hello ? 0n : (connection.control?.sessionEpoch ?? 0n),
        clientTimestampMicros: hello ? 0n : timestamp,
        payload,
      }),
    );
    if (bytes.byteLength > maximumBytes) return "oversized";
    if (!socket.send(bytes)) return "closed";
    connection.timestamp = timestamp;
    connection.clientSequence += 1n;
    return "sent";
  }

  private clearInput(connection: Connection): void {
    connection.input?.dispose();
    delete connection.input;
    this.view.setSurfaceSink();
    this.view.setKeyboardSink();
    this.view.setControlRequest();
  }

  private retire(connection: Connection): void {
    this.current = undefined;
    if (connection.helloTimer !== undefined) this.environment.cancel(connection.helloTimer);
    this.clearInput(connection);
    connection.heartbeat?.dispose();
    connection.socket?.close();
  }

  private failed(connection: Connection, terminal?: ConnectionStatus): void {
    if (this.current !== connection) return;
    this.retire(connection);
    if (this.pairingCheck) {
      this.pause();
      return;
    }
    if (terminal) {
      this.view.showStatus(terminal);
    } else {
      this.offerReconnect(
        this.environment.online() ? "computer-unavailable" : "network-unavailable",
      );
    }
  }
}

function surfaceAvailable(caps: CapabilitySnapshot, action: SurfaceAction): boolean {
  if (action.kind === "move") return caps.pointerAvailable;
  if (action.kind === "scroll")
    return action.axis === "vertical"
      ? caps.verticalScrollAvailable
      : caps.horizontalScrollAvailable;
  return (
    caps.mouseButtons.includes(buttonId[action.button]) &&
    (action.kind !== "click" || caps.atomicClickAvailable)
  );
}
