//! WASM Incremental Sync Protocol (W1.3)
//!
//! Bridges the native Rust `BufferRegistry` with the WASM compute modules
//! running in the Monaco webview. Instead of sending the full buffer content
//! on every change, this module computes and sends only the changed lines.

use crate::buffer::BufferRegistry;
use serde::{Deserialize, Serialize};

/// A delta update representing a range of changed lines.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LineDelta {
    /// 0-indexed start line of the changed range.
    pub start_line: usize,
    /// 0-indexed end line (exclusive) of the changed range.
    pub end_line: usize,
    /// The new text for the changed range (including newline separators).
    pub text: String,
    /// The buffer version ID at the time of this delta.
    pub version_id: u64,
}

/// A full snapshot sent when a buffer is first opened or when
/// the WASM module requests a resync.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BufferSnapshot {
    pub resource: String,
    pub content: String,
    pub version_id: u64,
    pub line_count: usize,
}

/// Compute a line delta between an old snapshot and the current buffer state.
/// Returns `None` if the buffer is not found.
pub fn compute_line_delta(
    registry: &BufferRegistry,
    resource: &str,
    old_content: &str,
    old_version: u64,
) -> Result<Option<LineDelta>, String> {
    let current = registry
        .get_buffer_content(resource)
        .ok_or_else(|| format!("Buffer not found: {}", resource))?;

    let current_version = registry
        .get_buffer_version(resource)
        .unwrap_or(0);

    if current == old_content && current_version == old_version {
        return Ok(None); // No change
    }

    let old_lines: Vec<&str> = old_content.lines().collect();
    let new_lines: Vec<&str> = current.lines().collect();

    // Find the first changed line
    let mut first_changed = 0usize;
    while first_changed < old_lines.len()
        && first_changed < new_lines.len()
        && old_lines[first_changed] == new_lines[first_changed]
    {
        first_changed += 1;
    }

    // Find the last changed line (from the end)
    let mut last_old = old_lines.len();
    let mut last_new = new_lines.len();
    while last_old > first_changed
        && last_new > first_changed
        && old_lines[last_old - 1] == new_lines[last_new - 1]
    {
        last_old -= 1;
        last_new -= 1;
    }

    let text = new_lines[first_changed..last_new].join("\n");

    Ok(Some(LineDelta {
        start_line: first_changed,
        end_line: last_old,
        text,
        version_id: current_version,
    }))
}

/// Build a full snapshot for a buffer.
pub fn build_snapshot(
    registry: &BufferRegistry,
    resource: &str,
) -> Result<Option<BufferSnapshot>, String> {
    let content = registry
        .get_buffer_content(resource)
        .ok_or_else(|| format!("Buffer not found: {}", resource))?;
    let version_id = registry
        .get_buffer_version(resource)
        .unwrap_or(0);

    Ok(Some(BufferSnapshot {
        resource: resource.to_string(),
        content: content.clone(),
        version_id,
        line_count: content.lines().count(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_registry_with(content: &str) -> BufferRegistry {
        let mut reg = BufferRegistry::new();
        reg.open_buffer("test://file.rs".to_string(), content);
        reg
    }

    #[test]
    fn delta_no_change() {
        let reg = make_registry_with("hello\nworld");
        let delta = compute_line_delta(&reg, "test://file.rs", "hello\nworld", 1).unwrap();
        assert!(delta.is_none());
    }

    #[test]
    fn delta_single_line_change() {
        let reg = make_registry_with("hello\nworld\nfoo");
        let delta = compute_line_delta(&reg, "test://file.rs", "hello\nworld\nbar", 0)
            .unwrap()
            .unwrap();
        assert_eq!(delta.start_line, 2);
        assert_eq!(delta.end_line, 3);
        assert_eq!(delta.text, "foo");
    }

    #[test]
    fn delta_insert_line() {
        let reg = make_registry_with("hello\nworld\nnew\nfoo");
        let delta = compute_line_delta(&reg, "test://file.rs", "hello\nworld\nfoo", 0)
            .unwrap()
            .unwrap();
        assert_eq!(delta.start_line, 2);
        assert_eq!(delta.end_line, 2);
        assert_eq!(delta.text, "new");
    }

    #[test]
    fn delta_delete_line() {
        let reg = make_registry_with("hello\nfoo");
        let delta = compute_line_delta(&reg, "test://file.rs", "hello\nworld\nfoo", 0)
            .unwrap()
            .unwrap();
        assert_eq!(delta.start_line, 1);
        assert_eq!(delta.end_line, 2);
        assert_eq!(delta.text, "");
    }

    #[test]
    fn snapshot_builds() {
        let reg = make_registry_with("a\nb\nc");
        let snap = build_snapshot(&reg, "test://file.rs").unwrap().unwrap();
        assert_eq!(snap.content, "a\nb\nc");
        assert_eq!(snap.line_count, 3);
        assert_eq!(snap.version_id, 1);
    }
}
