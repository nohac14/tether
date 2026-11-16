import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";

const rootRequire = createRequire(import.meta.url);
const eslintRequire = createRequire(rootRequire.resolve("eslint"));
const tsRequire = createRequire(rootRequire.resolve("typescript-eslint"));
const parserRequire = createRequire(tsRequire.resolve("@typescript-eslint/parser"));
const consumers = [
  eslintRequire,
  createRequire(eslintRequire.resolve("@eslint/config-array")),
  createRequire(parserRequire.resolve("@typescript-eslint/typescript-estree")),
  createRequire(rootRequire.resolve("markdownlint-cli")),
];
const directories = [];
afterEach(() => {
  for (const directory of directories.splice(0))
    rmSync(directory, { recursive: true, force: true });
});
function fixture() {
  const root = mkdtempSync(join(tmpdir(), "tether-glob-compatibility-"));
  directories.push(root);
  return root;
}
function runCli(binary, args, cwd) {
  return spawnSync(process.execPath, [binary, ...args], {
    cwd,
    encoding: "utf8",
    timeout: 10_000,
    maxBuffer: 64 * 1024,
  });
}

describe("owner-approved scoped glob remediation", () => {
  it("resolves the exact fixed version for all four consumers without changing direct pins", () => {
    expect(rootRequire("eslint/package.json").version).toBe("10.11.0");
    expect(rootRequire("typescript-eslint/package.json").version).toBe("8.71.1");
    expect(rootRequire("markdownlint-cli/package.json").version).toBe("0.49.1");
    for (const require of consumers)
      expect(require("minimatch/package.json").version).toBe("11.0.1");
  });
  it("preserves brace, dotfile, ignore and Windows-style path matching for each consumer", () => {
    for (const require of consumers) {
      const { Minimatch } = require("minimatch");
      const matcher = new Minimatch("**/*.{mjs,ts,tsx}", { dot: true, optimizationLevel: 2 });
      for (const file of [
        "tooling/check-protocol.mjs",
        "apps/mobile-web/src/control.tsx",
        ".config/test.ts",
      ])
        expect(matcher.match(file)).toBe(true);
      expect(matcher.match("docs/SPEC.md")).toBe(false);
      expect(
        new Minimatch("**/node_modules/**", { dot: true }).match(
          "apps/web/node_modules/pkg/index.ts",
        ),
      ).toBe(true);
      expect(
        new Minimatch("apps\\**\\*.ts", { platform: "win32", windowsPathsNoEscape: true }).match(
          "apps\\web\\control.ts",
        ),
      ).toBe(true);
    }
  });
  it("bounds the advisory's preprocessing expansion with the explicit supported budget", () => {
    const entry = eslintRequire.resolve("minimatch");
    const code = `const {Minimatch}=require(${JSON.stringify(entry)}); const matcher=new Minimatch('safe',{optimizationLevel:2,braceExpandMax:16}); const parts=matcher.firstPhasePreProcess([('**/../a/b/'.repeat(8)+'z').split('/')]); if(parts.length>16)process.exit(1); process.stdout.write(String(parts.length));`;
    const result = spawnSync(process.execPath, ["--max-old-space-size=128", "-e", code], {
      encoding: "utf8",
      timeout: 5000,
      maxBuffer: 4096,
    });
    expect(result.error).toBeUndefined();
    expect(result.status).toBe(0);
    expect(Number(result.stdout)).toBeGreaterThan(0);
    expect(Number(result.stdout)).toBeLessThanOrEqual(16);
  });
  it("actual ESLint selects invalid source and excludes ignored generated source", () => {
    const root = fixture();
    mkdirSync(join(root, "src/generated"), { recursive: true });
    writeFileSync(
      join(root, "eslint.config.mjs"),
      "export default [{ignores:['**/generated/**']},{files:['src/**/*.mjs'],rules:{'no-undef':'error'}}];\n",
    );
    writeFileSync(join(root, "src/good.mjs"), "export const answer=42;\n");
    writeFileSync(join(root, "src/bad.mjs"), "unknown_identifier();\n");
    writeFileSync(join(root, "src/generated/ignored.mjs"), "ignored_identifier();\n");
    const binary = join(dirname(rootRequire.resolve("eslint/package.json")), "bin/eslint.js");
    const result = runCli(
      binary,
      ["--config", "eslint.config.mjs", "--format", "json", "src"],
      root,
    );
    expect(result.error).toBeUndefined();
    expect(result.status).toBe(1);
    const rows = JSON.parse(result.stdout);
    expect(rows.reduce((count, row) => count + row.errorCount, 0)).toBe(1);
    expect(rows.some((row) => row.filePath.endsWith("ignored.mjs"))).toBe(false);
  });
  it("actual Markdown CLI preserves glob selection and ignore-file behavior", () => {
    const root = fixture();
    mkdirSync(join(root, "ignored"));
    writeFileSync(join(root, ".markdownlintignore"), "ignored/**\n");
    writeFileSync(join(root, "good.md"), "# Good\n\nText.\n");
    writeFileSync(join(root, "bad.md"), "#Bad\n");
    writeFileSync(join(root, "ignored/ignored.md"), "#Ignored\n");
    const result = runCli(rootRequire.resolve("markdownlint-cli"), ["**/*.md"], root);
    expect(result.error).toBeUndefined();
    expect(result.status).toBe(1);
    expect(result.stdout + result.stderr).toContain("bad.md");
    expect(result.stdout + result.stderr).not.toContain("ignored.md");
  });
});
