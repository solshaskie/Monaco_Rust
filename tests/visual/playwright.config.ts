import { defineConfig, devices } from '@playwright/test';
import os from 'os';

/**
 * Visual Regression Test Configuration (W3.2 / D.2)
 *
 * This file is still an experimental screenshot surface, not the authoritative
 * live-app Tauri harness. For real live-app proof use the direct WebDriver
 * script in `tests/visual/sparse-large-document.webdriver.mjs`.
 *
 * The old `tauri-driver --native-driver <app-binary>` wiring was incorrect:
 * `--native-driver` expects the platform WebDriver binary, not the Tauri app.
 * Usage:
 *   1. Build the Tauri app: cd src-tauri && cargo build --release
 *   2. Ensure the native WebKit driver exists (Linux: `webkit2gtk-driver`)
 *   3. Start tauri-driver: tauri-driver
 *   4. Run tests: npx playwright test
 *
 * Baseline screenshots are stored per-platform:
 *   tests/visual/snapshots/linux/
 *   tests/visual/snapshots/darwin/
 *   tests/visual/snapshots/win32/
 * CI fails on >1% pixel diff outside known change zones.
 */

const platform = os.platform();

export default defineConfig({
  testDir: '.',
  snapshotPathTemplate: `./snapshots/${platform}/{testFilePath}/{arg}{ext}`,
  fullyParallel: false,
  workers: 1,
  reporter: [['html', { open: 'never' }], ['list']],

  use: {
    trace: 'on-first-retry',
    baseURL: 'http://localhost:4444',
  },

  projects: [
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        viewport: { width: 1280, height: 720 },
      },
    },
  ],

  webServer: {
    command: 'tauri-driver',
    url: 'http://localhost:4444/status',
    reuseExistingServer: !process.env.CI,
    timeout: 120 * 1000,
  },
});
