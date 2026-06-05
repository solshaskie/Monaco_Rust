import { test, expect } from '@playwright/test';
import fs from 'fs';
import os from 'os';
import path from 'path';

declare global {
  interface Window {
    __monacoRustTest?: {
      openSparseLargeDocument: (path: string) => Promise<{
        session_id: string;
        path: string;
        language_id: string;
        byte_length: number;
        line_count: number;
        checkpoint_stride: number;
      }>;
      readSparseViewport: (
        sessionId: string,
        startLine: number,
        lineCount: number
      ) => Promise<{
        session_id: string;
        start_line: number;
        end_line: number;
        content: string;
      }>;
      closeSparseLargeDocument: (sessionId: string) => Promise<{ closed: boolean }>;
    };
  }
}

test.describe('Sparse Large Document Seam', () => {
  test('opens and reads viewport slices through sparse host session', async ({ page }) => {
    const tempFile = path.join(os.tmpdir(), `monaco-sparse-proof-${Date.now()}.log`);
    const content = Array.from({ length: 15000 }, (_, i) => `log-line-${i}\n`).join('');
    fs.writeFileSync(tempFile, content, 'utf8');

    await page.goto('/');
    await page.waitForSelector('.monaco-editor', { timeout: 10000 });
    await page.waitForFunction(() => !!window.__monacoRustTest, null, { timeout: 10000 });

    const opened = await page.evaluate(async (filePath) => {
      return window.__monacoRustTest?.openSparseLargeDocument(filePath);
    }, tempFile);

    expect(opened?.line_count).toBe(15000);
    expect(opened?.byte_length).toBeGreaterThan(0);

    const viewport = await page.evaluate(async (payload) => {
      return window.__monacoRustTest?.readSparseViewport(
        payload.sessionId,
        payload.startLine,
        payload.lineCount
      );
    }, {
      sessionId: opened?.session_id,
      startLine: 12048,
      lineCount: 3,
    });

    expect(viewport?.start_line).toBe(12048);
    expect(viewport?.end_line).toBe(12051);
    expect(viewport?.content).toContain('log-line-12048');
    expect(viewport?.content).toContain('log-line-12050');

    const closed = await page.evaluate(async (sessionId) => {
      return window.__monacoRustTest?.closeSparseLargeDocument(sessionId);
    }, opened?.session_id);
    expect(closed?.closed).toBe(true);

    fs.rmSync(tempFile, { force: true });
  });
});
