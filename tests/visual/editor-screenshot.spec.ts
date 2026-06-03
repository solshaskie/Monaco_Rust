import { test, expect } from '@playwright/test';

declare global {
  interface Window {
    monaco?: any;
    loadWorkspace?: (path: string) => void;
  }
}

/**
 * Visual Regression: Editor Screenshots (W3.2)
 *
 * Captures the editor at fixed states and compares against baselines.
 * Baselines are stored per-platform (macOS, Linux, Windows).
 */

test.describe('Editor Visual States', () => {
  test('empty editor state', async ({ page }) => {
    // Navigate to the app (tauri-driver handles the app launch)
    await page.goto('/');

    // Wait for Monaco to initialize
    await page.waitForSelector('.monaco-editor', { timeout: 10000 });

    // Screenshot the editor container
    const editor = page.locator('.monaco-editor');
    await expect(editor).toHaveScreenshot('editor-empty.png', {
      maxDiffPixelRatio: 0.01,
    });
  });

  test('editor with file open', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('.monaco-editor', { timeout: 10000 });

    // Trigger file open via the UI (or IPC if exposed)
    // This depends on the app's frontend API
    await page.evaluate(() => {
      // @ts-ignore
      if (window.loadWorkspace) window.loadWorkspace('/tmp');
    });

    // Allow time for file tree and editor to render
    await page.waitForTimeout(500);

    const editor = page.locator('.monaco-editor');
    await expect(editor).toHaveScreenshot('editor-with-file.png', {
      maxDiffPixelRatio: 0.01,
    });
  });

  test('editor with syntax errors', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('.monaco-editor', { timeout: 10000 });

    // Inject a model with errors via Monaco's API
    await page.evaluate(() => {
      // @ts-ignore
      const editor = window.monaco?.editor?.getEditors()?.[0];
      if (editor) {
        const model = window.monaco.editor.createModel(
          'fn main() { let x = \n }',
          'rust'
        );
        editor.setModel(model);
      }
    });

    // Allow diagnostics to compute
    await page.waitForTimeout(1000);

    const editor = page.locator('.monaco-editor');
    await expect(editor).toHaveScreenshot('editor-with-errors.png', {
      maxDiffPixelRatio: 0.01,
    });
  });
});
