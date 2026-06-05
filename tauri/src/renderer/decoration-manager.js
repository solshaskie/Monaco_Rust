/**
 * Decoration Manager (W2.3)
 *
 * Batches decoration updates from WASM compute modules and applies
 * minimal DOM mutations. Works with both Monaco's native renderer
 * and the virtual scroll renderer.
 */

import { tokenizeRangeCached } from '../../wasm-glue.js';

function getModelPath(model) {
  if (!model || !model.uri) {
    return null;
  }
  if (model.uri.scheme === 'file') {
    return model.uri.path;
  }
  return model.uri.toString();
}

function getWasmSyncSource(model) {
  const path = getModelPath(model);
  const manager = window.wasmBufferSync;
  if (path && manager && typeof manager.getContent === 'function') {
    const content = manager.getContent(path);
    if (typeof content === 'string') {
      return content;
    }
  }
  return model.getValue();
}

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
    this.currentDecorationsByLine = new Map(); // lineIndex -> [decId, ...]
    this.tokenCache = new Map(); // `${modelUri}@${versionId}` -> tokens
    this.tokenizationGen = new Map(); // model uri -> generation counter
    this.lastLineDecorations = new Map(); // lineIndex -> Array<dec object hash>
    this.latestTokensByUri = new Map(); // model uri -> most recent stable tokens

    // Inline widgets (parameter hints, inlay hints) — updated independently
    this.inlineWidgets = new Map(); // widgetId -> {line, column, html, type}
    this.inlineWidgetDecorations = []; // decoration ids managed by Monaco
    this.inlineWidgetBatch = new DecorationBatch();
    this.inlineWidgetBatch.onFlush = (batch) => this._applyInlineWidgets(batch);
  }

  /**
   * Add an inline widget (inlay hint or parameter hint).
   * Does NOT trigger a full re-tokenization or re-layout.
   * @param {string} id — unique widget id
   * @param {number} line — 0-indexed line
   * @param {number} column — 0-indexed column
   * @param {string} html — inline HTML content (e.g. `: Type` or param name)
   * @param {string} type — 'inlay' | 'parameter'
   */
  addInlineWidget(id, line, column, html, type = 'inlay') {
    this.inlineWidgets.set(id, { line, column, html, type });
    this.inlineWidgetBatch.queue(line, [{ id, column, html, type }]);
  }

  /**
   * Remove a single inline widget by id.
   */
  removeInlineWidget(id) {
    if (!this.inlineWidgets.has(id)) return;
    const widget = this.inlineWidgets.get(id);
    this.inlineWidgets.delete(id);
    // Re-apply all widgets for this line to remove the deleted one
    const lineWidgets = [];
    for (const [wid, w] of this.inlineWidgets) {
      if (w.line === widget.line) {
        lineWidgets.push({ id: wid, column: w.column, html: w.html, type: w.type });
      }
    }
    this.inlineWidgetBatch.queue(widget.line, lineWidgets);
  }

  /**
   * Clear all inline widgets.
   */
  clearInlineWidgets() {
    this.inlineWidgets.clear();
    this.inlineWidgetBatch.queue(0, []);
  }

  _applyInlineWidgets(batch) {
    if (!batch) {
      return;
    }

    const decorations = [];
    for (const widget of this.inlineWidgets.values()) {
      const className = widget.type === 'parameter'
        ? 'wasm-inline-parameter'
        : 'wasm-inline-inlay';
      decorations.push({
        range: new monaco.Range(widget.line + 1, widget.column + 1, widget.line + 1, widget.column + 1),
        options: {
          after: {
            content: widget.html,
            inlineClassName: className,
          },
        },
      });
    }
    this.inlineWidgetDecorations = this.editor.deltaDecorations(
      this.inlineWidgetDecorations,
      decorations
    );
  }

  /**
   * Tokenize the visible viewport and queue decoration updates.
   */
  async tokenizeViewport(model) {
    const path = getModelPath(model);
    const uri = model.uri.toString();
    const manager = window.wasmBufferSync;

    if (path && manager && manager.isSyncBarrierActive(path)) {
      const cachedTokens = this.latestTokensByUri.get(uri);
      if (cachedTokens) {
        this._queueFromTokens(cachedTokens);
      }
      return;
    }

    const source = getWasmSyncSource(model);
    const language = model.getLanguageId();
    const versionId = model.getVersionId();
    const cacheKey = `${uri}@${versionId}`;

    // Check cache
    if (this.tokenCache.has(cacheKey)) {
      this._queueFromTokens(this.tokenCache.get(cacheKey));
      return;
    }

    // Get visible range from Monaco
    const visibleRanges = this.editor.getVisibleRanges();
    if (!visibleRanges.length) return;

    const startLine = visibleRanges[0].startLineNumber - 1;
    const endLine = visibleRanges[visibleRanges.length - 1].endLineNumber;
    const resource = path || uri;

    // Generation-based cancellation: discard stale results
    const gen = (this.tokenizationGen.get(uri) || 0) + 1;
    this.tokenizationGen.set(uri, gen);

    try {
      const tokens = await tokenizeRangeCached(source, language, resource, startLine, endLine);
      if (this.tokenizationGen.get(uri) !== gen) {
        // A newer tokenization request arrived; discard this result
        return;
      }
      this.tokenCache.set(cacheKey, tokens);
      this.latestTokensByUri.set(uri, tokens);
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
    // Build per-line decoration hashes to detect dirty regions.
    // Only changed lines are passed to deltaDecorations; unchanged
    // lines are left alone, avoiding a full getAllDecorations() scan.
    const newDecorations = [];
    const oldIdsToRemove = [];
    const newLineDecorations = new Map();

    for (const [line, decs] of batch) {
      const hash = decs.map(d => `${d.start}:${d.end}:${d.color}`).join('|');
      const lastHash = this.lastLineDecorations.get(line);
      newLineDecorations.set(line, hash);

      if (lastHash !== hash) {
        for (const dec of decs) {
          newDecorations.push({
            range: new monaco.Range(line + 1, dec.start + 1, line + 1, dec.end + 1),
            options: {
              inlineClassName: `token-${dec.color.replace('#', '')}`,
              overviewRuler: { color: dec.color, position: monaco.editor.OverviewRulerLane.Full },
            },
          });
        }
        const oldIds = this.currentDecorationsByLine.get(line);
        if (oldIds) {
          oldIdsToRemove.push(...oldIds);
        }
      }
    }

    // Remove stale IDs from the master list
    if (oldIdsToRemove.length) {
      const removeSet = new Set(oldIdsToRemove);
      this.currentDecorations = this.currentDecorations.filter(id => !removeSet.has(id));
    }

    // Apply minimal delta; Monaco preserves everything not in oldIdsToRemove
    const newIds = this.editor.deltaDecorations(oldIdsToRemove, newDecorations);
    this.currentDecorations.push(...newIds);

    // Map returned IDs back to their lines
    let idIdx = 0;
    for (const [line, decs] of batch) {
      const lastHash = this.lastLineDecorations.get(line);
      const hash = newLineDecorations.get(line);
      if (lastHash !== hash) {
        const count = decs.length;
        this.currentDecorationsByLine.set(line, newIds.slice(idIdx, idIdx + count));
        idIdx += count;
      }
    }

    for (const [line, hash] of newLineDecorations) {
      this.lastLineDecorations.set(line, hash);
    }
  }

  clearCache() {
    this.tokenCache.clear();
    this.lastLineDecorations.clear();
    this.currentDecorationsByLine.clear();
    this.latestTokensByUri.clear();
  }

  dispose() {
    this.batch.dispose();
    this.inlineWidgetBatch.dispose();
    const allIds = [];
    for (const ids of this.currentDecorationsByLine.values()) {
      allIds.push(...ids);
    }
    this.editor.deltaDecorations(allIds, []);
    this.currentDecorations = [];
    this.currentDecorationsByLine.clear();
    this.lastLineDecorations.clear();
    this.editor.deltaDecorations(this.inlineWidgetDecorations, []);
    this.inlineWidgetDecorations = [];
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
    .wasm-inline-inlay {
      color: #808080;
      font-size: 0.9em;
      opacity: 0.8;
      pointer-events: none;
      user-select: none;
    }
    .wasm-inline-parameter {
      color: #ce9178;
      font-weight: bold;
      background: rgba(206,145,120,0.1);
      border-radius: 2px;
      padding: 0 2px;
      pointer-events: none;
      user-select: none;
    }
  `;
  document.head.appendChild(style);
}
