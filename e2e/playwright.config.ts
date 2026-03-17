import { defineConfig, devices } from '@playwright/test';
import { STORAGE_STATE } from './fixtures/helpers';

// Chrome flags needed for WebRTC loopback tests.
const chromiumArgs = [
  '--use-fake-ui-for-media-stream',
  '--allow-loopback-in-peer-connection',
  '--disable-web-security',
];

export default defineConfig({
  testDir: './tests',
  timeout: 60_000,
  expect: { timeout: 15_000 },
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: 1,
  reporter: [['html'], ['list']],
  globalSetup: './fixtures/server.ts',
  globalTeardown: './fixtures/teardown.ts',

  use: {
    baseURL: process.env.E2E_BASE_URL ?? 'http://localhost:8080',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },

  projects: [
    // ── 1. Auth setup ────────────────────────────────────────────────────────
    // Logs in once and saves localStorage state so other tests can skip login.
    {
      name: 'setup',
      testMatch: /auth\.setup\.ts/,
      use: {
        ...devices['Desktop Chrome'],
        launchOptions: { args: chromiumArgs },
      },
    },

    // ── 2. Authenticated tests ───────────────────────────────────────────────
    // Pre-loads the saved auth state — no login per test.
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        storageState: STORAGE_STATE,
        launchOptions: { args: chromiumArgs },
      },
      dependencies: ['setup'],
      // Auth-specific spec files and setup run elsewhere.
      testIgnore: /(?:0[25]-auth\.spec|auth\.setup)\.ts/,
    },

    // ── 3. Auth-flow tests ───────────────────────────────────────────────────
    // Must start unauthenticated — do NOT pre-load storage state.
    {
      name: 'chromium-no-auth',
      use: {
        ...devices['Desktop Chrome'],
        launchOptions: { args: chromiumArgs },
      },
      testMatch: /0[25]-auth\.spec\.ts/,
    },
  ],
});
