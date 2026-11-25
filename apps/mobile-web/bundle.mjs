import { Buffer } from "node:buffer";
import { lstatSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const MAX_ASSETS = 256;
const MAX_FILE_BYTES = 4 * 1024 * 1024;
const MAX_BUNDLE_BYTES = 16 * 1024 * 1024;
const MAX_MANIFEST_BYTES = 256 * 1024;
const MAX_KEY_BYTES = 128;
const MANIFEST = ".vite/manifest.json";
const ASSET_KEY =
  /^[a-zA-Z0-9_-][a-zA-Z0-9_.-]*-[a-zA-Z0-9_-]{8}\.(?:m?js|css|png|webp|jpe?g|avif|gif|svg|ico|woff2?)$/;

function assetKey(path) {
  if (!path.startsWith("assets/")) throw new Error("Expected a Vite asset path.");
  const key = path.slice("assets/".length);
  if (key.length > MAX_KEY_BYTES || key.includes("..") || ASSET_KEY.exec(key)?.[0] !== key) {
    throw new Error("Expected a flat, hashed, supported Vite asset filename.");
  }
  return key;
}

function bytesOf(output) {
  if (output.type !== "chunk" && output.type !== "asset") {
    throw new Error("Unexpected Vite output type.");
  }
  const value = output.type === "chunk" ? output.code : output.source;
  const length =
    typeof value === "string"
      ? Buffer.byteLength(value)
      : value instanceof Uint8Array
        ? value.byteLength
        : undefined;
  if (length === undefined) throw new Error("Unexpected Vite asset contents.");
  if (length === 0 || length > MAX_FILE_BYTES) {
    throw new Error("Mobile build output is empty or too large.");
  }
  return Buffer.from(value);
}

function manifestFiles(manifestBytes) {
  if (manifestBytes.length > MAX_MANIFEST_BYTES) throw new Error("Vite manifest is too large.");
  const manifest = JSON.parse(manifestBytes.toString("utf8"));
  if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)) {
    throw new Error("Expected a Vite manifest object.");
  }
  const records = Object.entries(manifest);
  if (records.length > MAX_ASSETS) throw new Error("Too many Vite manifest entries.");
  const entry = Object.hasOwn(manifest, "index.html") ? manifest["index.html"] : undefined;
  if (entry?.isEntry !== true || typeof entry.file !== "string") {
    throw new Error("Vite manifest must identify the mobile entry chunk.");
  }

  const paths = new Set();
  for (const [name, record] of records) {
    if (!record || typeof record.file !== "string" || (record.isEntry && name !== "index.html")) {
      throw new Error("Expected one mobile entry in the Vite manifest.");
    }
    paths.add(assetKey(record.file));
    for (const field of ["css", "assets", "imports", "dynamicImports"]) {
      const references = record[field];
      if (references === undefined) continue;
      if (!Array.isArray(references) || references.length > MAX_ASSETS) {
        throw new Error("Invalid Vite manifest references.");
      }
      for (const reference of references) {
        if (typeof reference !== "string") throw new Error("Invalid Vite manifest reference.");
        if (field === "css" || field === "assets") {
          paths.add(assetKey(reference));
        } else if (!Object.hasOwn(manifest, reference)) {
          throw new Error("Vite manifest references an unknown chunk.");
        }
      }
    }
  }
  return { entryFile: entry.file, paths };
}

export function prepareBundle(outputs) {
  if (outputs.length > MAX_ASSETS + 2) throw new Error("Too many mobile build outputs.");
  const files = new Map();
  let entry;
  let manifest;
  let total = 0;
  for (const output of outputs) {
    const bytes = bytesOf(output);
    const { fileName } = output;
    if (fileName === MANIFEST && !manifest) {
      manifest = bytes;
      continue;
    }
    total += bytes.length;
    if (total > MAX_BUNDLE_BYTES) throw new Error("Mobile bundle exceeds its size limit.");
    if (fileName === "index.html" && !entry) {
      entry = bytes;
      continue;
    }
    const key = assetKey(fileName);
    if (files.has(key)) throw new Error("Duplicate mobile build output.");
    files.set(key, bytes);
  }
  if (!entry || !manifest) throw new Error("Expected a mobile entry and Vite manifest.");
  const { entryFile, paths } = manifestFiles(manifest);
  if (
    files.size > MAX_ASSETS ||
    paths.size !== files.size ||
    [...paths].some((key) => !files.has(key))
  ) {
    throw new Error("Vite manifest does not match the emitted mobile assets.");
  }
  if (!entry.includes(Buffer.from(`src="/${entryFile}"`))) {
    throw new Error("Mobile HTML does not reference its emitted script.");
  }
  return { entry, manifest, files: [...files].sort(([a], [b]) => (a < b ? -1 : 1)) };
}

function existing(path) {
  return lstatSync(path, { throwIfNoEntry: false });
}

function outputDirectory(path) {
  const stat = existing(path);
  if (!stat) {
    mkdirSync(path);
  } else if (!stat.isDirectory() || stat.isSymbolicLink()) {
    throw new Error("Mobile build output must be a real directory.");
  }
  return path;
}

function writeFile(path, bytes) {
  const stat = existing(path);
  if (stat && (!stat.isFile() || stat.isSymbolicLink() || stat.nlink !== 1)) {
    throw new Error("Mobile build output must be a regular file with a single link.");
  }
  writeFileSync(path, bytes, { flag: stat ? "w" : "wx" });
}

export function writeBundle(output, bundle, embed) {
  if (embed) {
    const stat = existing(output);
    if (!stat?.isDirectory() || stat.isSymbolicLink()) {
      throw new Error("Cargo output must be an existing real directory.");
    }
  }
  const root = outputDirectory(embed ? join(output, "tether-mobile") : output);
  const assets = outputDirectory(join(root, "assets"));
  for (const [key, bytes] of bundle.files) {
    assetKey(`assets/${key}`);
    writeFile(join(assets, key), bytes);
  }
  writeFile(join(root, "index.html"), bundle.entry);
  if (embed) {
    const includes = bundle.files.map(
      ([key]) =>
        `    EmbeddedFile { key: "${key}", bytes: include_bytes!(concat!(env!("OUT_DIR"), "/tether-mobile/assets/${key}")) },`,
    );
    writeFile(
      join(root, "bundle.rs"),
      [
        "// Generated by the checked Vite build.",
        'static ENTRY: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tether-mobile/index.html"));',
        "static FILES: &[EmbeddedFile] = &[",
        ...includes,
        "];",
        "",
      ].join("\n"),
    );
  } else {
    const metadata = outputDirectory(join(root, ".vite"));
    writeFile(join(metadata, "manifest.json"), bundle.manifest);
  }
}
