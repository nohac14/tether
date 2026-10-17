import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL, URL } from "node:url";
import { afterEach, describe, expect, it } from "vitest";

const rootRequire = createRequire(import.meta.url);
const cliPath = rootRequire.resolve("markdownlint-cli");
const cliRequire = createRequire(cliPath);
const lintRequire = createRequire(cliRequire.resolve("markdownlint"));
const temporaryDirectories = [];

afterEach(() => {
  for (const directory of temporaryDirectories.splice(0)) {
    rmSync(directory, { force: true, recursive: true });
  }
});

function fixture() {
  const directory = mkdtempSync(join(tmpdir(), "tether-markdown-toolchain-"));
  temporaryDirectories.push(directory);
  writeFileSync(join(directory, "README.md"), "# Fixture\n\n" + "x".repeat(100) + "\n");
  return directory;
}

function lint(directory, configName, extraArguments = []) {
  return spawnSync(
    process.execPath,
    [cliPath, "--config", configName, ...extraArguments, "**/*.md"],
    {
      cwd: directory,
      encoding: "utf8",
      timeout: 10_000,
      maxBuffer: 64 * 1024,
    },
  );
}

describe("approved Markdown toolchain", () => {
  it("retains the existing rule engine and exact configuration", async () => {
    const { getVersion } = await import(pathToFileURL(cliRequire.resolve("markdownlint")));
    // CLI2 0.23.3 pins this same engine; switching CLIs must not remove rules.
    expect(getVersion()).toBe("0.41.1");
    const config = JSON.parse(
      readFileSync(new URL("../.markdownlint.json", import.meta.url), "utf8"),
    );
    expect(config).toEqual({
      MD013: false,
      MD024: { siblings_only: true },
      MD033: false,
      MD036: false,
    });
  });

  it("loads TOML configuration through the overridden parser", () => {
    const directory = fixture();
    writeFileSync(join(directory, "lint.toml"), "MD013 = false\n");
    const result = lint(directory, "lint.toml");
    expect(result.error).toBeUndefined();
    expect(result.stderr).toBe("");
    expect(result.status).toBe(0);

    // The CLI probes several formats, so an invalid TOML string may be valid
    // YAML. Check malformed TOML against the actual overridden parser itself.
    const { parse } = cliRequire("smol-toml");
    expect(() => parse("MD013 = [\n")).toThrow();
    expect(parse('name = "Tether — 键盘"\naxes = ["x", "y"]\n')).toEqual({
      name: "Tether — 键盘",
      axes: ["x", "y"],
    });
  });

  it("honors YAML configuration and rejects malformed YAML", () => {
    const directory = fixture();
    writeFileSync(join(directory, "lint.yaml"), "MD013: false\n");
    const result = lint(directory, "lint.yaml");
    expect(result.error).toBeUndefined();
    expect(result.stderr).toBe("");
    expect(result.status).toBe(0);

    writeFileSync(join(directory, "lint.yaml"), "MD013: [\n");
    expect(lint(directory, "lint.yaml").status).not.toBe(0);
  });

  it("excludes ignored artifacts but still rejects invalid source Markdown", () => {
    const directory = fixture();
    writeFileSync(join(directory, "lint.json"), '{"MD013":false}\n');
    writeFileSync(join(directory, ".gitignore"), ".pnpm-store/\nnode_modules/\ntarget/\n");
    for (const ignored of [".pnpm-store", "node_modules", "target"]) {
      mkdirSync(join(directory, ignored));
      writeFileSync(join(directory, ignored, "README.md"), "not a heading\n");
    }
    const extraArguments = ["--ignore-path", ".gitignore"];
    expect(lint(directory, "lint.json", extraArguments).status).toBe(0);

    writeFileSync(join(directory, "README.md"), "not a heading\n");
    const result = lint(directory, "lint.json", extraArguments);
    expect(result.error).toBeUndefined();
    expect(result.status).toBe(1);
    expect(result.stderr).toContain("README.md:1");
    expect(result.stderr).toContain("MD041");
    expect(result.stderr).not.toContain(".pnpm-store/");
  });

  it("renders inline and block math with the scoped KaTeX override", async () => {
    const { micromark } = await import(pathToFileURL(lintRequire.resolve("micromark")));
    const { math, mathHtml } = await import(
      pathToFileURL(lintRequire.resolve("micromark-extension-math"))
    );
    const options = { extensions: [math()], htmlExtensions: [mathHtml({ trust: false })] };
    const html = micromark("Inline $x^2$.\n\n$$\n\\frac{1}{2}\n$$\n", options);
    expect(html).toContain('class="math math-inline"');
    expect(html).toContain('class="math math-display"');
    expect(html).toContain('class="katex"');
    expect(html).not.toContain("katex-error");

    const untrusted = micromark("$\\href{javascript:alert(1)}{x}$", options);
    expect(untrusted).not.toContain("<a ");
    expect(untrusted).not.toContain('href="javascript:');
  });
});
