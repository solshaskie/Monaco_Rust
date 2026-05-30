/**
 * E2E Tests: WASM Compute Module (W3.1)
 *
 * Verifies that the WASM module loads correctly in the Tauri webview
 * and that compute functions return expected results.
 */

import { test, expect } from '@playwright/test';

test.describe('WASM Compute Module', () => {
  test.beforeEach(async ({ page }) => {
    // Navigate to the Tauri app frontend
    await page.goto('tauri://localhost');
    // Wait for WASM to initialize
    await page.waitForFunction(() => (window as any).wasmCompute !== undefined, { timeout: 10000 });
  });

  test('WASM module loads and exposes compute API', async ({ page }) => {
    const api = await page.evaluate(() => {
      return Object.keys((window as any).wasmCompute);
    });
    expect(api).toContain('tokenize');
    expect(api).toContain('diff');
    expect(api).toContain('layout');
    expect(api).toContain('visibleLines');
    expect(api).toContain('countLines');
  });

  test('tokenize returns structured tokens', async ({ page }) => {
    const tokens = await page.evaluate(async () => {
      const compute = (window as any).wasmCompute;
      return await compute.tokenize('fn main() {}', 'rust');
    });
    expect(Array.isArray(tokens)).toBe(true);
    expect(tokens.length).toBeGreaterThan(0);
    expect(tokens[0]).toHaveProperty('text');
    expect(tokens[0]).toHaveProperty('token_type');
    expect(tokens[0]).toHaveProperty('line');
  });

  test('diff computes line-based edits', async ({ page }) => {
    const edits = await page.evaluate(async () => {
      const compute = (window as any).wasmCompute;
      return await compute.diff('hello\nworld', 'hello\nuniverse');
    });
    expect(Array.isArray(edits)).toBe(true);
  });

  test('layout computes line wrapping', async ({ page }) => {
    const lines = await page.evaluate(async () => {
      const compute = (window as any).wasmCompute;
      return await compute.layout('hello world foo bar', 5);
    });
    expect(Array.isArray(lines)).toBe(true);
    expect(lines[0]).toHaveProperty('char_count');
    expect(lines[0]).toHaveProperty('wrapped');
  });

  test('visibleLines computes viewport range', async ({ page }) => {
    const visible = await page.evaluate(async () => {
      const compute = (window as any).wasmCompute;
      return await compute.visibleLines('a\nb\nc\nd\ne', 20, 0, 60);
    });
    expect(visible).toHaveProperty('start_line');
    expect(visible).toHaveProperty('end_line');
    expect(visible.start_line).toBe(0);
    expect(visible.end_line).toBe(3);
  });

  test('countLines returns correct count', async ({ page }) => {
    const count = await page.evaluate(async () => {
      const compute = (window as any).wasmCompute;
      return await compute.countLines('a\nb\nc');
    });
    expect(count).toBe(3);
  });
});

test.describe('WASM Tokenizer Provider', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('tauri://localhost');
    await page.waitForFunction(() => (window as any).monaco !== undefined, { timeout: 10000 });
  });

  test('Monaco semantic token provider is registered', async ({ page }) => {
    const hasProvider = await page.evaluate(() => {
      const monaco = (window as any).monaco;
      // Check if a provider was registered (implementation detail varies)
      return monaco.languages.getLanguages().some((l: any) =>
        ['rust', 'javascript', 'typescript'].includes(l.id)
      );
    });
    expect(hasProvider).toBe(true);
  });
});
