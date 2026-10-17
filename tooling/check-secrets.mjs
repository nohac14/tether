#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const MAX_SCANNED_FILE_BYTES = 1024 * 1024;
const BINARY_EXTENSIONS = new Set([
  ".avif",
  ".gif",
  ".ico",
  ".jpeg",
  ".jpg",
  ".pdf",
  ".png",
  ".webp",
  ".woff",
  ".woff2",
  ".zip",
]);

const RULES = [
  {
    id: "private-key",
    expression: /-----BEGIN(?: [A-Z0-9]+)? PRIVATE KEY-----/gu,
  },
  {
    id: "aws-access-key",
    expression: /\b(?:AKIA|ASIA)[A-Z0-9]{16}\b/gu,
  },
  {
    id: "github-token",
    expression: /\b(?:gh[pousr]_[A-Za-z0-9]{36,255}|github_pat_[A-Za-z0-9_]{40,255})\b/gu,
  },
  {
    id: "slack-token",
    expression: /\bxox[baprs]-[A-Za-z0-9-]{16,255}\b/gu,
  },
  {
    id: "google-api-key",
    expression: /\bAIza[A-Za-z0-9_-]{35}\b/gu,
  },
  {
    id: "live-payment-secret",
    expression: /\b(?:sk|rk)_live_[A-Za-z0-9]{16,255}\b/gu,
  },
  {
    id: "credential-url",
    expression: /\b[a-z][a-z0-9+.-]*:\/\/[^\s/:@]{2,}:[^\s/@]{8,}@/giu,
  },
  {
    id: "literal-secret-assignment",
    expression:
      /\b(?:api[_-]?key|client[_-]?secret|access[_-]?token|auth[_-]?token|pairing[_-]?token|password|passwd)\b\s*[:=]\s*["'][A-Za-z0-9_+/=.-]{16,}["']/giu,
  },
];

function lineNumber(text, offset) {
  let line = 1;
  for (let index = 0; index < offset; index += 1) {
    if (text.charCodeAt(index) === 10) line += 1;
  }
  return line;
}

export function scanText(text) {
  const findings = [];
  for (const rule of RULES) {
    rule.expression.lastIndex = 0;
    for (const match of text.matchAll(rule.expression)) {
      findings.push({ rule: rule.id, line: lineNumber(text, match.index) });
    }
  }
  return findings;
}

function extension(path) {
  const base = path.toLowerCase().split("/").at(-1);
  const dot = base.lastIndexOf(".");
  return dot === -1 ? "" : base.slice(dot);
}

export function repositoryFiles(rootDirectory = process.cwd()) {
  const options = {
    cwd: rootDirectory,
    encoding: "utf8",
    maxBuffer: 8 * 1024 * 1024,
    timeout: 10_000,
  };
  const output = execFileSync(
    "git",
    ["ls-files", "-z", "--cached", "--others", "--exclude-standard"],
    options,
  );
  // A tracked deletion has no worktree contents to scan. Exclude only paths
  // explicitly reported deleted by Git; any other read failure remains fatal.
  // History/deleted-blob inspection belongs to the separate CI history scan.
  const deleted = new Set(
    execFileSync("git", ["ls-files", "-z", "--deleted"], options).split("\0"),
  );
  return [...new Set(output.split("\0"))]
    .filter((path) => path !== "" && !deleted.has(path))
    .sort();
}

export function scanRepository(paths = repositoryFiles()) {
  const findings = [];
  let scanned = 0;
  let skippedBinary = 0;
  for (const path of paths) {
    if (BINARY_EXTENSIONS.has(extension(path))) {
      skippedBinary += 1;
      continue;
    }
    const bytes = readFileSync(path);
    if (bytes.length > MAX_SCANNED_FILE_BYTES) {
      findings.push({ path, rule: "oversized-text-file", line: 0 });
      continue;
    }
    if (bytes.includes(0)) {
      skippedBinary += 1;
      continue;
    }
    scanned += 1;
    const text = bytes.toString("utf8");
    for (const finding of scanText(text)) findings.push({ path, ...finding });
  }
  return { findings, scanned, skippedBinary };
}

function main() {
  const result = scanRepository();
  if (result.findings.length > 0) {
    for (const finding of result.findings) {
      const position = finding.line === 0 ? finding.path : `${finding.path}:${finding.line}`;
      process.stderr.write(`${position}: potential secret (${finding.rule})\n`);
    }
    process.stderr.write(
      `Secret scan failed with ${result.findings.length} finding(s); matched values were redacted.\n`,
    );
    process.exitCode = 1;
    return;
  }
  process.stdout.write(
    `Secret scan passed: ${result.scanned} text files scanned, ${result.skippedBinary} binary files skipped.\n`,
  );
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
