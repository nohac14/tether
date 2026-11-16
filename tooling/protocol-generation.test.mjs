import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { repositoryRoot } from "./check-protocol.mjs";
import {
  assertGeneratedTree,
  compareGenerated,
  generatedFiles,
  generateProtocol,
} from "./generate-protocol.mjs";

const directories = [];
afterEach(() => {
  for (const directory of directories.splice(0))
    rmSync(directory, { recursive: true, force: true });
});
function copyGenerated() {
  const root = mkdtempSync(join(tmpdir(), "tether-generation-regression-"));
  directories.push(root);
  for (const file of generatedFiles) {
    mkdirSync(dirname(join(root, file)), { recursive: true });
    copyFileSync(join(repositoryRoot, file), join(root, file));
  }
  return root;
}

describe("reproducible offline protocol generation", () => {
  it("pins generated and compatibility fixture checkout line endings to LF", () => {
    const files = [
      ...generatedFiles,
      "tests/fixtures/protocol/v1-baseline.json",
      "proto/tether/control/v1/control.proto",
    ];
    const result = spawnSync("git", ["check-attr", "text", "eol", "--", ...files], {
      cwd: repositoryRoot,
      encoding: "utf8",
      timeout: 5000,
      maxBuffer: 4096,
    });
    expect(result.error).toBeUndefined();
    expect(result.status).toBe(0);
    for (const file of files) {
      expect(result.stdout).toContain(`${file}: eol: lf`);
      expect(result.stdout).toContain(`${file}: text: set`);
    }
  });
  it("regenerates both outputs without changing checked-in sources", () => {
    const before = generatedFiles.map((file) => readFileSync(join(repositoryRoot, file)));
    generateProtocol();
    expect(generatedFiles.map((file) => readFileSync(join(repositoryRoot, file)))).toEqual(before);
    for (const file of generatedFiles) {
      const source = readFileSync(join(repositoryRoot, file), "utf8");
      expect(source.endsWith("\n")).toBe(true);
      expect(source.endsWith("\n\n")).toBe(false);
    }
  }, 30_000);
  it("rejects stale, missing and unexpected outputs without rewriting them", () => {
    const expected = copyGenerated();
    const destination = copyGenerated();
    compareGenerated(expected, destination);
    writeFileSync(join(destination, generatedFiles[0]), "// intentionally stale\n");
    expect(() => compareGenerated(expected, destination)).toThrow("differs");
    expect(readFileSync(join(destination, generatedFiles[0]), "utf8")).toBe(
      "// intentionally stale\n",
    );
    rmSync(join(destination, generatedFiles[0]));
    expect(() => compareGenerated(expected, destination)).toThrow("Missing");
    const extra = join(expected, dirname(generatedFiles[0]), "unexpected.rs");
    writeFileSync(extra, "// extra\n");
    expect(() => assertGeneratedTree(expected)).toThrow("Unexpected");
  });
  it("rejects symlinks, including dangling links, instead of treating them as absent", () => {
    const root = copyGenerated();
    const file = join(root, generatedFiles[0]);
    rmSync(file);
    symlinkSync(join(root, "does-not-exist"), file);
    expect(() => assertGeneratedTree(root)).toThrow("symlink");
  });
  it("rejects a symlink in an output ancestor and oversized files", () => {
    const root = copyGenerated();
    renameSync(join(root, "crates"), join(root, "actual-crates"));
    symlinkSync(join(root, "actual-crates"), join(root, "crates"), "junction");
    expect(() => assertGeneratedTree(root)).toThrow("real directories");
    const oversized = copyGenerated();
    writeFileSync(join(oversized, generatedFiles[0]), new Uint8Array(1024 * 1024 + 1));
    expect(() => assertGeneratedTree(oversized)).toThrow("size");
  });
  it("all generated Rust types have redacted diagnostics and no payload Debug derives", () => {
    const rust = readFileSync(join(repositoryRoot, generatedFiles[0]), "utf8");
    const typeCount = [...rust.matchAll(/pub (?:struct|enum) /g)].length;
    expect(typeCount).toBeGreaterThan(40);
    expect([...rust.matchAll(/impl ::core::fmt::Debug for /g)].length).toBe(typeCount);
    expect(rust).not.toMatch(/#\[derive\([^\]]*\bDebug\b/);
    expect([...rust.matchAll(/WireMessage\(<redacted>\)/g)].length).toBe(typeCount);
    expect(rust).not.toMatch(/#\[allow\(/);
    expect([...rust.matchAll(/#\[expect\(/g)].length).toBe(1);
    expect(rust).toMatch(/expect\(\s*clippy::struct_excessive_bools,/);
    expect(rust).toMatch(/\)\]\s*pub struct CapabilitySnapshot/);
  });
});
