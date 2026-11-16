import { create, fromBinary, toBinary } from "@bufbuild/protobuf";
import { describe, expect, it } from "vitest";
import {
  CapabilitySnapshotSchema,
  ClientEnvelopeSchema,
  MediaCommandActionSchema,
} from "./generated/tether/control/v1/control_pb.js";

describe("generated wire representation (not domain validation)", () => {
  it("retains UTF-8 text and full-width bigint sequence/epoch", () => {
    const message = create(ClientEnvelopeSchema, {
      protocolMajor: 1,
      sequence: 18_446_744_073_709_551_615n,
      sessionEpoch: 18_446_744_073_709_551_614n,
      payload: {
        case: "operationBatch",
        value: {
          operations: [{ action: { case: "textInsertion", value: { text: "test-only 🙂 日本" } } }],
        },
      },
    });
    expect(fromBinary(ClientEnvelopeSchema, toBinary(ClientEnvelopeSchema, message))).toEqual(
      message,
    );
  });
  it("keeps atomic-click availability separate from its positive count limit", () => {
    for (const atomicClickAvailable of [false, true]) {
      const message = create(CapabilitySnapshotSchema, {
        atomicClickAvailable,
        limits: { maximumAtomicClickCount: 2 },
      });
      const decoded = fromBinary(
        CapabilitySnapshotSchema,
        toBinary(CapabilitySnapshotSchema, message),
      );
      expect(decoded.atomicClickAvailable).toBe(atomicClickAvailable);
      expect(decoded.limits?.maximumAtomicClickCount).toBe(2);
    }
  });
  it("rejects truncated encoding but does not invent semantic validation", () => {
    expect(() => fromBinary(ClientEnvelopeSchema, new Uint8Array([0x80]))).toThrow();
    expect(fromBinary(ClientEnvelopeSchema, new Uint8Array()).payload.case).toBeUndefined();
    // Field 1, varint 999: avoid pretending the closed TS enum can produce it.
    expect(fromBinary(MediaCommandActionSchema, new Uint8Array([0x08, 0xe7, 0x07])).command).toBe(
      999,
    );
  });
});
