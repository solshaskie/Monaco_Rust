import { test, expect } from '@playwright/test';

declare global {
  interface Window {
    __monacoRustTest?: {
      getRendererState: () => {
        hasEditor: boolean;
        hasShim: boolean;
        isVirtualRendererActive: boolean;
        hiddenMonacoLines: boolean;
        totalLines: number;
        activeLineCount: number;
        firstActiveLine: number | null;
        lastActiveLine: number | null;
      };
      createLargeModel: (options?: { lineCount?: number; languageId?: string; path?: string }) => Promise<unknown>;
      scrollEditor: (scrollTop: number) => Promise<unknown>;
      editLine: (lineNumber: number, text: string) => Promise<string>;
      getLineText: (lineNumber: number) => string | null;
      setSmallModel: () => Promise<unknown>;
    };
  }
}

test.describe('Large Buffer Renderer Proof', () => {
  test('large model auto-promotes to virtual renderer and downshifts cleanly', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('.monaco-editor', { timeout: 10000 });

    await page.waitForFunction(() => !!window.__monacoRustTest?.getRendererState().hasShim, null, {
      timeout: 10000,
    });

    const initialState = await page.evaluate(async () => {
      return window.__monacoRustTest?.createLargeModel({
        lineCount: 12050,
        languageId: 'rust',
        path: '/tmp/large-buffer-proof.rs',
      });
    });

    expect(initialState).toBeTruthy();

    await page.waitForFunction(() => {
      const state = window.__monacoRustTest?.getRendererState();
      return !!state?.isVirtualRendererActive && !!state?.hiddenMonacoLines;
    }, null, { timeout: 10000 });

    const promotedState = await page.evaluate(() => window.__monacoRustTest?.getRendererState());
    expect(promotedState?.totalLines).toBe(12050);
    expect(promotedState?.activeLineCount).toBeGreaterThan(0);
    expect(promotedState?.hiddenMonacoLines).toBe(true);

    const scrolledState = await page.evaluate(async () => {
      return window.__monacoRustTest?.scrollEditor(240000);
    });
    expect(scrolledState).toBeTruthy();

    const afterScroll = await page.evaluate(() => window.__monacoRustTest?.getRendererState());
    expect(afterScroll?.firstActiveLine).not.toBeNull();
    expect(afterScroll?.lastActiveLine).not.toBeNull();
    expect((afterScroll?.lastActiveLine ?? 0)).toBeGreaterThan(afterScroll?.firstActiveLine ?? 0);

    const editedLine = await page.evaluate(async () => {
      return window.__monacoRustTest?.editLine(6001, 'fn focus_line() { let marker = 99; }');
    });
    expect(editedLine).toContain('marker = 99');

    const modelLine = await page.evaluate(() => window.__monacoRustTest?.getLineText(6001));
    expect(modelLine).toContain('marker = 99');

    const smallState = await page.evaluate(async () => {
      return window.__monacoRustTest?.setSmallModel();
    });
    expect(smallState).toBeTruthy();

    await page.waitForFunction(() => {
      const state = window.__monacoRustTest?.getRendererState();
      return !!state && !state.isVirtualRendererActive;
    }, null, { timeout: 10000 });

    const afterDownshift = await page.evaluate(() => window.__monacoRustTest?.getRendererState());
    expect(afterDownshift?.isVirtualRendererActive).toBe(false);
    expect(afterDownshift?.hiddenMonacoLines).toBe(false);
  });
});
