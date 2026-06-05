use wasm_bindgen::prelude::*;
use serde::Serialize;

mod cache;
mod diff;
mod layout;
mod tokenize;

pub mod incremental;
pub mod tree_sitter_tokenizer;

// ---------------------------------------------------------------------------
// B.7 — Tree-sitter parity flag
// ---------------------------------------------------------------------------
//
// The original `tokenize` module was a hand-rolled heuristic that did NOT match
// the native Rust backend's tree-sitter-based semantic tokens. This constant
// and the `b7_*` family of functions expose the tree-sitter backed tokenizer
// that the rest of the system now uses.
//
// We keep the legacy `tokenize` / `tokenize_range` symbols (now delegating to
// tree-sitter) so existing JS consumers do not need a code change. The legacy
// `tokenize_source` heuristic is still available behind `legacy_*` for
// debugging and is marked `#[doc(hidden)]` in `tokenize.rs`.
//
// Determinism: tree-sitter parses are deterministic for a given grammar and
// source. The same input bytes produce the same token stream every time.

/// Returns true if the WASM build is using the tree-sitter backed tokenizer
/// (B.7 parity with the native Rust backend).
#[wasm_bindgen]
pub fn b7_tree_sitter_parity() -> bool {
    true
}

/// Initialize the WASM module. Call once from JS before any compute functions.
#[wasm_bindgen(start)]
pub fn start() {
    // Set up panic hook for better debugging in the browser console.
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

// ---------------------------------------------------------------------------
// Tokenization API
// ---------------------------------------------------------------------------

/// Tokenize a source string and return a JSON array of token objects.
/// Each token: { "text": "...", "type": "...", "line": 0, "start": 0, "end": 0,
///               "start_column": 0, "end_column": 0 }
///
/// This now uses tree-sitter (B.7 parity) instead of the hand-rolled heuristic.
#[wasm_bindgen]
pub fn tokenize(source: &str, language: &str) -> Result<String, JsValue> {
    let tokens = tree_sitter_tokenizer::tokenize_source(source, language);
    serde_json::to_string(&tokens)
        .map_err(|e| JsValue::from_str(&format!("serialization error: {}", e)))
}

/// Tokenize a range of lines [start_line, end_line) (0-indexed).
/// Faster than full-file tokenization for viewport-aware highlighting.
#[wasm_bindgen]
pub fn tokenize_range(
    source: &str,
    language: &str,
    start_line: usize,
    end_line: usize,
) -> Result<String, JsValue> {
    let tokens = tree_sitter_tokenizer::tokenize_source_range(source, language, start_line, end_line);
    serde_json::to_string(&tokens)
        .map_err(|e| JsValue::from_str(&format!("serialization error: {}", e)))
}

// ---------------------------------------------------------------------------
// Incremental Tokenization API (B.7)
// ---------------------------------------------------------------------------

/// Initialise incremental parsing for a resource. Must be called before
/// `incremental_edit` or `incremental_tokenize`.
#[wasm_bindgen]
pub fn incremental_init(resource: &str, source: &str, language: &str) -> bool {
    incremental::init(resource, source, language)
}

/// Apply an edit to the cached parse tree for a resource.
/// `start_byte`/`old_end_byte`/`new_end_byte` are byte offsets in the old source.
/// Row/column values are 0-indexed.
#[wasm_bindgen]
pub fn incremental_edit(
    resource: &str,
    start_byte: usize,
    old_end_byte: usize,
    new_end_byte: usize,
    start_row: usize,
    start_col: usize,
    old_end_row: usize,
    old_end_col: usize,
    new_end_row: usize,
    new_end_col: usize,
    replacement_text: &str,
) -> bool {
    incremental::edit(
        resource,
        start_byte,
        old_end_byte,
        new_end_byte,
        start_row,
        start_col,
        old_end_row,
        old_end_col,
        new_end_row,
        new_end_col,
        replacement_text,
    )
}

/// Re-tokenize a resource after incremental edits. Returns a JSON array of
/// token objects.
#[wasm_bindgen]
pub fn incremental_tokenize(resource: &str) -> Result<String, JsValue> {
    let tokens = incremental::tokenize(resource);
    serde_json::to_string(&tokens)
        .map_err(|e| JsValue::from_str(&format!("serialization error: {}", e)))
}

/// Drop the incremental parse state for a resource.
#[wasm_bindgen]
pub fn incremental_invalidate(resource: &str) {
    incremental::invalidate(resource);
}

/// Drop all incremental parse states.
#[wasm_bindgen]
pub fn incremental_clear() {
    incremental::clear();
}

/// Return the number of resources with active incremental state.
#[wasm_bindgen]
pub fn incremental_state_count() -> usize {
    incremental::len()
}

// ---------------------------------------------------------------------------
// Monaco Semantic Token Type Mapping (B.7)
// ---------------------------------------------------------------------------

/// Return a JSON object mapping tree-sitter node kinds to Monaco/LSP
/// semantic token types.  This lets the frontend verify that the WASM
/// tokenizer uses the same token-type vocabulary as the native backend.
#[wasm_bindgen]
pub fn semantic_token_types() -> Result<String, JsValue> {
    let mapping = serde_json::json!({
        "comment": "comment",
        "line_comment": "comment",
        "block_comment": "comment",
        "string": "string",
        "string_literal": "string",
        "raw_string_literal": "string",
        "char_literal": "string",
        "number": "number",
        "integer_literal": "number",
        "float_literal": "number",
        "keyword": "keyword",
        "fn": "keyword",
        "let": "keyword",
        "if": "keyword",
        "else": "keyword",
        "struct": "keyword",
        "enum": "keyword",
        "trait": "keyword",
        "impl": "keyword",
        "type": "type",
        "type_identifier": "type",
        "function": "function",
        "function_item": "function",
        "call_expression": "function",
        "variable": "variable",
        "identifier": "variable",
        "property_identifier": "property",
        "field_identifier": "property",
        "macro": "macro",
        "macro_invocation": "macro",
        "attribute_item": "macro",
        "operator": "operator",
        "binary_expression": "operator",
        "assignment_expression": "operator",
    });
    serde_json::to_string(&mapping)
        .map_err(|e| JsValue::from_str(&format!("serialization error: {}", e)))
}

// ---------------------------------------------------------------------------
// Diff API
// ---------------------------------------------------------------------------

/// Compute a line-based diff between old_text and new_text.
/// Returns a JSON array of edit objects:
/// { "start_line": 0, "start_column": 0, "end_line": 0, "end_column": 0, "text": "..." }
#[wasm_bindgen]
pub fn compute_diff(old_text: &str, new_text: &str) -> Result<String, JsValue> {
    let edits = diff::line_diff(old_text, new_text);
    serde_json::to_string(&edits)
        .map_err(|e| JsValue::from_str(&format!("serialization error: {}", e)))
}

// ---------------------------------------------------------------------------
// Layout API
// ---------------------------------------------------------------------------

/// Compute line layout information for a source string.
/// Returns a JSON array of line info: { "line": 0, "char_count": 0, "wrapped": false }
/// `line_width` is the maximum characters per line before wrapping.
#[wasm_bindgen]
pub fn compute_layout(source: &str, line_width: usize) -> Result<String, JsValue> {
    let lines = layout::compute_line_layout(source, line_width);
    serde_json::to_string(&lines)
        .map_err(|e| JsValue::from_str(&format!("serialization error: {}", e)))
}

/// Compute viewport-visible lines given total line count, scroll offset and viewport height.
/// Returns { "start_line": 0, "end_line": 0 }
/// Callers should cache `total_lines` (e.g. from `count_lines`) to avoid
/// re-scanning the source on every scroll event.
#[wasm_bindgen]
pub fn compute_visible_lines(
    total_lines: usize,
    line_height_px: f64,
    scroll_top_px: f64,
    viewport_height_px: f64,
) -> Result<String, JsValue> {
    let visible = layout::compute_visible_lines(total_lines, line_height_px, scroll_top_px, viewport_height_px);
    serde_json::to_string(&visible)
        .map_err(|e| JsValue::from_str(&format!("serialization error: {}", e)))
}

// ---------------------------------------------------------------------------
// Zero-copy buffer helpers (Uint8Array / SharedArrayBuffer compatible)
// ---------------------------------------------------------------------------

/// Read a UTF-8 string from a JS Uint8Array (or SharedArrayBuffer-backed view)
/// at the given offset and length. This avoids JSON serialization for large buffers.
#[wasm_bindgen]
pub fn read_string_from_buffer(
    buffer: &js_sys::Object, // Uint8Array or SharedArrayBuffer-backed view
    offset: usize,
    length: usize,
) -> Result<String, JsValue> {
    let uint8_array = js_sys::Uint8Array::new(buffer);
    let available = uint8_array.length() as usize;
    if offset >= available {
        return Ok(String::new());
    }
    let end = (offset + length).min(available);
    let slice = uint8_array.slice(offset as u32, end as u32);
    let mut bytes = vec![0u8; slice.length() as usize];
    slice.copy_to(&mut bytes);
    String::from_utf8(bytes)
        .map_err(|e| JsValue::from_str(&format!("utf-8 decode error: {}", e)))
}

/// Write a UTF-8 string into a JS Uint8Array (or SharedArrayBuffer-backed view)
/// at the given offset. Returns the number of bytes written.
#[wasm_bindgen]
pub fn write_string_to_buffer(
    buffer: &js_sys::Object,
    offset: usize,
    text: &str,
) -> Result<u32, JsValue> {
    let bytes = text.as_bytes();
    let uint8_array = js_sys::Uint8Array::new(buffer);
    let available = (uint8_array.length() as usize).saturating_sub(offset);
    let length = bytes.len().min(available);
    if length == 0 {
        return Ok(0);
    }
    let slice = uint8_array.slice(offset as u32, (offset + length) as u32);
    slice.copy_from(&bytes[..length]);
    Ok(length as u32)
}

/// Return the total line count for a source string (faster than full tokenization).
#[wasm_bindgen]
pub fn count_lines(source: &str) -> usize {
    source.lines().count()
}

// ---------------------------------------------------------------------------
// Binary delta/snapshot parser
// ---------------------------------------------------------------------------

/// Result of parsing a binary delta/snapshot produced by Rust `wasm_sync`.
#[derive(Serialize)]
struct BinarySyncResult {
    mode: String,
    start_line: Option<usize>,
    end_line: Option<usize>,
    text: Option<String>,
    version_id: Option<u64>,
    content: Option<String>,
    line_count: Option<usize>,
}

/// Parse a compact binary delta or snapshot from Rust `wasm_sync`.
///
/// The binary format is:
/// - mode 0 (noop): `[0]`
/// - mode 1 (delta): `[1][version_id:8LE][start_line:4LE][end_line:4LE][text_len:4LE][text:utf8]`
/// - mode 2 (snapshot): `[2][version_id:8LE][line_count:4LE][text_len:4LE][text:utf8]`
///
/// Returns a JS object with `mode` and optional fields.
#[wasm_bindgen]
pub fn apply_binary_delta(buffer: &js_sys::Object) -> Result<JsValue, JsValue> {
    let uint8_array = js_sys::Uint8Array::new(buffer);
    let bytes = uint8_array.to_vec();

    if bytes.is_empty() {
        return serde_wasm_bindgen::to_value(&BinarySyncResult {
            mode: "noop".to_string(),
            start_line: None,
            end_line: None,
            text: None,
            version_id: None,
            content: None,
            line_count: None,
        })
        .map_err(|e| JsValue::from_str(&format!("serialization error: {}", e)));
    }

    let mode = bytes[0];
    match mode {
        0 => serde_wasm_bindgen::to_value(&BinarySyncResult {
            mode: "noop".to_string(),
            start_line: None,
            end_line: None,
            text: None,
            version_id: None,
            content: None,
            line_count: None,
        })
        .map_err(|e| JsValue::from_str(&format!("serialization error: {}", e))),

        1 => {
            // Delta
            if bytes.len() < 21 {
                return Err(JsValue::from_str("binary delta too short"));
            }
            let version_id = u64::from_le_bytes([
                bytes[1], bytes[2], bytes[3], bytes[4],
                bytes[5], bytes[6], bytes[7], bytes[8],
            ]);
            let start_line = u32::from_le_bytes([bytes[9], bytes[10], bytes[11], bytes[12]]) as usize;
            let end_line = u32::from_le_bytes([bytes[13], bytes[14], bytes[15], bytes[16]]) as usize;
            let text_len = u32::from_le_bytes([bytes[17], bytes[18], bytes[19], bytes[20]]) as usize;

            if bytes.len() < 21 + text_len {
                return Err(JsValue::from_str("binary delta text truncated"));
            }
            let text = String::from_utf8(bytes[21..21 + text_len].to_vec())
                .map_err(|e| JsValue::from_str(&format!("utf-8 decode error: {}", e)))?;

            serde_wasm_bindgen::to_value(&BinarySyncResult {
                mode: "delta".to_string(),
                start_line: Some(start_line),
                end_line: Some(end_line),
                text: Some(text),
                version_id: Some(version_id),
                content: None,
                line_count: None,
            })
            .map_err(|e| JsValue::from_str(&format!("serialization error: {}", e)))
        }

        2 => {
            // Snapshot
            if bytes.len() < 17 {
                return Err(JsValue::from_str("binary snapshot too short"));
            }
            let version_id = u64::from_le_bytes([
                bytes[1], bytes[2], bytes[3], bytes[4],
                bytes[5], bytes[6], bytes[7], bytes[8],
            ]);
            let line_count = u32::from_le_bytes([bytes[9], bytes[10], bytes[11], bytes[12]]) as usize;
            let text_len = u32::from_le_bytes([bytes[13], bytes[14], bytes[15], bytes[16]]) as usize;

            if bytes.len() < 17 + text_len {
                return Err(JsValue::from_str("binary snapshot text truncated"));
            }
            let text = String::from_utf8(bytes[17..17 + text_len].to_vec())
                .map_err(|e| JsValue::from_str(&format!("utf-8 decode error: {}", e)))?;

            serde_wasm_bindgen::to_value(&BinarySyncResult {
                mode: "snapshot".to_string(),
                start_line: None,
                end_line: None,
                text: None,
                version_id: Some(version_id),
                content: Some(text),
                line_count: Some(line_count),
            })
            .map_err(|e| JsValue::from_str(&format!("serialization error: {}", e)))
        }

        _ => Err(JsValue::from_str(&format!("unknown binary mode: {}", mode))),
    }
}

// ---------------------------------------------------------------------------
// Cached tokenization API
// ---------------------------------------------------------------------------

/// Prime the token cache for a resource. Tokenizes the full source and stores
/// it keyed by `resource`. Returns the number of tokens cached.
#[wasm_bindgen]
pub fn prime_token_cache(resource: &str, source: &str, language: &str) -> usize {
    cache::prime(resource, source, language)
}

/// Tokenize a source string, using the cache when the snapshot is unchanged.
/// Returns a JSON array of token objects.
#[wasm_bindgen]
pub fn tokenize_cached(source: &str, language: &str, resource: &str) -> Result<String, JsValue> {
    let tokens = cache::tokenize(resource, source, language);
    serde_json::to_string(&tokens)
        .map_err(|e| JsValue::from_str(&format!("serialization error: {}", e)))
}

/// Tokenize a range of lines, using the full-file cache when possible.
/// Returns a JSON array of token objects.
#[wasm_bindgen]
pub fn tokenize_range_cached(
    source: &str,
    language: &str,
    resource: &str,
    start_line: usize,
    end_line: usize,
) -> Result<String, JsValue> {
    let tokens = cache::tokenize_range(resource, source, language, start_line, end_line);
    serde_json::to_string(&tokens)
        .map_err(|e| JsValue::from_str(&format!("serialization error: {}", e)))
}

/// Remove a single resource from the token cache.
#[wasm_bindgen]
pub fn invalidate_token_cache(resource: &str) {
    cache::invalidate(resource);
}

/// Clear the entire token cache.
#[wasm_bindgen]
pub fn clear_token_cache() {
    cache::clear();
}

/// Return the number of cached resources.
#[wasm_bindgen]
pub fn token_cache_len() -> usize {
    cache::len()
}

/// Return approximate total cached bytes.
#[wasm_bindgen]
pub fn token_cache_total_bytes() -> usize {
    cache::total_bytes()
}

// ---------------------------------------------------------------------------
// Memory management
// ---------------------------------------------------------------------------

/// Pre-grow the WASM linear memory by `additional_pages` (64 KiB each).
///
/// Call this before loading large buffers to avoid reallocation stalls
/// during compute operations. Returns the previous page count.
///
/// # Example
/// - `preallocate_memory(256)` adds 16 MiB (256 × 64 KiB).
/// - The max allowed is governed by `--max-memory` at link time
///   (currently 128 MiB = 2048 pages; see `wasm/.cargo/config.toml`).
///
/// # Panics
/// Panics if the grow exceeds `--max-memory` or if the memory object is
/// unavailable (should never happen in practice).
#[wasm_bindgen]
pub fn preallocate_memory(additional_pages: u32) -> u32 {
    let mem = wasm_bindgen::memory()
        .dyn_into::<js_sys::WebAssembly::Memory>()
        .expect("wasm memory should be a WebAssembly.Memory");
    mem.grow(additional_pages)
}

/// Return the current WASM memory size in pages (64 KiB per page).
#[wasm_bindgen]
pub fn current_memory_pages() -> u32 {
    let mem = wasm_bindgen::memory()
        .dyn_into::<js_sys::WebAssembly::Memory>()
        .expect("wasm memory should be a WebAssembly.Memory");
    js_sys::ArrayBuffer::from(mem.buffer()).byte_length() as u32 / 65536
}

// ---------------------------------------------------------------------------
// Memory64 evaluation
// ---------------------------------------------------------------------------
//
// ## Status of the memory64 proposal for buffers >4 GB
//
// The WebAssembly memory64 proposal extends linear memory addresses from 32-bit
// to 64-bit, lifting the 4 GB hard limit (65536 pages × 64 KiB) to a theoretical
// 2^64 bytes.
//
// ### Current reality (as of mid-2026)
// - The proposal is **Stage 4+ / shipping** in V8, SpiderMonkey, and JavaScriptCore.
// - HOWEVER, `wasm-bindgen` does **not yet emit memory64-compatible bindings**.
// - `wasm32v8` (Chrome/V8's 64-bit wasm target) exists experimentally but is
//   not supported by `wasm-bindgen` or stable `wasm-pack` toolchains.
// - wasm-ld supports `--experimental-memory64`, but this is not wired into
//   Rust's stable `wasm32-unknown-unknown` target.
//
// ### Practical implication for Monaco_Rust
// - Buffers >4 GB cannot live in a single WASM linear memory today.
// - The correct strategy is **chunking / streaming**: pass the buffer in
//   1–2 MB slices via `Uint8Array` / `SharedArrayBuffer` views.
// - Our `read_string_from_buffer` / `write_string_to_buffer` helpers already
//   support this model (offset + length into an external JS buffer).
// - The Rust backend (native, not WASM) has no 4 GB restriction and remains
//   authoritative for huge files; WASM only ever sees viewport-sized slices.
//
// ### Recommendation
// - **Do not pursue memory64** until `wasm-bindgen` adds stable support.
// - Instead, rely on the hybrid architecture: native Rust backend holds the
//   full rope/buffer; WASM receives only the visible-line slice.
// - Re-evaluate in 2027 once the wasm-bindgen memory64 tracking issue
//   (rustwasm/wasm-bindgen#4112) lands.
//
// ---------------------------------------------------------------------------
// B.7 — Tree-sitter parity: how to verify
// ---------------------------------------------------------------------------
//
// Add an integration assertion at the test layer that compares the WASM
// tokenizer output for a fixed Rust fixture against the native
// `syntax::semantic_tokens` output. The expected token set is recorded in
// `wasm/tests/parity-fixtures/`. The CI step `cargo test -p monaco-wasm --test
// parity_native` (in `.github/workflows/build.yml`) runs both sides and
// diffs the result. If the two diverge, the test fails.
