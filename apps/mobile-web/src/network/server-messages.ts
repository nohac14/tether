import { fromBinary } from "@bufbuild/protobuf";
import {
  type CapabilitySnapshot,
  type ControlState,
  ControlStateKind,
  PermissionState,
  type ProtocolLimits,
  type ServerEnvelope,
  ServerEnvelopeSchema,
  type ServerHello,
} from "../protocol/generated/tether/control/v1/control_pb";

export const MAX_MESSAGE_BYTES = 65_536;
export const MAX_COUNTER = (1n << 64n) - 1n;

export function decodeServerMessage(
  data: unknown,
  limit: number,
  sequence: bigint,
): ServerEnvelope | undefined {
  if (!(data instanceof ArrayBuffer) || data.byteLength === 0 || data.byteLength > limit) return;
  try {
    const wire = fromBinary(ServerEnvelopeSchema, new Uint8Array(data), {
      readUnknownFields: false,
    });
    if (wire.protocolMajor !== 1 || wire.protocolMinor !== 0 || wire.sequence !== sequence) return;
    return wire;
  } catch {
    return undefined;
  }
}

function bounded(value: number, limit: number): boolean {
  return Number.isSafeInteger(value) && value > 0 && value <= limit;
}

function validLimits(value: ProtocolLimits): boolean {
  return (
    bounded(value.maximumEncodedMessageBytes, MAX_MESSAGE_BYTES) &&
    bounded(value.maximumOperationsPerBatch, 256) &&
    bounded(value.maximumTextOperationBytes, 4_096) &&
    bounded(value.maximumTextBytesPerBatch, 16_384) &&
    value.maximumTextBytesPerBatch >= value.maximumTextOperationBytes &&
    bounded(value.maximumAbsoluteFixedDelta, 8_388_608) &&
    bounded(value.maximumAtomicClickCount, 2)
  );
}

function enumeration(values: number[], maximum: number): boolean {
  return (
    values.length <= maximum &&
    new Set(values).size === values.length &&
    values.every((value) => bounded(value, maximum))
  );
}

export function validCapabilities(value: CapabilitySnapshot, limits: ProtocolLimits): boolean {
  const advertised = value.limits;
  if (!advertised || !validLimits(advertised)) return false;
  const permission = value.permissionState;
  const needsAuthorization =
    permission === PermissionState.DENIED || permission === PermissionState.PROMPT_REQUIRED;
  const canInject =
    value.backendInitialized &&
    (permission === PermissionState.GRANTED || permission === PermissionState.NOT_REQUIRED);
  const offersInput =
    value.pointerAvailable ||
    value.atomicClickAvailable ||
    value.verticalScrollAvailable ||
    value.horizontalScrollAvailable ||
    value.physicalKeysAvailable ||
    [value.mouseButtons, value.textModes, value.logicalCommands, value.mediaCommands].some(
      (values) => values.length > 0,
    );
  return (
    value.revision > 0n &&
    bounded(permission, PermissionState.UNAVAILABLE) &&
    needsAuthorization === value.userAuthorizationRequired &&
    (canInject || !offersInput) &&
    (!value.atomicClickAvailable || value.mouseButtons.length > 0) &&
    enumeration(value.mouseButtons, 5) &&
    enumeration(value.textModes, 3) &&
    enumeration(value.logicalCommands, 11) &&
    enumeration(value.mediaCommands, 6) &&
    advertised.maximumEncodedMessageBytes === limits.maximumEncodedMessageBytes &&
    advertised.maximumOperationsPerBatch === limits.maximumOperationsPerBatch &&
    advertised.maximumTextOperationBytes === limits.maximumTextOperationBytes &&
    advertised.maximumTextBytesPerBatch === limits.maximumTextBytesPerBatch &&
    advertised.maximumAbsoluteFixedDelta === limits.maximumAbsoluteFixedDelta &&
    advertised.maximumAtomicClickCount === limits.maximumAtomicClickCount
  );
}

export function validControl(value: ControlState): boolean {
  return (
    value.revision > 0n &&
    (value.sessionEpoch > 0n ||
      value.state === ControlStateKind.NO_CONTROLLER ||
      value.state === ControlStateKind.SUSPENDED) &&
    bounded(value.state, ControlStateKind.SUSPENDED) &&
    value.thisClientControls === (value.state === ControlStateKind.CONTROLLING)
  );
}

export type Welcome = ServerHello & {
  limits: ProtocolLimits;
  capabilities: CapabilitySnapshot;
  controlState: ControlState;
};

export function acceptWelcome(message: ServerEnvelope): Welcome | undefined {
  if (message.sessionEpoch !== 0n || message.payload.case !== "serverHello") return;
  const hello = message.payload.value;
  const { limits, capabilities, controlState } = hello;
  if (
    hello.selectedProtocolMajor !== 1 ||
    hello.selectedProtocolMinor !== 0 ||
    hello.connectionIdentifier.byteLength !== 16 ||
    !limits ||
    !validLimits(limits) ||
    !capabilities ||
    !validCapabilities(capabilities, limits) ||
    !controlState ||
    !validControl(controlState) ||
    !bounded(hello.heartbeatIntervalMillis, 1000) ||
    hello.heartbeatIntervalMillis < 250 ||
    !bounded(hello.heartbeatTimeoutMillis, 4000) ||
    hello.heartbeatTimeoutMillis < Math.max(1000, 2 * hello.heartbeatIntervalMillis)
  )
    return;
  return { ...hello, limits, capabilities, controlState };
}
