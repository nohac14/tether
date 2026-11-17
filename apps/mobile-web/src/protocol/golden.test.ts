import { create, fromBinary, toBinary } from "@bufbuild/protobuf";
import type { MessageInitShape } from "@bufbuild/protobuf";
import { describe, expect, it } from "vitest";
import corpus from "../../../../tests/fixtures/protocol/v1-golden.json";
import {
  ClientEnvelopeSchema,
  ClientCapability,
  ClientLeavingReason,
  ControlStateKind,
  LogicalCommand,
  MediaCommand,
  MediaCommandActionSchema,
  MouseButton,
  NumericContextKind,
  PermissionState,
  ProtocolErrorCode,
  ScrollAxis,
  ServerEnvelopeSchema,
  TextEditCommand,
  TextMode,
  TransitionState,
} from "./generated/tether/control/v1/control_pb.js";

type ClientInit = MessageInitShape<typeof ClientEnvelopeSchema>;
type ServerInit = MessageInitShape<typeof ServerEnvelopeSchema>;
type Operation = NonNullable<
  Extract<NonNullable<ClientInit["payload"]>, { case: "operationBatch" }>["value"]["operations"]
>[number];

const limits = {
  maximumEncodedMessageBytes: 65536,
  maximumOperationsPerBatch: 256,
  maximumTextOperationBytes: 4096,
  maximumTextBytesPerBatch: 16384,
  maximumAbsoluteFixedDelta: 8388608,
  maximumAtomicClickCount: 2,
};
const capabilities = {
  revision: 1n,
  backendInitialized: true,
  permissionState: PermissionState.NOT_REQUIRED,
  userAuthorizationRequired: false,
  pointerAvailable: true,
  physicalKeysAvailable: true,
  verticalScrollAvailable: true,
  horizontalScrollAvailable: true,
  atomicClickAvailable: true,
  mouseButtons: [
    MouseButton.LEFT,
    MouseButton.RIGHT,
    MouseButton.MIDDLE,
    MouseButton.BACK,
    MouseButton.FORWARD,
  ],
  textModes: [
    TextMode.ACTIVE_LAYOUT_MAPPABLE,
    TextMode.NATIVE_UNICODE_BEST_EFFORT,
    TextMode.EI_UTF8,
  ],
  // Current domain set order: Find precedes Save. Lists have no wire sort policy.
  logicalCommands: [
    LogicalCommand.COPY,
    LogicalCommand.CUT,
    LogicalCommand.PASTE_SHORTCUT,
    LogicalCommand.UNDO,
    LogicalCommand.REDO,
    LogicalCommand.SELECT_ALL,
    LogicalCommand.FIND,
    LogicalCommand.SAVE,
    LogicalCommand.CLOSE_TAB,
    LogicalCommand.SWITCH_APPLICATION,
    LogicalCommand.TASK_MANAGER_EQUIVALENT,
  ],
  mediaCommands: [
    MediaCommand.PLAY_PAUSE,
    MediaCommand.NEXT_TRACK,
    MediaCommand.PREVIOUS_TRACK,
    MediaCommand.VOLUME_UP,
    MediaCommand.VOLUME_DOWN,
    MediaCommand.MUTE,
  ],
  limits,
};
const control = {
  revision: 1n,
  state: ControlStateKind.CONTROLLING,
  sessionEpoch: 7n,
  thisClientControls: true,
};
const pointer: Operation = {
  action: { case: "pointerMove", value: { deltaX1024: 1024, deltaY1024: -512 } },
};
const text = (value: string): Operation => ({
  action: { case: "textInsertion", value: { text: value } },
});
function client(payload: ClientInit["payload"]): ClientInit {
  if (payload === undefined) throw new Error("synthetic payload required");
  return {
    protocolMajor: 1,
    sequence: 1n,
    sessionEpoch: 7n,
    clientTimestampMicros: 100n,
    payload,
  };
}
function batch(operations: Operation[]): ClientInit {
  return client({ case: "operationBatch", value: { operations } });
}
function server(payload: ServerInit["payload"]): ServerInit {
  if (payload === undefined) throw new Error("synthetic payload required");
  return { protocolMajor: 1, sequence: 1n, sessionEpoch: 7n, payload };
}

// Independently constructed expected values, not parsed from the Rust messages.
function messages() {
  const clients = new Map<string, ClientInit>();
  const servers = new Map<string, ServerInit>();
  clients.set("client-hello", {
    protocolMajor: 1,
    payload: {
      case: "clientHello",
      value: {
        protocolMajor: 1,
        minimumProtocolMinor: 0,
        maximumProtocolMinor: 5,
        buildIdentifier: "test-1",
        clientDisplayName: "Untrusted phone",
        capabilities: [
          ClientCapability.ORDERED_OPERATION_BATCHES,
          ClientCapability.APPLICATION_HEARTBEAT,
          ClientCapability.COMPOSE_TEXT,
        ],
        limits,
      },
    },
  });
  servers.set("server-hello", {
    protocolMajor: 1,
    payload: {
      case: "serverHello",
      value: {
        selectedProtocolMajor: 1,
        connectionIdentifier: new Uint8Array(16).fill(0x42),
        heartbeatIntervalMillis: 1000,
        heartbeatTimeoutMillis: 4000,
        limits,
        capabilities,
        controlState: control,
      },
    },
  });
  clients.set("pointer", batch([pointer]));
  clients.set(
    "button-transitions",
    batch([
      {
        action: {
          case: "mouseButtonTransition",
          value: { button: MouseButton.LEFT, state: TransitionState.DOWN },
        },
      },
      pointer,
      {
        action: {
          case: "mouseButtonTransition",
          value: { button: MouseButton.LEFT, state: TransitionState.UP },
        },
      },
    ]),
  );
  clients.set(
    "key-transitions",
    batch(
      [TransitionState.DOWN, TransitionState.UP].map((state) => ({
        action: {
          case: "physicalKeyTransition",
          value: { key: { usagePage: 7, usageId: 4 }, state },
        },
      })),
    ),
  );
  clients.set("text-unicode", batch([text("synthetic 🙂e\u{301}\n日本")]));
  clients.set("request-control", batch([{ action: { case: "requestControl", value: {} } }]));
  clients.set("relinquish-control", batch([{ action: { case: "relinquishControl", value: {} } }]));
  clients.set("release-all", batch([{ action: { case: "releaseAll", value: {} } }]));
  for (const axis of [ScrollAxis.VERTICAL, ScrollAxis.HORIZONTAL]) {
    clients.set(
      `scroll-${String(axis)}`,
      batch([{ action: { case: "scroll", value: { axis, delta1024: -1025 } } }]),
    );
  }
  for (const button of [
    MouseButton.LEFT,
    MouseButton.RIGHT,
    MouseButton.MIDDLE,
    MouseButton.BACK,
    MouseButton.FORWARD,
  ]) {
    clients.set(
      `click-${String(button)}`,
      batch([{ action: { case: "mouseButtonClick", value: { button, clickCount: 2 } } }]),
    );
  }
  for (const command of [
    TextEditCommand.DELETE_BACKWARD,
    TextEditCommand.DELETE_FORWARD,
    TextEditCommand.INSERT_LINE_BREAK,
  ]) {
    clients.set(
      `edit-${String(command)}`,
      batch([{ action: { case: "textEdit", value: { command, repeatCount: 2 } } }]),
    );
  }
  for (const command of capabilities.logicalCommands) {
    clients.set(
      `logical-${String(command)}`,
      batch([{ action: { case: "logicalCommand", value: { command } } }]),
    );
  }
  for (const command of capabilities.mediaCommands) {
    clients.set(
      `media-${String(command)}`,
      batch([{ action: { case: "mediaCommand", value: { command } } }]),
    );
  }
  clients.set(
    "heartbeat-ping",
    client({ case: "heartbeatPing", value: { heartbeatIdentifier: 42n } }),
  );
  clients.set("full-width-counters", {
    ...client({ case: "heartbeatPing", value: { heartbeatIdentifier: 18446744073709551615n } }),
    sequence: 18446744073709551615n,
    sessionEpoch: 18446744073709551615n,
    clientTimestampMicros: 18446744073709551615n,
  });
  for (const reason of [
    ClientLeavingReason.USER_DISCONNECT,
    ClientLeavingReason.PAGE_HIDE,
    ClientLeavingReason.NAVIGATION,
  ]) {
    clients.set(`leaving-${String(reason)}`, client({ case: "clientLeaving", value: { reason } }));
  }
  servers.set(
    "heartbeat-pong",
    server({ case: "heartbeatPong", value: { heartbeatIdentifier: 42n } }),
  );
  servers.set("capability-snapshot", server({ case: "capabilitySnapshot", value: capabilities }));
  servers.set(
    "capability-without-atomic-click",
    server({
      case: "capabilitySnapshot",
      value: { ...capabilities, atomicClickAvailable: false },
    }),
  );
  servers.set("control-state", server({ case: "controlState", value: control }));
  for (const code of [
    ProtocolErrorCode.MALFORMED_MESSAGE,
    ProtocolErrorCode.MESSAGE_TOO_LARGE,
    ProtocolErrorCode.INCOMPATIBLE_VERSION,
    ProtocolErrorCode.UNKNOWN_OR_UNSUPPORTED_OPERATION,
    ProtocolErrorCode.AUTHENTICATION_REQUIRED,
    ProtocolErrorCode.NOT_CONTROLLER,
    ProtocolErrorCode.STALE_EPOCH,
    ProtocolErrorCode.INVALID_SEQUENCE,
    ProtocolErrorCode.CAPABILITY_UNAVAILABLE,
    ProtocolErrorCode.RATE_LIMITED,
    ProtocolErrorCode.QUEUE_SATURATED,
    ProtocolErrorCode.BACKEND_UNAVAILABLE,
    ProtocolErrorCode.SERVER_SUSPENDING,
    ProtocolErrorCode.INTERNAL_FAILURE,
  ]) {
    servers.set(`error-${String(code)}`, server({ case: "protocolError", value: { code } }));
  }
  servers.set(
    "error-optional-presence",
    server({
      case: "protocolError",
      value: {
        code: ProtocolErrorCode.INCOMPATIBLE_VERSION,
        supportedMinimumMinor: 0,
        supportedMaximumMinor: 0,
        numericContext: { kind: NumericContextKind.MESSAGE_BYTES, observed: 65537n, limit: 65536n },
      },
    }),
  );
  clients.set("text-maximum", batch([text("é".repeat(2048))]));
  clients.set("text-oversize", batch([text("x".repeat(4097))]));
  clients.set("batch-maximum", batch(Array<Operation>(256).fill(pointer)));
  clients.set("batch-oversize", batch(Array<Operation>(257).fill(pointer)));
  clients.set("aggregate-text-maximum", batch(Array<Operation>(4).fill(text("x".repeat(4096)))));
  clients.set("aggregate-text-oversize", batch(Array<Operation>(5).fill(text("x".repeat(4096)))));
  clients.set("incompatible-major", { ...batch([pointer]), protocolMajor: 2 });
  clients.set("incompatible-minor", { ...batch([pointer]), protocolMinor: 1 });
  for (const size of [65536, 65537]) {
    clients.set(
      `message-${String(size)}`,
      client({ case: "clientLeaving", value: { reason: ClientLeavingReason.USER_DISCONNECT } }),
    );
  }
  clients.set(
    "unknown-enum",
    batch([
      {
        action: {
          case: "mediaCommand",
          value: fromBinary(MediaCommandActionSchema, new Uint8Array([0x08, 0xe7, 0x07])),
        },
      },
    ]),
  );
  for (const id of ["unknown-nested-field", "unknown-union-field", "duplicate-sequence"]) {
    clients.set(id, batch([pointer]));
  }
  const ping = clients.get("heartbeat-ping");
  if (ping === undefined) throw new Error("missing synthetic heartbeat case");
  clients.set("duplicate-payload", ping);
  return { clients, servers };
}

function bytes(hex: string) {
  if (!/^(?:[0-9a-f]{2})*$/.test(hex)) throw new Error("invalid synthetic hex");
  return Uint8Array.from(hex.match(/../g) ?? [], (pair) => parseInt(pair, 16));
}
function hex(bytes: Uint8Array) {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

describe("shared frozen Rust/TypeScript goldens (not browser admission)", () => {
  const { clients, servers } = messages();
  it("contains exactly the independently constructed cases", () => {
    expect(corpus.version).toBe(1);
    expect(new Set(corpus.cases.map((value) => value.id))).toEqual(
      new Set([...clients.keys(), ...servers.keys(), "truncated"]),
    );
    expect(new Set(corpus.cases.map((value) => value.id)).size).toBe(corpus.cases.length);
  });
  for (const value of corpus.cases) {
    it(value.id, () => {
      const raw = bytes(value.wire_hex);
      if (value.known_encoding_hex === null) {
        expect(() => fromBinary(ClientEnvelopeSchema, raw)).toThrow();
      } else if (value.direction === "client") {
        const expected = create(ClientEnvelopeSchema, clients.get(value.id));
        const decoded = fromBinary(ClientEnvelopeSchema, raw, { readUnknownFields: false });
        expect(decoded).toEqual(expected);
        expect(hex(toBinary(ClientEnvelopeSchema, expected))).toBe(value.known_encoding_hex);
        expect(hex(toBinary(ClientEnvelopeSchema, decoded))).toBe(value.known_encoding_hex);
        if (value.id.startsWith("unknown-") || value.id.startsWith("message-")) {
          const retained = fromBinary(ClientEnvelopeSchema, raw);
          expect(hex(toBinary(ClientEnvelopeSchema, retained))).toBe(value.wire_hex);
        }
      } else {
        const expected = create(ServerEnvelopeSchema, servers.get(value.id));
        const decoded = fromBinary(ServerEnvelopeSchema, raw, { readUnknownFields: false });
        expect(decoded).toEqual(expected);
        expect(hex(toBinary(ServerEnvelopeSchema, expected))).toBe(value.known_encoding_hex);
      }
      if (value.id.startsWith("message-")) expect(raw.length).toBe(Number(value.id.slice(8)));
    });
  }
});
