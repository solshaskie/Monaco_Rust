/**
 * WASM Compute Glue Layer — Hybrid Architecture (Native Rust + WASM Compute)
 *
 * Loads the monaco-wasm module and exposes a clean API for:
 *   - tokenize(source, language) → SemanticTokens
 *   - diff(oldText, newText) → ContentChange[]
 *   - layout(source, lineWidth) → LineLayout[]
 *   - visibleLines(source, lineHeight, scrollTop, viewportHeight) → { startLine, endLine }
 *
 * This module is designed for the Tauri webview context.
 */

let wasmModule = null;
let initPromise = null;

/**
 * Initialize the WASM compute module.
 * Call once before any compute operations.
 */
export async function initWasmCompute() {
  if (wasmModule) return wasmModule;
  if (initPromise) return initPromise;

  initPromise = (async () => {
    const { default: init, tokenize, tokenize_range, compute_diff, compute_layout, compute_visible_lines, count_lines, read_string_from_buffer, write_string_to_buffer } = await import('./wasm/monaco_wasm.js');
    await init();

    wasmModule = {
      tokenize,
      tokenize_range,
      compute_diff,
      compute_layout,
      compute_visible_lines,
      count_lines,
      read_string_from_buffer,
      write_string_to_buffer,
    };
    return wasmModule;
  })();

  return initPromise;
}

/**
 * Tokenize a source string.
 * @param {string} source
 * @param {string} language — 'rust', 'javascript', 'typescript', etc.
 * @returns {Promise<Array<{text, type, line, start, end}>>}
 */
export async function tokenize(source, language = 'rust') {
  const mod = await initWasmCompute();
  const json = mod.tokenize(source, language);
  return JSON.parse(json);
}

/**
 * Tokenize a range of lines [startLine, endLine).
 * @param {string} source
 * @param {string} language
 * @param {number} startLine — 0-indexed, inclusive
 * @param {number} endLine — 0-indexed, exclusive
 * @returns {Promise<Array<{text, type, line, start, end}>>}
 */
export async function tokenizeRange(source, language, startLine, endLine) {
  const mod = await initWasmCompute();
  const json = mod.tokenize_range(source, language, startLine, endLine);
  return JSON.parse(json);
}

/**
 * Compute a line-based diff between two texts.
 * @param {string} oldText
 * @param {string} newText
 * @returns {Promise<Array<{start_line, start_column, end_line, end_column, text}>>}
 */
export async function computeDiff(oldText, newText) {
  const mod = await initWasmCompute();
  const json = mod.compute_diff(oldText, newText);
  return JSON.parse(json);
}

/**
 * Compute line layout information.
 * @param {string} source
 * @param {number} lineWidth — max characters per line before wrapping
 * @returns {Promise<Array<{line, char_count, wrapped, wrap_count}>>}
 */
export async function computeLayout(source, lineWidth = 80) {
  const mod = await initWasmCompute();
  const json = mod.compute_layout(source, lineWidth);
  return JSON.parse(json);
}

/**
 * Compute viewport-visible lines.
 * @param {string} source
 * @param {number} lineHeightPx
 * @param {number} scrollTopPx
 * @param {number} viewportHeightPx
 * @returns {Promise<{start_line, end_line}>}
 */
export async function computeVisibleLines(source, lineHeightPx, scrollTopPx, viewportHeightPx) {
  const mod = await initWasmCompute();
  const json = mod.compute_visible_lines(source, lineHeightPx, scrollTopPx, viewportHeightPx);
  return JSON.parse(json);
}

/**
 * Fast line count (no full tokenization).
 * @param {string} source
 * @returns {Promise<number>}
 */
export async function countLines(source) {
  const mod = await initWasmCompute();
  return mod.count_lines(source);
}

/**
 * Read a UTF-8 string from a SharedArrayBuffer / Uint8Array.
 * @param {Uint8Array} buffer
 * @param {number} offset
 * @param {number} length
 * @returns {Promise<string>}
 */
export async function readStringFromBuffer(buffer, offset, length) {
  const mod = await initWasmCompute();
  return mod.read_string_from_buffer(buffer, offset, length);
}

/**
 * Write a UTF-8 string into a pre-allocated buffer.
 * @param {Uint8Array} buffer
 * @param {number} offset
 * @param {string} text
 * @returns {Promise<number>} bytes written
 */
export async function writeStringToBuffer(buffer, offset, text) {
  const mod = await initWasmCompute();
  return mod.write_string_to_buffer(buffer, offset, text);
}

/**
 * Convenience: batch tokenize multiple files.
 * @param {Array<{source, language}>} items
 * @returns {Promise<Array<Array<{text, type, line, start, end}>>>}
 */
export async function tokenizeBatch(items) {
  const results = [];
  for (const { source, language } of items) {
    results.push(await tokenize(source, language));
  }
  return results;
}
