import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    // Long App flows take ~2 s alone but passed 5 s in the pre-push hook, which
    // runs them while Rust compiles; one global cap replaces per-test ones.
    testTimeout: 15_000,
    setupFiles: ["./src/test/setup.ts"],
    exclude: [
      "**/node_modules/**",
      "**/dist/**",
      "**/.git/**",
      "**/coverage/**",
      "e2e/**",
      "playwright-report/**",
      "test-results/**",
      // Node's built-in test runner owns standalone policy fixtures in tools/.
      "tools/**/*.test.mjs",
    ],
    coverage: {
      provider: "v8",
      reporter: ["text", "json-summary"],
      include: ["src/**/*.{ts,tsx}"],
      exclude: ["src/**/*.test.{ts,tsx}", "src/test/**", "src/main.tsx"],
      thresholds: {
        statements: 80,
        branches: 75,
        functions: 75,
        lines: 80,
      },
    },
  },
});
