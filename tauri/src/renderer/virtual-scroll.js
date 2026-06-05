/**
 * Virtual Scroll Renderer (W2.2)
 *
 * A viewport-aware text rendering layer that replaces Monaco's
 * native renderer for large buffers (>10k lines).
 *
 * Design principles:
 * - Computes visible lines via WASM layout engine (not DOM measurement)
 * - Renders only visible lines + overscroll buffer
 * - Pools DOM nodes for smooth scrolling
 * - Decoration updates are DOM-diffed to prevent flicker
 *
 * When active, this renderer owns the editor container and
 * delegates edit/model events back to the host Monaco instance.
 * The compatibility shim (`compatibility.js`) wires the two
 * together so Monaco upgrades do not require renderer changes.
 */

import { computeVisibleLines, computeLayout, countLines } from '../../wasm-glue.js';

const OVERSCROLL_LINES = 50;
const LINE_HEIGHT_PX = 20;

export class VirtualScrollRenderer {
  constructor(container, options = {}) {
    this.container = container;
    this.lineHeight = options.lineHeight || LINE_HEIGHT_PX;
    this.overscroll = options.overscroll || OVERSCROLL_LINES;
    this.source = '';
    this.lines = [];
    this.loadedLines = new Map();
    this.totalLines = 0;
    this.scrollTop = 0;
    this.viewportHeight = container.clientHeight || 600;
    this.wasmLayout = null; // per-line layout from WASM (for variable heights)
    this._mountedMonacoLinesContent = null;
    this.sparseLoader = null;
    this.pendingSparseLoads = new Map();

    // DOM node pool
    this.pool = [];
    this.activeNodes = new Map(); // lineIndex -> DOM node
    this.lastTokenHashByLine = new Map(); // lineIndex -> hash of last applied tokens

    this._setupDOM();
    this._bindEvents();
  }

  /**
   * Set per-line layout data from WASM compute module.
   * Enables variable-height line rendering.
   */
  setLayout(layout) {
    this.wasmLayout = layout;
    this._updateSpacer();
    this._renderViewport();
  }

  _setupDOM() {
    this.container.style.overflow = 'auto';
    this.container.style.position = 'relative';

    // Spacer to create the full scrollable height
    this.spacer = document.createElement('div');
    this.spacer.style.position = 'absolute';
    this.spacer.style.top = '0';
    this.spacer.style.left = '0';
    this.spacer.style.width = '1px';
    this.container.appendChild(this.spacer);

    // Content layer for visible lines
    this.contentLayer = document.createElement('div');
    this.contentLayer.style.position = 'absolute';
    this.contentLayer.style.top = '0';
    this.contentLayer.style.left = '0';
    this.contentLayer.style.width = '100%';
    this.contentLayer.style.pointerEvents = 'none'; // Let scroll events pass through
    this.container.appendChild(this.contentLayer);
  }

  _bindEvents() {
    this._onScroll = () => {
      this.scrollTop = this.container.scrollTop;
      this._renderViewport();
    };
    this.container.addEventListener('scroll', this._onScroll, { passive: true });

    this._onResize = () => {
      this.viewportHeight = this.container.clientHeight;
      this._updateSpacer();
      this._renderViewport();
    };
    window.addEventListener('resize', this._onResize);
  }

  /**
   * Set the source text and update layout.
   */
  async setSource(source) {
    this.source = source;
    this.sparseLoader = null;
    this.lines = source.split('\n');
    this.loadedLines.clear();
    this.totalLines = await countLines(source);
    this._updateSpacer();
    this._renderViewport();
  }

  async setSparseSource(totalLines, sparseLoader) {
    this.source = '__sparse__';
    this.lines = [];
    this.loadedLines.clear();
    this.totalLines = totalLines;
    this.sparseLoader = sparseLoader;
    this.pendingSparseLoads.clear();
    this._updateSpacer();
    this._renderViewport();
  }

  _updateSpacer() {
    let totalHeight;
    if (this.wasmLayout) {
      // Variable height: each line contributes (1 + wrap_count) * lineHeight
      totalHeight = this.wasmLayout.reduce((sum, line) => {
        return sum + (1 + (line.wrap_count || 0)) * this.lineHeight;
      }, 0);
    } else {
      totalHeight = this.totalLines * this.lineHeight;
    }
    this.spacer.style.height = `${totalHeight}px`;
  }

  /**
   * Compute visible lines using WASM layout engine, then render.
   */
  async _renderViewport() {
    if (!this.source) return;

    const visible = await computeVisibleLines(
      this.totalLines,
      this.lineHeight,
      this.scrollTop,
      this.viewportHeight
    );

    const startLine = Math.max(0, visible.start_line - this.overscroll);
    const endLine = Math.min(this.totalLines, visible.end_line + this.overscroll);
    this._requestSparseViewport(startLine, endLine);

    // Precompute cumulative Y offsets for variable-height lines
    const yOffsets = new Array(endLine + 1);
    yOffsets[0] = 0;
    for (let i = 0; i < endLine; i++) {
      const h = this._lineHeight(i);
      yOffsets[i + 1] = yOffsets[i] + h;
    }

    // Recycle nodes that are no longer needed
    const neededLines = new Set();
    for (let i = startLine; i < endLine; i++) {
      neededLines.add(i);
    }

    // Return unused nodes to pool
    for (const [lineIdx, node] of this.activeNodes) {
      if (!neededLines.has(lineIdx)) {
        node.remove();
        this.pool.push(node);
        this.activeNodes.delete(lineIdx);
      }
    }

    // Render needed lines
    for (let i = startLine; i < endLine; i++) {
      const y = yOffsets[i];
      if (this.activeNodes.has(i)) {
        // Node already rendered, just update position
        const node = this.activeNodes.get(i);
        node.style.transform = `translateY(${y}px)`;
        continue;
      }

      const lineText = this._getLineText(i);
      const node = this._acquireNode(i, lineText);
      node.style.transform = `translateY(${y}px)`;
      node.style.height = `${this._lineHeight(i)}px`;
      this.contentLayer.appendChild(node);
      this.activeNodes.set(i, node);
    }
  }

  _getLineText(lineIndex) {
    if (this.sparseLoader) {
      return this.loadedLines.get(lineIndex) || '';
    }
    return this.lines[lineIndex] || '';
  }

  _requestSparseViewport(startLine, endLine) {
    if (!this.sparseLoader || endLine <= startLine) {
      return;
    }
    const key = `${startLine}:${endLine}`;
    if (this.pendingSparseLoads.has(key)) {
      return;
    }
    let missing = false;
    for (let i = startLine; i < endLine; i++) {
      if (!this.loadedLines.has(i)) {
        missing = true;
        break;
      }
    }
    if (!missing) {
      return;
    }

    const promise = Promise.resolve(this.sparseLoader(startLine, endLine - startLine))
      .then((slice) => {
        if (!slice || typeof slice.content !== 'string') {
          return;
        }
        const lines = slice.content.split('\n');
        if (lines.length && lines[lines.length - 1] === '') {
          lines.pop();
        }
        const baseLine = typeof slice.start_line === 'number' ? slice.start_line : startLine;
        for (let i = 0; i < lines.length; i++) {
          this.loadedLines.set(baseLine + i, lines[i]);
        }
        this._renderViewport();
      })
      .catch((error) => {
        console.warn('[VirtualScrollRenderer] Sparse viewport load failed:', error);
      })
      .finally(() => {
        this.pendingSparseLoads.delete(key);
      });
    this.pendingSparseLoads.set(key, promise);
  }

  _lineHeight(lineIndex) {
    if (this.wasmLayout && this.wasmLayout[lineIndex]) {
      return (1 + (this.wasmLayout[lineIndex].wrap_count || 0)) * this.lineHeight;
    }
    return this.lineHeight;
  }

  _acquireNode(lineIndex, text) {
    let node;
    if (this.pool.length > 0) {
      node = this.pool.pop();
      node.textContent = text;
    } else {
      node = document.createElement('div');
      node.style.position = 'absolute';
      node.style.left = '0';
      node.style.width = '100%';
      node.style.height = `${this.lineHeight}px`;
      node.style.whiteSpace = 'pre';
      node.style.fontFamily = 'monospace';
      node.style.fontSize = '13px';
      node.style.lineHeight = `${this.lineHeight}px`;
      node.style.color = '#d4d4d4';
      node.textContent = text;
    }
    node.dataset.line = lineIndex;
    return node;
  }

  /**
   * Apply syntax highlighting decorations to visible lines.
   * Uses per-line token hashing to avoid DOM rewrites when
   * nothing changed, preventing scroll / cursor flicker.
   *
   * @param {Array<{line, start, end, type}>} tokens — from WASM tokenizer
   */
  applyDecorations(tokens) {
    // Group tokens by line so we can hash per line
    const byLine = new Map();
    for (const tok of tokens) {
      const arr = byLine.get(tok.line) || [];
      arr.push(tok);
      byLine.set(tok.line, arr);
    }

    // Clear stale highlighting for visible lines that no longer have tokens.
    for (const [line, node] of this.activeNodes) {
      if (!byLine.has(line) && this.lastTokenHashByLine.has(line)) {
        node.textContent = this._getLineText(line);
        this.lastTokenHashByLine.delete(line);
      }
    }

    for (const [line, lineTokens] of byLine) {
      const node = this.activeNodes.get(line);
      if (!node) continue;

      const hash = lineTokens
        .map(t => `${t.start_column}:${t.end_column}:${t.token_type}`)
        .join('|');
      const lastHash = this.lastTokenHashByLine.get(line);
      if (hash === lastHash) continue; // No change → skip DOM write

      const color = TOKEN_COLORS[lineTokens[0].token_type] || TOKEN_COLORS.identifier;
      const html = this._highlightLine(node.textContent, lineTokens, color);
      if (html !== node.innerHTML) {
        node.innerHTML = html;
      }
      this.lastTokenHashByLine.set(line, hash);
    }
  }

  _highlightLine(text, tokens, defaultColor) {
    // Sort tokens by line-local columns. WASM token byte offsets are absolute
    // within the parsed source, so renderer-side slicing must use columns.
    const sorted = tokens.slice().sort((a, b) => {
      const aStart = Math.max(0, (a.start_column || 1) - 1);
      const bStart = Math.max(0, (b.start_column || 1) - 1);
      return aStart - bStart;
    });
    let result = '';
    let pos = 0;
    for (const tok of sorted) {
      const start = Math.max(pos, Math.max(0, (tok.start_column || 1) - 1));
      const end = Math.max(start, Math.max(0, (tok.end_column || tok.start_column || 1) - 1));
      if (start > pos) {
        result += this._escapeHtml(text.slice(pos, start));
      }
      if (end > start) {
        const color = TOKEN_COLORS[tok.token_type] || defaultColor;
        result += `<span style="color:${color}">${this._escapeHtml(text.slice(start, end))}</span>`;
      }
      pos = Math.max(pos, end);
    }
    if (pos < text.length) {
      result += this._escapeHtml(text.slice(pos));
    }
    return result;
  }

  _escapeHtml(str) {
    return str
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;');
  }

  /**
   * Mount this renderer as the primary view inside an existing
   * Monaco editor container, hiding Monaco's native lines layer.
   *
   * When used with MonacoCompatibilityShim, the shim wires model
   * events here and the renderer sends edits back via the shim's
   * `onEdit` callback. This isolates Monaco upgrade concerns to
   * the shim while the renderer remains independent.
   */
  mountAsPrimary(monacoEditor, shim) {
    const container = monacoEditor.getContainerDomNode();
    // Hide Monaco's own line rendering layer but keep the model
    const monacoDom = container.querySelector('.lines-content');
    if (monacoDom) {
      monacoDom.style.visibility = 'hidden';
      this._mountedMonacoLinesContent = monacoDom;
    }
    const containerChanged = this.container && this.container !== container;
    if (containerChanged) {
      this.container.removeEventListener('scroll', this._onScroll);
    }
    // Move our content layer into Monaco's container
    container.appendChild(this.contentLayer);
    container.appendChild(this.spacer);
    this.container = container;
    if (containerChanged) {
      this.container.addEventListener('scroll', this._onScroll, { passive: true });
    }
    this._onResize();

    // Wire model changes back through the shim
    if (shim && shim.editor) {
      const model = shim.editor.getModel();
      if (model) {
        this.setSource(model.getValue());
      }
    }
  }

  unmountPrimary() {
    if (this._mountedMonacoLinesContent) {
      this._mountedMonacoLinesContent.style.visibility = '';
      this._mountedMonacoLinesContent = null;
    }
  }

  destroy() {
    this.container.removeEventListener('scroll', this._onScroll);
    window.removeEventListener('resize', this._onResize);
    this.unmountPrimary();
    this.contentLayer.remove();
    this.spacer.remove();
    this.lastTokenHashByLine.clear();
    this.pendingSparseLoads.clear();
    this.loadedLines.clear();
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
 * Factory: create a virtual scroll renderer for a container.
 */
export async function createVirtualScrollEditor(containerId, source) {
  const container = document.getElementById(containerId);
  if (!container) throw new Error(`Container #${containerId} not found`);

  const renderer = new VirtualScrollRenderer(container);
  await renderer.setSource(source);
  return renderer;
}
