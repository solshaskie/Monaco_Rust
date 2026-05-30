/**
 * Monaco Compatibility Shim (W2.5)
 *
 * Bridges Monaco's native APIs to the WASM compute layer.
 * Provides graceful degradation: unimplemented APIs fall back
 * to Monaco's default implementation.
 */

import { WasmDecorationManager, registerTokenStyles } from './decoration-manager.js';

/**
 * Wraps a Monaco editor instance to intercept key APIs and
 * route compute-heavy operations to WASM.
 */
export class MonacoCompatibilityShim {
  constructor(editor) {
    this.editor = editor;
    this.wasmDecorations = null;
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

    registerTokenStyles();
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
    // In production, this would merge WASM-computed layout metrics
    // (line heights, glyph positions) with native layout.
    return native;
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
  }
}

/**
 * Wrap an existing Monaco editor with the compatibility shim.
 */
export function shimMonacoEditor(editor) {
  return new MonacoCompatibilityShim(editor);
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
