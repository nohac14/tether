import { Buffer } from "node:buffer";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { URL, fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { prepareBundle, writeBundle } from "../apps/mobile-web/bundle.mjs";

const SCRIPT = "assets/index-abcd12EF.js";
const CSS = "assets/index-TK1234ab.css";
const BUILD = fileURLToPath(new URL("../apps/mobile-web/build.mjs", import.meta.url));

function asset(fileName, source) {
  return { type: "asset", fileName, source };
}

function fixture({
  script = SCRIPT,
  html = `<script type="module" src="/${script}"></script>`,
  manifest = { "index.html": { file: script, isEntry: true, css: [CSS] } },
} = {}) {
  return [
    asset("index.html", html),
    asset(".vite/manifest.json", JSON.stringify(manifest)),
    asset(CSS, ":root{}"),
    { type: "chunk", fileName: script, code: "export const ready = true;" },
  ];
}

function temporaryDirectory(run) {
  const root = mkdtempSync(join(realpathSync(tmpdir()), "tether-mobile-bundle-"));
  try {
    return run(root);
  } finally {
    // This exact directory was created above and never comes from build data.
    rmSync(root, { recursive: true, force: true });
  }
}

describe("mobile build manifest", () => {
  it("sorts emitted assets and generates include references without absolute paths", () => {
    const bundle = prepareBundle(fixture().reverse());
    expect(bundle.files.map(([key]) => key)).toEqual([CSS.slice(7), SCRIPT.slice(7)]);
    temporaryDirectory((directory) => {
      writeBundle(directory, bundle, true);
      const root = join(directory, "tether-mobile");
      const manifest = readFileSync(join(root, "bundle.rs"), "utf8");
      expect(manifest).not.toContain(directory);
      expect(manifest).toContain(SCRIPT);
      expect(manifest).toContain(CSS);
      expect(readFileSync(join(root, "index.html"))).toEqual(bundle.entry);
      expect(readdirSync(root).sort()).toEqual(["assets", "bundle.rs", "index.html"]);
    });
  });

  it.each([
    "assets/../index-abcd12EF.js",
    "assets/nested/index-abcd12EF.js",
    "assets/index-abcd12EF.js\n",
    "assets/index-abcd12EF.js.map",
    "assets/index.js",
    "https://example.invalid/index-abcd12EF.js",
  ])("rejects an unsafe or unsupported build key %s", (script) => {
    expect(() => prepareBundle(fixture({ script }))).toThrow();
  });

  it("rejects missing, duplicate, unreferenced and oversized outputs", () => {
    const outputs = fixture();
    expect(() => prepareBundle(outputs.filter(({ fileName }) => fileName !== CSS))).toThrow(
      "does not match",
    );
    expect(() => prepareBundle([...outputs, outputs[2]])).toThrow("Duplicate");
    expect(() => prepareBundle([...outputs, asset("assets/extra-abcd12EF.css", "div{}")])).toThrow(
      "does not match",
    );
    expect(() =>
      prepareBundle([asset("index.html", Buffer.alloc(4 * 1024 * 1024 + 1)), ...outputs.slice(1)]),
    ).toThrow("too large");
  });

  it("rejects missing chunk references and an HTML entry for another build", () => {
    const manifest = {
      "index.html": { file: SCRIPT, isEntry: true, css: [CSS], imports: ["absent"] },
    };
    expect(() => prepareBundle(fixture({ manifest }))).toThrow("unknown chunk");
    expect(() => prepareBundle(fixture({ html: "<html>Missing script</html>" }))).toThrow(
      "does not reference",
    );
  });

  it("builds two identical embedded bundles from the pinned frontend sources", () => {
    temporaryDirectory((directory) => {
      const copies = ["one", "two"].map((name) => {
        const output = join(directory, name);
        mkdirSync(output);
        const run = spawnSync(process.execPath, [BUILD, "--cargo-output", output], {
          timeout: 15_000,
          windowsHide: true,
          encoding: "utf8",
        });
        expect(run.error).toBeUndefined();
        expect(run.status, run.stderr).toBe(0);
        const root = join(output, "tether-mobile");
        const files = ["bundle.rs", "index.html"];
        files.push(...readdirSync(join(root, "assets")).map((key) => `assets/${key}`));
        return files.sort().map((file) => [file, readFileSync(join(root, file))]);
      });
      expect(copies[0]).toEqual(copies[1]);
    });
  }, 35_000);
});
