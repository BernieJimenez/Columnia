import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  // A local gate must never silently run a subset of the suite.
  forbidOnly: true,
  // LIM-04: no CI runs this suite; a failure is never retried away.
  retries: 0,
  reporter: [["list"], ["html", { open: "never" }]],
  use: {
    baseURL: "http://127.0.0.1:4173",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
    launchOptions: {
      args: ["--no-proxy-server", "--disable-gpu", "--disable-dev-shm-usage"],
    },
    // LIM-04: Edge, the engine of WebView2 that runs the real app; it is the
    // only browser `npm run test:e2e:install` sets up.
    channel: "msedge",
    ...devices["Desktop Chrome"],
  },
  webServer: {
    command: "npm run build && npm run preview -- --host 127.0.0.1 --port 4173",
    url: "http://127.0.0.1:4173",
    // Never accept a preview started from another checkout or bundle.
    reuseExistingServer: false,
    timeout: 120_000,
    stdout: "pipe",
    stderr: "pipe",
  },
});
