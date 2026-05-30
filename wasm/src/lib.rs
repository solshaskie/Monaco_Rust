use wasm_bindgen::prelude::*;

mod diff;
mod layout;
mod tokenize;

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
/// Each token: { "text": "...", "type": "...", "line": 0, "start": 0, "end": 0 }
#[wasm_bindgen]
pub fn tokenize(source: &str, language: &str) -> Result<String, JsValue> {
    let tokens = tokenize::tokenize_source(source, language);
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
    let tokens = tokenize::tokenize_source_range(source, language, start_line, end_line);
    serde_json::to_string(&tokens)
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

/// Compute viewport-visible lines given a scroll offset and viewport height.
/// Returns { "start_line": 0, "end_line": 0 }
#[wasm_bindgen]
pub fn compute_visible_lines(
    source: &str,
    line_height_px: f64,
    scroll_top_px: f64,
    viewport_height_px: f64,
) -> Result<String, JsValue> {
    let visible = layout::compute_visible_lines(source, line_height_px, scroll_top_px, viewport_height_px);
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
