import { defineConfig, devices } from '@playwright/test';
import path from 'path';
import os from 'os';

/**
 * Visual Regression Test Configuration (W3.2 / D.2)
 *
 * Tests run against the built Tauri app via WebDriver (tauri-driver).
 * Usage:
 *   1. Build the Tauri app: cd src-tauri && cargo build --release
 *   2. Start tauri-driver: tauri-driver
 *   3. Run tests: npx playwright test
 *
 * Baseline screenshots are stored per-platform:
 *   tests/visual/snapshots/linux/
 *   tests/visual/snapshots/darwin/
 *   tests/visual/snapshots/win32/
 * CI fails on >1% pixel diff outside known change zones.
 */

const platform = os.platform();
const tauriBinary = path.resolve(
  __dirname,
  '../../src-tauri/target/release/monaco-tauri'
);

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
    command: `tauri-driver --native-driver ${tauriBinary}`,
    url: 'http://localhost:4444/status',
    reuseExistingServer: !process.env.CI,
    timeout: 120 * 1000,
  },
});
