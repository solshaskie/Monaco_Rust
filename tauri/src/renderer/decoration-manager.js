/**
 * Decoration Manager (W2.3)
 *
 * Batches decoration updates from WASM compute modules and applies
 * minimal DOM mutations. Works with both Monaco's native renderer
 * and the virtual scroll renderer.
 */

import { tokenizeRange } from '../../wasm-glue.js';

/**
 * A decoration batch that accumulates updates and flushes them
 * on the next animation frame.
 */
export class DecorationBatch {
  constructor() {
    this.pending = new Map(); // line -> Array<Decoration>
    this.rafId = null;
  }

  queue(line, decorations) {
    const existing = this.pending.get(line) || [];
    this.pending.set(line, existing.concat(decorations));
    this._scheduleFlush();
  }

  _scheduleFlush() {
    if (this.rafId !== null) return;
    this.rafId = requestAnimationFrame(() => this._flush());
  }

  _flush() {
    this.rafId = null;
    const batch = new Map(this.pending);
    this.pending.clear();
    this.onFlush(batch);
  }

  onFlush(_batch) {
    // Override in subclasses
  }

  dispose() {
    if (this.rafId !== null) {
      cancelAnimationFrame(this.rafId);
      this.rafId = null;
    }
    this.pending.clear();
  }
}

/**
 * Monaco-compatible decoration manager that uses WASM tokenization.
 */
export class WasmDecorationManager {
  constructor(monacoEditor) {
    this.editor = monacoEditor;
    this.batch = new DecorationBatch();
    this.batch.onFlush = (batch) => this._applyBatch(batch);
    this.currentDecorations = [];
    this.tokenCache = new Map(); // versionId -> tokens
  }

  /**
   * Tokenize the visible viewport and queue decoration updates.
   */
  async tokenizeViewport(model) {
    const source = model.getValue();
    const language = model.getLanguageId();
    const versionId = model.getVersionId();

    // Check cache
    if (this.tokenCache.has(versionId)) {
      this._queueFromTokens(this.tokenCache.get(versionId));
      return;
    }

    // Get visible range from Monaco
    const visibleRanges = this.editor.getVisibleRanges();
    if (!visibleRanges.length) return;

    const startLine = visibleRanges[0].startLineNumber - 1;
    const endLine = visibleRanges[visibleRanges.length - 1].endLineNumber;

    try {
      const tokens = await tokenizeRange(source, language, startLine, endLine);
      this.tokenCache.set(versionId, tokens);
      this._queueFromTokens(tokens);
    } catch (e) {
      console.warn('[DecorationManager] Tokenization failed:', e);
    }
  }

  _queueFromTokens(tokens) {
    for (const tok of tokens) {
      const color = TOKEN_COLORS[tok.token_type] || TOKEN_COLORS.identifier;
      this.batch.queue(tok.line, [{
        start: tok.start,
        end: tok.end,
        color,
      }]);
    }
  }

  _applyBatch(batch) {
    const oldIds = this.currentDecorations;
    const newDecorations = [];

    for (const [line, decs] of batch) {
      for (const dec of decs) {
        newDecorations.push({
          range: new monaco.Range(line + 1, dec.start + 1, line + 1, dec.end + 1),
          options: {
            inlineClassName: `token-${dec.color.replace('#', '')}`,
            overviewRuler: { color: dec.color, position: monaco.editor.OverviewRulerLane.Full },
          },
        });
      }
    }

    // Use Monaco's deltaDecorations for minimal DOM mutation
    this.currentDecorations = this.editor.deltaDecorations(oldIds, newDecorations);
  }

  clearCache() {
    this.tokenCache.clear();
  }

  dispose() {
    this.batch.dispose();
    this.editor.deltaDecorations(this.currentDecorations, []);
    this.currentDecorations = [];
  }
}

const TOKEN_COLORS = {
  keyword: '#569cd6',
  identifier: '#d4d4d4',
  string: '#ce9178',
  number: '#b5cea8',
  comment: '#6a9955',
  operator: '#d4d4d4',
  punctuation: '#d4d4d4',
};

/**
 * Register dynamic CSS for token colors.
 */
export function registerTokenStyles() {
  const style = document.createElement('style');
  style.textContent = `
    .token-569cd6 { color: #569cd6 !important; }
    .token-ce9178 { color: #ce9178 !important; }
    .token-b5cea8 { color: #b5cea8 !important; }
    .token-6a9955 { color: #6a9955 !important; }
  `;
  document.head.appendChild(style);
}
