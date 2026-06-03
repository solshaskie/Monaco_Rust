/**
 * Monaco Compatibility Shim (W2.5)
 *
 * Bridges Monaco's native APIs to the WASM compute layer.
 * Provides graceful degradation: unimplemented APIs fall back
 * to Monaco's default implementation.
 */

import { WasmDecorationManager, registerTokenStyles } from './decoration-manager.js';
import { computeLayout } from '../../wasm-glue.js';

/**
 * Wraps a Monaco editor instance to intercept key APIs and
 * route compute-heavy operations to WASM.
 */
export class MonacoCompatibilityShim {
  constructor(editor) {
    this.editor = editor;
    this.wasmDecorations = null;
    this.wasmLayout = null; // cached per-model layout from WASM
    this._originalDeltaDecorations = editor.deltaDecorations.bind(editor);
    this._originalGetLayoutInfo = editor.getLayoutInfo.bind(editor);

    // Override deltaDecorations to track decoration IDs
    editor.deltaDecorations = (oldDecorations, newDecorations) => {
      return this._deltaDecorations(oldDecorations, newDecorations);
    };

    // Override getLayoutInfo to inject WASM-computed layout
    editor.getLayoutInfo = () => {
      return this._getLayoutInfo();
    };

    // Widget registries for disposal
    this._codeLensProviders = [];
    this._glyphWidgets = new Map();

    registerTokenStyles();
  }

  /**
   * Register a WASM-backed code lens provider.
   * Returns a disposable handle.
   * @param {string} languageId
   * @param {() => Promise<Array<{range, command}>>} provider
   */
  addCodeLensProvider(languageId, provider) {
    const adapter = new CodeLensAdapter(provider);
    const disposable = monaco.languages.registerCodeLensProvider(languageId, adapter);
    this._codeLensProviders.push(disposable);
    return disposable;
  }

  /**
   * Add a glyph margin widget (e.g. breakpoint indicator).
   * Wraps the widget in GlyphMarginWidgetAdapter for WASM layout compatibility.
   * @param {object} widget — must implement IGlyphMarginWidget
   */
  addGlyphMarginWidget(widget) {
    const adapter = new GlyphMarginWidgetAdapter(widget);
    this.editor.addGlyphMarginWidget(adapter);
    this._glyphWidgets.set(adapter.getId(), adapter);
    return adapter.getId();
  }

  /**
   * Remove a glyph margin widget by id.
   */
  removeGlyphMarginWidget(id) {
    const adapter = this._glyphWidgets.get(id);
    if (adapter) {
      this.editor.removeGlyphMarginWidget(adapter);
      this._glyphWidgets.delete(id);
    }
  }

  /**
   * Asynchronously compute and cache WASM layout for the current model.
   * Call this when the model content or viewport width changes.
   * @param {string} source — full buffer text
   * @param {number} lineWidth — characters per line (viewport width / glyph width)
   */
  async computeWasmLayout(source, lineWidth = 80) {
    try {
      this.wasmLayout = await computeLayout(source, lineWidth);
    } catch (e) {
      console.warn('[CompatibilityShim] WASM layout compute failed:', e);
      this.wasmLayout = null;
    }
  }

  /**
   * Enable WASM-backed tokenization for this editor.
   */
  enableWasmTokenization() {
    if (this.wasmDecorations) return;
    this.wasmDecorations = new WasmDecorationManager(this.editor);

    // Re-tokenize on content change
    const model = this.editor.getModel();
    if (model) {
      this._disposables = [
        model.onDidChangeContent(() => {
          this.wasmDecorations.clearCache();
          this.wasmDecorations.tokenizeViewport(model);
        }),
      ];
      this.wasmDecorations.tokenizeViewport(model);
    }
  }

  _deltaDecorations(oldDecorations, newDecorations) {
    // If WASM decorations are active, merge them with native decorations
    if (this.wasmDecorations) {
      // WASM decorations are managed separately; pass through native ones
      return this._originalDeltaDecorations(oldDecorations, newDecorations);
    }
    return this._originalDeltaDecorations(oldDecorations, newDecorations);
  }

  _getLayoutInfo() {
    const native = this._originalGetLayoutInfo();
    if (!this.wasmLayout) {
      return native;
    }

    // Compute total content height from WASM layout data.
    // Each line contributes (1 + wrap_count) * lineHeight pixels.
    const lineHeight = native.lineHeight || 20;
    let totalLines = 0;
    for (const line of this.wasmLayout) {
      totalLines += 1 + (line.wrap_count || 0);
    }
    const contentHeight = totalLines * lineHeight;

    return {
      ...native,
      contentHeight,
      // Expose whether any line is wrapped so callers can decide
      // to use variable-height layout paths.
      hasWrappedLines: this.wasmLayout.some(l => l.wrapped),
    };
  }

  /**
   * Restore original Monaco APIs.
   */
  dispose() {
    this.editor.deltaDecorations = this._originalDeltaDecorations;
    this.editor.getLayoutInfo = this._originalGetLayoutInfo;
    if (this.wasmDecorations) {
      this.wasmDecorations.dispose();
      this.wasmDecorations = null;
    }
    if (this._disposables) {
      for (const d of this._disposables) {
        d.dispose();
      }
      this._disposables = null;
    }
    for (const disposable of this._codeLensProviders) {
      disposable.dispose();
    }
    this._codeLensProviders = [];
    for (const [id, adapter] of this._glyphWidgets) {
      this.editor.removeGlyphMarginWidget(adapter);
    }
    this._glyphWidgets.clear();
  }
}

/**
 * Wrap an existing Monaco editor with the compatibility shim.
 */
export function shimMonacoEditor(editor) {
  return new MonacoCompatibilityShim(editor);
}

/**
 * Adapter for Monaco's ICodeLensProvider that delegates to a WASM
 * (or any async) compute function.
 */
export class CodeLensAdapter {
  constructor(provider) {
    this.provider = provider;
  }

  async provideCodeLenses(model, _token) {
    try {
      const lenses = await this.provider(model);
      return {
        lenses: lenses.map((lens) => ({
          range: new monaco.Range(
            lens.range.startLineNumber,
            lens.range.startColumn || 1,
            lens.range.endLineNumber,
            lens.range.endColumn || 1
          ),
          id: lens.id,
          command: lens.command,
        })),
        dispose: () => { },
      };
    } catch (e) {
      console.warn('[CodeLensAdapter] Provider failed:', e);
      return { lenses: [], dispose: () => { } };
    }
  }

  resolveCodeLens(_model, codeLens, _token) {
    return codeLens;
  }
}

/**
 * Compatibility layer for IContentWidget.
 * Wraps a widget so it works with both native and WASM layout.
 */
export class ContentWidgetAdapter {
  constructor(widget) {
    this.widget = widget;
  }

  getId() {
    return this.widget.getId?.() || 'wasm-widget';
  }

  getDomNode() {
    return this.widget.getDomNode?.();
  }

  getPosition() {
    // Merge native position with WASM layout adjustments if needed
    return this.widget.getPosition?.();
  }
}

/**
 * Compatibility layer for IGlyphMarginWidget.
 */
export class GlyphMarginWidgetAdapter {
  constructor(widget) {
    this.widget = widget;
  }

  getId() {
    return this.widget.getId?.() || 'wasm-glyph-widget';
  }

  getDomNode() {
    return this.widget.getDomNode?.();
  }

  getPosition() {
    return this.widget.getPosition?.();
  }
}
