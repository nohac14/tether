import { configDefaults, defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    exclude: [
      ...configDefaults.exclude,
      ".pnpm-store/**",
      // This dependency-free suite uses node:test; test:unit runs it afterward
      // through test:secrets so both runners are mandatory for the unit gate.
      "tooling/check-secrets.test.mjs",
    ],
  },
});
