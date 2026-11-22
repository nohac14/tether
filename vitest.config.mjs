import { configDefaults, defineConfig } from "vitest/config";

export default defineConfig({
  // pnpm can create relative directory symlinks on Windows in Developer Mode.
  // Keep their import paths intact so Vite can resolve the locked dependencies.
  resolve: { preserveSymlinks: process.platform === "win32" },
  test: {
    exclude: [
      ...configDefaults.exclude,
      ".pnpm-store/**",
      // Native builds and isolated verification checkouts live under target.
      "target/**",
      // This dependency-free suite uses node:test; test:unit runs it afterward
      // through test:secrets so both runners are mandatory for the unit gate.
      "tooling/check-secrets.test.mjs",
    ],
  },
});
