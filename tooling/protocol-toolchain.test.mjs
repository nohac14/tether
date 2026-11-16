import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  baselinePath,
  checkProtocol,
  invokeBuf,
  repositoryRoot,
  toolPin,
  verifiedBuf,
} from "./check-protocol.mjs";

const directories = [];
afterEach(() => {
  for (const directory of directories.splice(0))
    rmSync(directory, { recursive: true, force: true });
});
function fixture() {
  const root = mkdtempSync(join(tmpdir(), "tether-protocol-fixture-"));
  directories.push(root);
  cpSync(join(repositoryRoot, "proto"), join(root, "proto"), { recursive: true });
  return root;
}

describe("version-one schema compatibility", () => {
  it("preserves reserved numbers and explicit atomic-click availability in the baseline", () => {
    const image = JSON.parse(readFileSync(baselinePath, "utf8"));
    expect(image.file).toHaveLength(1);
    const file = image.file[0];
    expect(file.name).toBe("tether/control/v1/control.proto");
    expect(file.syntax).toBe("proto3");
    const capabilities = file.messageType.find((message) => message.name === "CapabilitySnapshot");
    expect(
      capabilities.field.find((field) => field.name === "atomic_click_available"),
    ).toMatchObject({ number: 21, type: "TYPE_BOOL" });
    expect(capabilities.reservedRange).toContainEqual({ start: 14, end: 21 });
    for (const hash of Object.values(toolPin.sha256)) expect(hash).toMatch(/^[a-f0-9]{64}$/);
  });
  it("rejects missing, relative and checksum-mismatched tools before executing", () => {
    expect(() => verifiedBuf("")).toThrow("Set TETHER_BUF");
    expect(() => verifiedBuf("buf")).toThrow("Set TETHER_BUF");
    expect(() => verifiedBuf(baselinePath)).toThrow("checksum mismatch");
  });
  it("passes the pinned real format/lint/build/FILE gate without rewriting the baseline", () => {
    const before = readFileSync(baselinePath);
    checkProtocol();
    expect(readFileSync(baselinePath)).toEqual(before);
  });
  it("demonstrates that removing the click field fails FILE compatibility", () => {
    const root = fixture();
    const path = join(root, "proto/tether/control/v1/control.proto");
    const source = readFileSync(path, "utf8");
    const changed = source.replace("  bool atomic_click_available = 21;\n", "");
    expect(changed).not.toBe(source);
    writeFileSync(path, changed);
    const result = invokeBuf(
      verifiedBuf(),
      ["breaking", "proto", "--against", baselinePath],
      root,
      join(root, "cache"),
    );
    expect(result.error).toBeUndefined();
    expect(result.status).toBe(100);
    expect(result.stdout + result.stderr).toContain("atomic_click_available");
  });
  it("permits an additive field while preserving the frozen baseline", () => {
    const root = fixture();
    const path = join(root, "proto/tether/control/v1/control.proto");
    const source = readFileSync(path, "utf8");
    writeFileSync(
      path,
      source.replace(
        "  bool atomic_click_available = 21;",
        "  bool atomic_click_available = 21;\n  bool future_optional_marker = 22;",
      ),
    );
    const result = invokeBuf(
      verifiedBuf(),
      ["breaking", "proto", "--against", baselinePath],
      root,
      join(root, "cache"),
    );
    expect(result.error).toBeUndefined();
    expect(result.status).toBe(0);
  });
});
