import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SKIPPED_DIRECTORIES = new Set([".git", ".pnpm-store", "dist", "node_modules", "target"]);
const INLINE_LINK = /!?\[[^\]]*\]\(([^)]+)\)/g;
const REFERENCE_LINK = /^\s*\[[^\]]+\]:\s*(\S+)/gm;

function walkMarkdown(directory, files = []) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      if (!SKIPPED_DIRECTORIES.has(entry.name)) {
        walkMarkdown(join(directory, entry.name), files);
      }
    } else if (entry.isFile() && extname(entry.name).toLowerCase() === ".md") {
      files.push(join(directory, entry.name));
    }
  }
  return files;
}

function parseDestination(rawDestination) {
  const trimmed = rawDestination.trim();
  if (trimmed.startsWith("<")) {
    const end = trimmed.indexOf(">");
    return end === -1 ? trimmed : trimmed.slice(1, end);
  }
  return trimmed.split(/\s+/, 1)[0] ?? "";
}

export function extractLocalLinks(markdown) {
  const links = [];
  for (const pattern of [INLINE_LINK, REFERENCE_LINK]) {
    pattern.lastIndex = 0;
    for (const match of markdown.matchAll(pattern)) {
      const destination = parseDestination(match[1] ?? "");
      if (
        destination !== "" &&
        !destination.startsWith("//") &&
        !/^[a-z][a-z0-9+.-]*:/i.test(destination)
      ) {
        links.push(destination);
      }
    }
  }
  return links;
}

function decodePath(encodedPath) {
  try {
    return decodeURIComponent(encodedPath);
  } catch {
    return encodedPath;
  }
}

export function findBrokenLocalLinks(rootDirectory) {
  const failures = [];
  for (const markdownFile of walkMarkdown(rootDirectory)) {
    const markdown = readFileSync(markdownFile, "utf8");
    for (const destination of extractLocalLinks(markdown)) {
      const [encodedPath] = destination.split("#", 1);
      if (encodedPath === undefined || encodedPath === "") {
        continue;
      }

      let target = resolve(dirname(markdownFile), decodePath(encodedPath));
      if (existsSync(target) && statSync(target).isDirectory()) {
        target = join(target, "README.md");
      }
      if (!existsSync(target)) {
        failures.push({ destination, markdownFile });
      }
    }
  }
  return failures;
}

const invokedPath = process.argv[1] === undefined ? "" : resolve(process.argv[1]);
if (invokedPath === fileURLToPath(import.meta.url)) {
  const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const failures = findBrokenLocalLinks(repositoryRoot);
  if (failures.length > 0) {
    for (const failure of failures) {
      process.stderr.write(
        failure.markdownFile + ": broken local link " + JSON.stringify(failure.destination) + "\n",
      );
    }
    process.exitCode = 1;
  } else {
    process.stdout.write(
      "Checked local links in " + walkMarkdown(repositoryRoot).length + " Markdown files.\n",
    );
  }
}
