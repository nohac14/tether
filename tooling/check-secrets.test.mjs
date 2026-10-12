import assert from "node:assert/strict";
import test from "node:test";

import { scanText } from "./check-secrets.mjs";

test("ordinary source and documentation terms are accepted", () => {
  assert.deepEqual(
    scanText("pairing_token = request.token; never log passwords or API keys"),
    [],
  );
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
  const password = ["password", "=", "\"", "correcthorsevalue", "\""].join("");
  const url = ["https://user:", "longpassword", "@example.invalid/path"].join("");
  assert.deepEqual(
    scanText(`${password}\n${url}`).map(({ rule, line }) => ({ rule, line })),
    [
      { rule: "credential-url", line: 2 },
      { rule: "literal-secret-assignment", line: 1 },
    ],
  );
});
