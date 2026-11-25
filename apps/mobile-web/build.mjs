import { isAbsolute, join } from "node:path";
import { URL, fileURLToPath, pathToFileURL } from "node:url";
import { prepareBundle, writeBundle } from "./bundle.mjs";

const project = fileURLToPath(new URL(".", import.meta.url));

export async function buildMobile() {
  if (process.versions.node !== "24.19.0") {
    throw new Error("Tether mobile build requires pinned Node 24.19.0.");
  }
  const { build, version } = await import("vite");
  if (version !== "8.1.6") {
    throw new Error("Tether mobile build requires locked Vite 8.1.6.");
  }
  const output = await build({
    root: project,
    configFile: false,
    envDir: false,
    publicDir: false,
    base: "/",
    mode: "production",
    logLevel: "warn",
    build: {
      write: false,
      manifest: true,
      assetsInlineLimit: 0,
      sourcemap: false,
      reportCompressedSize: false,
    },
  });
  if (!("output" in output)) throw new Error("Expected one production Vite output.");
  return prepareBundle(output.output);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const args = process.argv.slice(2);
    let output = join(project, "dist");
    const embed = args.length === 2 && args[0] === "--cargo-output" && isAbsolute(args[1]);
    if (embed) {
      output = args[1];
    } else if (args.length !== 0) {
      throw new Error("Expected no arguments or an absolute Cargo output directory.");
    }
    process.env.NODE_ENV = "production";
    writeBundle(output, await buildMobile(), embed);
  } catch (error) {
    const message = error instanceof Error ? error.message : "unexpected error";
    process.stderr.write(`mobile build: ${message}\n`);
    process.exitCode = 1;
  }
}
