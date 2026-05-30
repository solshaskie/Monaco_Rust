/**
 * Virtual Scroll Renderer (W2.2)
 *
 * A viewport-aware text rendering layer that:
 * - Computes visible lines via WASM layout engine
 * - Renders only visible lines + overscroll buffer
 * - Pools DOM nodes for smooth scrolling
 * - Delegates layout math to WASM (not DOM measurement)
 *
 * This is designed as an alternative view that can be toggled
 * alongside Monaco for large files (>10k lines).
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
    this.totalLines = 0;
    this.scrollTop = 0;
    this.viewportHeight = container.clientHeight || 600;

    // DOM node pool
    this.pool = [];
    this.activeNodes = new Map(); // lineIndex -> DOM node

    this._setupDOM();
    this._bindEvents();
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
    this.totalLines = await countLines(source);
    this._updateSpacer();
    this._renderViewport();
  }

  _updateSpacer() {
    const totalHeight = this.totalLines * this.lineHeight;
    this.spacer.style.height = `${totalHeight}px`;
  }

  /**
   * Compute visible lines using WASM layout engine, then render.
   */
  async _renderViewport() {
    if (!this.source) return;

    const visible = await computeVisibleLines(
      this.source,
      this.lineHeight,
      this.scrollTop,
      this.viewportHeight
    );

    const startLine = Math.max(0, visible.start_line - this.overscroll);
    const endLine = Math.min(this.totalLines, visible.end_line + this.overscroll);

    // Get lines from source
    const lines = this.source.split('\n');

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
      if (this.activeNodes.has(i)) {
        // Node already rendered, just update position
        const node = this.activeNodes.get(i);
        node.style.transform = `translateY(${i * this.lineHeight}px)`;
        continue;
      }

      const lineText = lines[i] || '';
      const node = this._acquireNode(i, lineText);
      node.style.transform = `translateY(${i * this.lineHeight}px)`;
      this.contentLayer.appendChild(node);
      this.activeNodes.set(i, node);
    }
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
   * @param {Array<{line, start, end, type}>} tokens — from WASM tokenizer
   */
  applyDecorations(tokens) {
    for (const tok of tokens) {
      const node = this.activeNodes.get(tok.line);
      if (!node) continue;

      // Simple decoration: wrap token text in a colored span
      // In production, this would use more sophisticated DOM diffing
      const color = TOKEN_COLORS[tok.token_type] || TOKEN_COLORS.identifier;
      const html = this._highlightLine(node.textContent, tok, color);
      if (html !== node.innerHTML) {
        node.innerHTML = html;
      }
    }
  }

  _highlightLine(text, token, color) {
    const before = text.slice(0, token.start);
    const highlighted = text.slice(token.start, token.end);
    const after = text.slice(token.end);
    return (
      this._escapeHtml(before) +
      `<span style="color:${color}">${this._escapeHtml(highlighted)}</span>` +
      this._escapeHtml(after)
    );
  }

  _escapeHtml(str) {
    return str
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;');
  }

  destroy() {
    this.container.removeEventListener('scroll', this._onScroll);
    window.removeEventListener('resize', this._onResize);
    this.contentLayer.remove();
    this.spacer.remove();
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
