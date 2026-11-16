import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { invokeBuf, repositoryRoot, verifiedBuf } from "./check-protocol.mjs";

export const generatedFiles = [
  "crates/protocol/src/generated/tether.control.v1.rs",
  "apps/mobile-web/src/protocol/generated/tether/control/v1/control_pb.ts",
];
const generatedRoots = ["crates/protocol/src/generated", "apps/mobile-web/src/protocol/generated"];

function run(program, args, root) {
  const result = spawnSync(program, args, {
    cwd: root,
    env: { ...process.env, CARGO_NET_OFFLINE: "true" },
    timeout: 60_000,
    maxBuffer: 1024 * 1024,
    encoding: "utf8",
    windowsHide: true,
  });
  if (result.error || result.status !== 0) {
    // Build/schema diagnostics, never runtime payloads.
    process.stderr.write(result.stdout ?? "");
    process.stderr.write(result.stderr ?? "");
    throw new Error(`Protocol generator subprocess failed: ${program}.`);
  }
}

function filesBelow(root, relative, files, depth = 0) {
  if (depth > 8 || files.length > 32) throw new Error("Generated tree exceeds bounds.");
  const path = join(root, relative);
  let stat;
  try {
    stat = lstatSync(path);
  } catch (error) {
    if (error.code === "ENOENT") return;
    throw error;
  }
  if (stat.isSymbolicLink()) throw new Error("Generated tree contains a symlink.");
  if (stat.isDirectory()) {
    const children = readdirSync(path);
    if (children.length > 32) throw new Error("Generated directory exceeds bounds.");
    for (const name of children) filesBelow(root, `${relative}/${name}`, files, depth + 1);
  } else if (stat.isFile() && stat.size <= 1024 * 1024) {
    files.push(relative);
  } else throw new Error("Invalid generated file type/size.");
}

export function assertGeneratedTree(root, requireComplete = true) {
  const files = [];
  for (const directory of generatedRoots) {
    realDirectories(root, directory, false);
    filesBelow(root, directory, files);
  }
  const unexpected = files.filter((file) => !generatedFiles.includes(file));
  if (unexpected.length)
    throw new Error(
      `Unexpected generated files: ${unexpected.join(", ")}; review the explicit output manifest.`,
    );
  if (requireComplete && files.length !== generatedFiles.length)
    throw new Error("Missing generated file.");
}

export function compareGenerated(scratch, destination = repositoryRoot) {
  assertGeneratedTree(scratch);
  assertGeneratedTree(destination);
  for (const file of generatedFiles) {
    if (!readFileSync(join(scratch, file)).equals(readFileSync(join(destination, file))))
      throw new Error(`Generated file differs: ${file}; run cargo xtask codegen.`);
  }
}

function realDirectories(root, relative, create = true) {
  let path = root;
  for (const segment of relative.split("/")) {
    path = join(path, segment);
    let stat;
    try {
      stat = lstatSync(path);
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
      if (!create) return;
      mkdirSync(path);
      stat = lstatSync(path);
    }
    if (!stat.isDirectory() || stat.isSymbolicLink())
      throw new Error("Generated destination must contain only real directories.");
  }
}

export function generateProtocol({ check = true } = {}) {
  if (process.versions.node !== "24.19.0") throw new Error("Pinned Node 24.19.0 required.");
  const plugin = join(repositoryRoot, "node_modules/@bufbuild/protoc-gen-es");
  const metadata = JSON.parse(readFileSync(join(plugin, "package.json"), "utf8"));
  const runtime = JSON.parse(
    readFileSync(
      join(repositoryRoot, "apps/mobile-web/node_modules/@bufbuild/protobuf/package.json"),
      "utf8",
    ),
  );
  if (metadata.version !== "2.17.0" || runtime.version !== "2.17.0")
    throw new Error("Pinned protobuf-es plugin/runtime 2.17.0 required.");
  const buf = verifiedBuf();
  const scratch = mkdtempSync(join(tmpdir(), "tether-protocol-generation-"));
  try {
    const version = invokeBuf(buf, ["--version"], repositoryRoot, scratch);
    if (version.error || version.status !== 0 || version.stdout.trim() !== "1.72.0")
      throw new Error("Pinned Buf 1.72.0 required.");
    const descriptor = join(scratch, "control.binpb");
    const build = invokeBuf(
      buf,
      ["build", "proto", "--as-file-descriptor-set", "--output", descriptor, "--disable-symlinks"],
      repositoryRoot,
      scratch,
    );
    if (build.error || build.status !== 0) throw new Error("Buf descriptor generation failed.");
    const es = invokeBuf(
      buf,
      [
        "generate",
        "proto",
        "--template",
        "proto/buf.gen.yaml",
        "--output",
        scratch,
        "--disable-symlinks",
      ],
      repositoryRoot,
      scratch,
      { PATH: `${dirname(process.execPath)}${delimiter}${process.env.PATH ?? ""}` },
    );
    if (es.error || es.status !== 0) {
      process.stderr.write(es.stderr ?? "");
      throw new Error("TypeScript protocol generation failed.");
    }
    const rustOutput = join(scratch, generatedRoots[0]);
    mkdirSync(rustOutput, { recursive: true });
    run(
      "cargo",
      ["run", "--locked", "--offline", "-p", "xtask", "--", "codegen-rust", descriptor, rustOutput],
      repositoryRoot,
    );
    run("rustfmt", ["--edition", "2024", join(scratch, generatedFiles[0])], repositoryRoot);
    assertGeneratedTree(scratch);
    // Canonical EOF also passes Git's staged whitespace check. Do not edit a
    // generated file by hand or suppress whitespace checks for generated code.
    const typescript = join(scratch, generatedFiles[1]);
    writeFileSync(typescript, readFileSync(typescript, "utf8").replace(/\n+$/u, "\n"));
    if (!check) {
      assertGeneratedTree(repositoryRoot, false);
      for (const file of generatedFiles) {
        realDirectories(repositoryRoot, dirname(file).replaceAll("\\", "/"));
        copyFileSync(join(scratch, file), join(repositoryRoot, file));
      }
    }
    compareGenerated(scratch);
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
}

if (resolve(process.argv[1] ?? "") === fileURLToPath(import.meta.url)) {
  try {
    if (process.argv.length !== 3 || !["--check", "--write"].includes(process.argv[2]))
      throw new Error("Usage: generate-protocol.mjs --check|--write");
    generateProtocol({ check: process.argv[2] === "--check" });
    process.stdout.write("Pinned offline Rust/TypeScript generation matches.\n");
  } catch (error) {
    process.stderr.write(`codegen: ${error.message}\n`);
    process.exitCode = 1;
  }
}
