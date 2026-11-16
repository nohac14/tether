import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const repositoryRoot = resolve(import.meta.dirname, "..");
export const baselinePath = join(repositoryRoot, "tests/fixtures/protocol/v1-baseline.json");
export const toolPin = JSON.parse(
  readFileSync(join(repositoryRoot, "tooling/protocol-tools.json"), "utf8"),
).buf;

// No download, shell, PATH fallback, registry access, or baseline rewrite occurs.
export function verifiedBuf(candidate = process.env.TETHER_BUF) {
  if (!candidate || !isAbsolute(candidate)) {
    throw new Error("Set TETHER_BUF to the absolute, checksum-verified pinned Buf executable.");
  }
  const expected = toolPin.sha256[`${process.platform}-${process.arch}`];
  if (!expected || !/^[a-f0-9]{64}$/.test(expected)) {
    throw new Error("No reviewed Buf checksum for this host.");
  }
  const info = statSync(candidate);
  if (!info.isFile() || info.size < 1 || info.size > 128 * 1024 * 1024) {
    throw new Error("Invalid Buf executable size/type.");
  }
  const actual = createHash("sha256").update(readFileSync(candidate)).digest("hex");
  if (actual !== expected) throw new Error("Buf executable checksum mismatch.");
  return candidate;
}

export function invokeBuf(executable, arguments_, directory, cache, environment = {}) {
  return spawnSync(executable, arguments_, {
    cwd: directory,
    env: { ...process.env, ...environment, BUF_CACHE_DIR: cache },
    encoding: "utf8",
    timeout: 15_000,
    maxBuffer: 1024 * 1024,
    windowsHide: true,
  });
}

export function checkProtocol(executable = verifiedBuf()) {
  const directory = mkdtempSync(join(tmpdir(), "tether-protocol-check-"));
  try {
    const version = invokeBuf(executable, ["--version"], repositoryRoot, directory);
    if (version.error || version.status !== 0 || version.stdout.trim() !== toolPin.version) {
      throw new Error("Incorrect Buf version or failed invocation.");
    }
    for (const args of [
      ["format", "--diff", "--exit-code", "proto"],
      ["lint", "proto", "--disable-symlinks"],
      ["build", "proto", "--disable-symlinks"],
      ["breaking", "proto", "--against", baselinePath, "--disable-symlinks"],
    ]) {
      const result = invokeBuf(executable, args, repositoryRoot, directory);
      if (result.error || result.status !== 0) {
        // Schema diagnostics only, never runtime input or sensitive values.
        process.stderr.write(result.stdout ?? "");
        process.stderr.write(result.stderr ?? "");
        throw new Error(`Protocol gate failed: ${args[0]}.`);
      }
    }
  } finally {
    // Exact newly created temporary directory, never a caller-supplied path.
    rmSync(directory, { recursive: true, force: true });
  }
}

if (resolve(process.argv[1] ?? "") === fileURLToPath(import.meta.url)) {
  try {
    checkProtocol();
    process.stdout.write("Pinned Buf format/lint/build/FILE compatibility checks passed.\n");
  } catch (error) {
    process.stderr.write(`protocol: ${error.message}\n`);
    process.exitCode = 1;
  }
}
