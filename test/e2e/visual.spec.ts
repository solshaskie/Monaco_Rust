/**
 * Visual Regression Tests (W3.2)
 *
 * Ensures frontend stability across Monaco upgrades and custom renderer changes.
 */

import { test, expect } from '@playwright/test';

const VIEWPORT = { width: 1280, height: 720 };

test.describe('Visual Regression', () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize(VIEWPORT);
    await page.goto('tauri://localhost');
    await page.waitForFunction(() => (window as any).monaco !== undefined, { timeout: 10000 });
  });

  test('editor renders without visual regressions', async ({ page }) => {
    // Wait for editor to be ready
    await page.waitForSelector('.monaco-editor', { timeout: 10000 });

    // Take screenshot of the editor area
    const editor = await page.locator('#editor');
    await expect(editor).toHaveScreenshot('editor-initial.png', {
      threshold: 0.2,
      maxDiffPixels: 100,
    });
  });

  test('sidebar renders without visual regressions', async ({ page }) => {
    await page.waitForSelector('#sidebar', { timeout: 5000 });
    const sidebar = await page.locator('#sidebar');
    await expect(sidebar).toHaveScreenshot('sidebar-initial.png', {
      threshold: 0.2,
      maxDiffPixels: 50,
    });
  });

  test('WASM compute module badge appears', async ({ page }) => {
    // Check that WASM module loaded successfully
    const status = await page.evaluate(() => {
      return (window as any).wasmCompute !== undefined;
    });
    expect(status).toBe(true);
  });
});

test.describe('Performance Regression', () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize(VIEWPORT);
    await page.goto('tauri://localhost');
    await page.waitForFunction(() => (window as any).monaco !== undefined, { timeout: 10000 });
  });

  test('tokenization latency under threshold', async ({ page }) => {
    const latency = await page.evaluate(async () => {
      const compute = (window as any).wasmCompute;
      const start = performance.now();
      await compute.tokenize('fn main() {\n  println!("hello");\n}', 'rust');
      return performance.now() - start;
    });
    expect(latency).toBeLessThan(100); // ms
  });

  test('diff latency under threshold', async ({ page }) => {
    const latency = await page.evaluate(async () => {
      const compute = (window as any).wasmCompute;
      const start = performance.now();
      await compute.diff('hello\nworld\nfoo', 'hello\nworld\nbar');
      return performance.now() - start;
    });
    expect(latency).toBeLessThan(50); // ms
  });
});
