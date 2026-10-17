import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { repositoryFiles, scanRepository, scanText } from "./check-secrets.mjs";

test("ordinary source and documentation terms are accepted", () => {
  assert.deepEqual(scanText("pairing_token = request.token; never log passwords or API keys"), []);
});

test("known credential shapes are detected without returning their values", () => {
  const github = ["gh", "p_", "A".repeat(36)].join("");
  const aws = ["AKIA", "B".repeat(16)].join("");
  const findings = scanText(`first=${github}\nsecond=${aws}`);
  assert.deepEqual(
    findings.map(({ rule, line }) => ({ rule, line })),
    [
      { rule: "aws-access-key", line: 2 },
      { rule: "github-token", line: 1 },
    ],
  );
  assert.equal(JSON.stringify(findings).includes(github), false);
  assert.equal(JSON.stringify(findings).includes(aws), false);
});

test("literal secret assignments and credential URLs are detected", () => {
  const password = ["password", "=", '"', "correcthorsevalue", '"'].join("");
  const url = ["https://user:", "longpassword", "@example.invalid/path"].join("");
  assert.deepEqual(
    scanText(`${password}\n${url}`).map(({ rule, line }) => ({ rule, line })),
    [
      { rule: "credential-url", line: 2 },
      { rule: "literal-secret-assignment", line: 1 },
    ],
  );
});

test("tracked deletions do not hide existing tracked or untracked files", () => {
  const root = mkdtempSync(join(tmpdir(), "tether-secret-scan-test-"));
  try {
    const options = { cwd: root, timeout: 10_000, maxBuffer: 64 * 1024 };
    // Only this disposable fixture repository is writable; the real .git is
    // never modified by the scanner or its tests. No commit or hook is run.
    execFileSync("git", ["init", "--quiet"], options);
    writeFileSync(join(root, "deleted.txt"), "removed worktree file\n");
    writeFileSync(join(root, "tracked.txt"), "ordinary tracked text\n");
    execFileSync("git", ["add", "deleted.txt", "tracked.txt"], options);
    unlinkSync(join(root, "deleted.txt"));
    const fake = ["gh", "p_", "A".repeat(36)].join("");
    writeFileSync(join(root, "untracked.txt"), fake);

    const paths = repositoryFiles(root);
    assert.deepEqual(paths, ["tracked.txt", "untracked.txt"]);
    const result = scanRepository(paths.map((path) => join(root, path)));
    assert.equal(result.scanned, 2);
    assert.deepEqual(result.findings, [
      { path: join(root, "untracked.txt"), rule: "github-token", line: 1 },
    ]);
    assert.equal(JSON.stringify(result).includes(fake), false);
    assert.throws(() => scanRepository([join(root, "unexpected-missing.txt")]), { code: "ENOENT" });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
