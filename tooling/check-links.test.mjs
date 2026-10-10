import { randomUUID } from "node:crypto";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";

import { extractLocalLinks, findBrokenLocalLinks } from "./check-links.mjs";

const temporaryDirectories = [];

afterEach(() => {
  for (const directory of temporaryDirectories.splice(0)) {
    rmSync(directory, { force: true, recursive: true });
  }
});

function temporaryDirectory() {
  const directory = join(tmpdir(), "tether-link-test-" + randomUUID());
  mkdirSync(directory, { mode: 0o700 });
  temporaryDirectories.push(directory);
  return directory;
}

describe("extractLocalLinks", () => {
  it("keeps relative links and ignores external schemes", () => {
    expect(
      extractLocalLinks(
        "[local](docs/file.md) [web](https://example.test) [mail](mailto:test@example.test)",
      ),
    ).toEqual(["docs/file.md"]);
  });

  it("handles angle-bracket destinations containing spaces", () => {
    expect(extractLocalLinks("[local](<docs/file name.md>)")).toEqual(["docs/file name.md"]);
  });
});

describe("findBrokenLocalLinks", () => {
  it("accepts files, directories with README files, and fragment-only links", () => {
    const root = temporaryDirectory();
    mkdirSync(join(root, "docs"));
    writeFileSync(join(root, "docs", "README.md"), "# Docs\n");
    writeFileSync(join(root, "target.md"), "# Target\n");
    writeFileSync(join(root, "README.md"), "[file](target.md) [directory](docs/) [self](#section)\n");

    expect(findBrokenLocalLinks(root)).toEqual([]);
  });

  it("reports missing local targets", () => {
    const root = temporaryDirectory();
    writeFileSync(join(root, "README.md"), "[missing](not-there.md)\n");

    expect(findBrokenLocalLinks(root)).toEqual([
      {
        destination: "not-there.md",
        markdownFile: join(root, "README.md"),
      },
    ]);
  });
});
