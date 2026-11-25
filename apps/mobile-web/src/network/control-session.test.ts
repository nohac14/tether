import { create, fromBinary, toBinary } from "@bufbuild/protobuf";
import { describe, expect, it } from "vitest";
import type { SurfaceAction } from "../app/control-surface";
import type { ConnectionStatus } from "../app/shell";
import type { KeyboardBinding } from "../keyboard/editor";
import {
  ClientEnvelopeSchema,
  ClientLeavingReason,
  type ControlState,
  ControlStateKind,
  ControlStateSchema,
  MouseButton,
  PermissionState,
  type ServerEnvelope,
  ServerEnvelopeSchema,
  TextMode,
} from "../protocol/generated/tether/control/v1/control_pb";
import {
  type ControlEnvironment,
  controlUrl,
  MobileControlSession,
  SUBPROTOCOL,
} from "./control-session";

const bounds = {
  maximumEncodedMessageBytes: 65_536,
  maximumOperationsPerBatch: 256,
  maximumTextOperationBytes: 4_096,
  maximumTextBytesPerBatch: 16_384,
  maximumAbsoluteFixedDelta: 8_388_608,
  maximumAtomicClickCount: 2,
};

function control(state: ControlStateKind, epoch = 3n, revision = 1n) {
  return create(ControlStateSchema, {
    state,
    revision,
    sessionEpoch: epoch,
    thisClientControls: state === ControlStateKind.CONTROLLING,
  });
}

function setup() {
  const status: ConnectionStatus[] = [];
  const sockets: FakeSocket[] = [];
  let request: (() => void) | undefined;
  let surface: ((value: SurfaceAction) => void) | undefined;
  let keyboard: KeyboardBinding | undefined;
  let online = true;
  let now = 0;
  let next = 0;
  const timers = new Map<number, { at: number; task: () => void }>();
  const schedule = (task: () => void, delay: number) => {
    next += 1;
    timers.set(next, { at: now + delay, task });
    return next;
  };
  type Events = Parameters<ControlEnvironment["open"]>[1];
  class FakeSocket {
    readonly writes: Uint8Array[] = [];
    closed = false;
    buffered = 0;
    constructor(
      readonly url: string,
      readonly events: Events,
    ) {}
    open() {
      this.events.opened(SUBPROTOCOL);
    }
    read(sequence: bigint, payload: ServerEnvelope["payload"], sessionEpoch = 0n) {
      this.events.received(
        toBinary(
          ServerEnvelopeSchema,
          create(ServerEnvelopeSchema, {
            protocolMajor: 1,
            sequence,
            payload,
            sessionEpoch,
          }),
        ).buffer,
      );
    }
    closeFromServer(code: number) {
      this.events.closed(code);
    }
    bufferedAmount() {
      return this.buffered;
    }
    send(bytes: Uint8Array) {
      if (this.closed) return false;
      this.writes.push(bytes);
      return true;
    }
    close() {
      this.closed = true;
    }
    envelopes() {
      return this.writes.map((bytes) => fromBinary(ClientEnvelopeSchema, bytes));
    }
    greet(state: ControlState, active = true, identifierLength = 16) {
      this.read(0n, {
        case: "serverHello",
        value: {
          $typeName: "tether.control.v1.ServerHello",
          selectedProtocolMajor: 1,
          selectedProtocolMinor: 0,
          connectionIdentifier: new Uint8Array(identifierLength),
          heartbeatIntervalMillis: 1000,
          heartbeatTimeoutMillis: 4000,
          limits: { $typeName: "tether.control.v1.ProtocolLimits", ...bounds },
          capabilities: {
            $typeName: "tether.control.v1.CapabilitySnapshot",
            revision: 1n,
            permissionState: PermissionState.NOT_REQUIRED,
            userAuthorizationRequired: false,
            backendInitialized: true,
            physicalKeysAvailable: active,
            pointerAvailable: active,
            verticalScrollAvailable: active,
            horizontalScrollAvailable: active,
            atomicClickAvailable: active,
            mediaCommands: [],
            logicalCommands: [],
            mouseButtons: active ? [MouseButton.LEFT] : [],
            textModes: active ? [TextMode.NATIVE_UNICODE_BEST_EFFORT] : [],
            limits: { $typeName: "tether.control.v1.ProtocolLimits", ...bounds },
          },
          controlState: state,
        },
      });
    }
  }
  const session = new MobileControlSession(
    controlUrl("http://[fd00::1]:54321"),
    {
      showStatus(value) {
        status.push(value);
      },
      setSurfaceSink(value) {
        surface = value;
      },
      setKeyboardSink(value) {
        keyboard = value;
      },
      setControlRequest(value) {
        request = value;
      },
    },
    {
      now: () => now,
      schedule,
      cancel(timer) {
        timers.delete(timer);
      },
      requestFrame: (task) => schedule(task, 16),
      cancelFrame(frame) {
        timers.delete(frame);
      },
      random: () => 1,
      online: () => online,
      open(url, events) {
        const socket = new FakeSocket(url, events);
        sockets.push(socket);
        return socket;
      },
    },
  );
  function advance(duration: number) {
    const end = now + duration;
    for (;;) {
      const earliest = [...timers.entries()].sort((a, b) => a[1].at - b[1].at)[0];
      if (!earliest || earliest[1].at > end) break;
      const [key, timer] = earliest;
      timers.delete(key);
      now = timer.at;
      timer.task();
    }
    now = end;
  }
  return {
    sockets,
    socket(index: number) {
      const socket = sockets[index];
      if (!socket) throw new Error("Missing test socket");
      return socket;
    },
    status,
    session,
    advance,
    get request() {
      return request;
    },
    get surface() {
      return surface;
    },
    get keyboard() {
      return keyboard;
    },
    setOnline(value: boolean) {
      online = value;
    },
  };
}

describe("mobile control connection", () => {
  it("uses the same authority with a fixed WebSocket route", () => {
    expect(controlUrl("http://192.0.2.12:54321")).toBe("ws://192.0.2.12:54321/control");
    expect(controlUrl("http://[fd00::1]:54321")).toBe("ws://[fd00::1]:54321/control");
    expect(controlUrl("null")).toBeUndefined();
  });

  it("claims a free PC, then shares sequence and epoch across input and heartbeat", () => {
    const f = setup();
    f.session.start();
    const socket = f.socket(0);
    socket.open();
    expect(f.surface).toBeUndefined();
    expect(socket.envelopes()[0]).toMatchObject({
      sequence: 0n,
      sessionEpoch: 0n,
      payload: { case: "clientHello" },
    });
    socket.greet(control(ControlStateKind.NO_CONTROLLER));
    expect(socket.envelopes()[1]).toMatchObject({
      sequence: 1n,
      sessionEpoch: 3n,
      payload: {
        case: "operationBatch",
        value: { operations: [{ action: { case: "requestControl" } }] },
      },
    });
    expect(f.surface).toBeUndefined();
    socket.read(
      1n,
      { case: "controlState", value: control(ControlStateKind.CONTROLLING, 4n, 2n) },
      4n,
    );
    expect(f.status.at(-1)).toBe("controlling");
    f.surface?.({ kind: "move", dx: 2, dy: 3 });
    f.surface?.({ kind: "click", button: "left", count: 1 });
    f.keyboard?.send({ kind: "insert", text: "猫" });
    f.advance(1_000);
    expect(
      socket
        .envelopes()
        .slice(2)
        .map((message) => [message.sequence, message.sessionEpoch, message.payload.case]),
    ).toEqual([
      [2n, 4n, "operationBatch"],
      [3n, 4n, "operationBatch"],
      [4n, 4n, "heartbeatPing"],
    ]);
    socket.closeFromServer(1006);
    expect(f.surface).toBeUndefined();
    expect(f.keyboard).toBeUndefined();
    f.advance(250);
    const next = f.socket(1);
    next.open();
    expect(next.envelopes().map((message) => message.payload.case)).toEqual(["clientHello"]);
  });

  it("leaves another controller alone until the standby explicitly requests control", () => {
    const f = setup();
    f.session.start();
    const socket = f.socket(0);
    socket.open();
    socket.greet(control(ControlStateKind.STANDBY));
    expect(f.status.at(-1)).toBe("standby");
    expect(socket.writes).toHaveLength(1);
    f.request?.();
    expect(socket.writes).toHaveLength(2);
    expect(f.request).toBeUndefined();
    socket.read(
      1n,
      { case: "controlState", value: control(ControlStateKind.CONTROLLING, 4n, 2n) },
      4n,
    );
    f.session.pause(ClientLeavingReason.PAGE_HIDE);
    expect(socket.envelopes().at(-1)?.payload.case).toBe("clientLeaving");
    expect(socket.closed).toBe(true);
    expect(f.surface).toBeUndefined();
    f.advance(10_000);
    expect(f.sockets).toHaveLength(1);
  });

  it("rejects malformed hello and stops on an explicit authorization failure", () => {
    const f = setup();
    f.session.start();
    const socket = f.socket(0);
    socket.open();
    socket.greet(control(ControlStateKind.NO_CONTROLLER), true, 15);
    expect(f.status.at(-1)).toBe("incompatible");
    f.advance(10_000);
    expect(f.sockets).toHaveLength(1);
    f.session.start();
    f.sockets[1]?.closeFromServer(4001);
    expect(f.status.at(-1)).toBe("pairing-unavailable");
    expect(f.surface).toBeUndefined();
  });

  it("bounds repeated failures and allows the user to restart attempts", () => {
    const f = setup();
    f.session.start();
    for (const wait of [250, 500, 1000, 2000, 4000]) {
      f.sockets.at(-1)?.closeFromServer(1006);
      f.advance(wait);
    }
    expect(f.sockets).toHaveLength(6);
    f.sockets.at(-1)?.closeFromServer(1006);
    f.advance(10_000);
    expect(f.sockets).toHaveLength(6);
    expect(f.status.at(-1)).toBe("computer-unavailable");
    f.setOnline(false);
    f.request?.();
    expect(f.status.at(-1)).toBe("network-unavailable");
  });
});
