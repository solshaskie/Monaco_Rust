use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use crate::buffer::{ContentChange, ModelContentChangedEvent, TextBuffer};

/// A registry that manages multiple text buffers by their resource URI.
///
/// This provides a headless document management system where buffers can be
/// opened, edited, and closed independently of any UI view. Multiple views
/// can observe the same buffer, and changes are synchronized through events.
#[derive(Debug)]
pub struct BufferRegistry {
    /// Map of resource URI to text buffer.
    buffers: HashMap<String, Arc<RwLock<TextBuffer>>>,
}

impl BufferRegistry {
    /// Creates a new empty buffer registry.
    pub fn new() -> Self {
        Self {
            buffers: HashMap::new(),
        }
    }

    /// Returns true if a buffer with the given resource is open.
    pub fn has_buffer(&self, resource: &str) -> bool {
        self.buffers.contains_key(resource)
    }

    /// Gets a buffer by resource URI.
    pub fn get_buffer(&self, resource: &str) -> Option<Arc<RwLock<TextBuffer>>> {
        self.buffers.get(resource).cloned()
    }

    /// Opens a new buffer with the given content.
    /// If a buffer already exists for this resource, returns the existing one.
    pub fn open_buffer(&mut self, resource: String, content: &str) -> Arc<RwLock<TextBuffer>> {
        if let Some(existing) = self.buffers.get(&resource) {
            return existing.clone();
        }

        let buffer = Arc::new(RwLock::new(TextBuffer::new(resource.clone(), content)));
        self.buffers.insert(resource, buffer.clone());
        buffer
    }

    /// Opens a buffer from existing content bytes.
    pub fn open_buffer_from_bytes(
        &mut self,
        resource: String,
        content_bytes: &[u8],
    ) -> Result<Arc<RwLock<TextBuffer>>, String> {
        let content = String::from_utf8(content_bytes.to_vec())
            .map_err(|e| format!("Invalid UTF-8 content: {}", e))?;
        Ok(self.open_buffer(resource, &content))
    }

    /// Closes a buffer by resource URI.
    /// Returns true if the buffer was open and is now closed.
    pub fn close_buffer(&mut self, resource: &str) -> bool {
        self.buffers.remove(resource).is_some()
    }

    /// Closes a buffer and returns it.
    pub fn close_buffer_with_value(&mut self, resource: &str) -> Option<TextBuffer> {
        self.buffers
            .remove(resource)
            .and_then(|arc| Arc::try_unwrap(arc).ok())
            .and_then(|rwlock| rwlock.into_inner().ok())
    }

    /// Returns the number of open buffers.
    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }

    /// Returns a list of all open buffer resource URIs.
    pub fn open_resources(&self) -> Vec<String> {
        self.buffers.keys().cloned().collect()
    }

    /// Applies an edit to a buffer.
    pub fn apply_edit(
        &self,
        resource: &str,
        change: &ContentChange,
    ) -> Option<ModelContentChangedEvent> {
        let buffer = self.buffers.get(resource)?;
        let mut buffer = buffer.write().ok()?;
        buffer.apply_change(change)
    }

    /// Applies an edit with optimistic version checking.
    /// Returns `None` if the buffer version does not match `expected_version`.
    pub fn apply_edit_optimistic(
        &self,
        resource: &str,
        change: &ContentChange,
        expected_version: u64,
    ) -> Result<Option<ModelContentChangedEvent>, String> {
        let buffer = self.buffers
            .get(resource)
            .ok_or_else(|| format!("No buffer found for resource: {}", resource))?;
        let buffer = buffer.read().map_err(|e| e.to_string())?;
        let current_version = buffer.version_id();
        if current_version != expected_version {
            return Err(format!(
                "version conflict: expected {} but found {}",
                expected_version, current_version
            ));
        }
        drop(buffer);
        let buffer = self.buffers.get(resource).unwrap();
        let mut buffer = buffer.write().map_err(|e| e.to_string())?;
        Ok(buffer.apply_change(change))
    }

    /// Sets the content of a buffer.
    pub fn set_buffer_content(
        &self,
        resource: &str,
        content: &str,
    ) -> Option<ModelContentChangedEvent> {
        let buffer = self.buffers.get(resource)?;
        let mut buffer = buffer.write().ok()?;
        buffer.set_value(content)
    }

    /// Gets the current content of a buffer.
    pub fn get_buffer_content(&self, resource: &str) -> Option<String> {
        let buffer = self.buffers.get(resource)?;
        let buffer = buffer.read().ok()?;
        Some(buffer.get_value())
    }

    /// Gets a range of lines from a buffer [start_line, end_line) (0-indexed).
    pub fn get_buffer_content_range(
        &self,
        resource: &str,
        start_line: usize,
        end_line: usize,
    ) -> Option<String> {
        let buffer = self.buffers.get(resource)?;
        let buffer = buffer.read().ok()?;
        Some(buffer.get_value_in_line_range(start_line, end_line))
    }

    /// Gets the dirty line ranges from the most recent edit.
    pub fn get_buffer_dirty_lines(&self, resource: &str) -> Option<Vec<super::LineRange>> {
        let buffer = self.buffers.get(resource)?;
        let buffer = buffer.read().ok()?;
        Some(buffer.dirty_line_ranges().to_vec())
    }

    /// Clears dirty line tracking for a buffer.
    pub fn clear_buffer_dirty_lines(&self, resource: &str) -> Option<()> {
        let buffer = self.buffers.get(resource)?;
        let mut buffer = buffer.write().ok()?;
        buffer.clear_dirty_lines();
        Some(())
    }

    /// Gets the current content of a buffer as bytes.
    pub fn get_buffer_content_bytes(&self, resource: &str) -> Option<Vec<u8>> {
        let buffer = self.buffers.get(resource)?;
        let buffer = buffer.read().ok()?;
        Some(buffer.get_value_bytes())
    }

    /// Gets the version ID of a buffer.
    pub fn get_buffer_version(&self, resource: &str) -> Option<u64> {
        let buffer = self.buffers.get(resource)?;
        let buffer = buffer.read().ok()?;
        Some(buffer.version_id())
    }

    /// Checks if a buffer is dirty.
    pub fn is_buffer_dirty(&self, resource: &str) -> Option<bool> {
        let buffer = self.buffers.get(resource)?;
        let buffer = buffer.read().ok()?;
        Some(buffer.is_dirty())
    }

    /// Marks a buffer as saved.
    pub fn mark_buffer_saved(&self, resource: &str) -> Option<()> {
        let buffer = self.buffers.get(resource)?;
        let mut buffer = buffer.write().ok()?;
        buffer.mark_as_saved();
        Some(())
    }

    /// Undoes the last operation on a buffer.
    pub fn undo(&self, resource: &str) -> Option<ModelContentChangedEvent> {
        let buffer = self.buffers.get(resource)?;
        let mut buffer = buffer.write().ok()?;
        buffer.undo()
    }

    /// Redoes the last undone operation on a buffer.
    pub fn redo(&self, resource: &str) -> Option<ModelContentChangedEvent> {
        let buffer = self.buffers.get(resource)?;
        let mut buffer = buffer.write().ok()?;
        buffer.redo()
    }

    /// Checks if a buffer can undo.
    pub fn can_undo(&self, resource: &str) -> Option<bool> {
        let buffer = self.buffers.get(resource)?;
        let buffer = buffer.read().ok()?;
        Some(buffer.can_undo())
    }

    /// Checks if a buffer can redo.
    pub fn can_redo(&self, resource: &str) -> Option<bool> {
        let buffer = self.buffers.get(resource)?;
        let buffer = buffer.read().ok()?;
        Some(buffer.can_redo())
    }

    /// Gets a snapshot of a buffer.
    pub fn get_buffer_snapshot(
        &self,
        resource: &str,
    ) -> Option<crate::buffer::TextBufferSnapshot> {
        let buffer = self.buffers.get(resource)?;
        let buffer = buffer.read().ok()?;
        Some(buffer.get_snapshot())
    }

    /// Clears all buffers from the registry.
    pub fn clear(&mut self) {
        self.buffers.clear();
    }
}

impl Default for BufferRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::{ContentChange, Position};

    #[test]
    fn registry_new() {
        let registry = BufferRegistry::new();
        assert_eq!(registry.buffer_count(), 0);
    }

    #[test]
    fn registry_open_buffer() {
        let mut registry = BufferRegistry::new();
        let _buffer = registry.open_buffer("test://file.txt".to_string(), "hello");

        assert!(registry.has_buffer("test://file.txt"));
        assert_eq!(registry.buffer_count(), 1);

        let content = registry.get_buffer_content("test://file.txt");
        assert_eq!(content, Some("hello".to_string()));
    }

    #[test]
    fn registry_open_existing_buffer() {
        let mut registry = BufferRegistry::new();
        let _buffer1 = registry.open_buffer("test://file.txt".to_string(), "hello");
        let _buffer2 = registry.open_buffer("test://file.txt".to_string(), "world");

        // Should return the same buffer, not create a new one
        assert_eq!(registry.buffer_count(), 1);

        // Content should be from the first open
        let content = registry.get_buffer_content("test://file.txt");
        assert_eq!(content, Some("hello".to_string()));
    }

    #[test]
    fn registry_close_buffer() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://file.txt".to_string(), "hello");

        assert!(registry.has_buffer("test://file.txt"));

        let closed = registry.close_buffer("test://file.txt");
        assert!(closed);
        assert!(!registry.has_buffer("test://file.txt"));
        assert_eq!(registry.buffer_count(), 0);
    }

    #[test]
    fn registry_apply_edit() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://file.txt".to_string(), "hello world");

        let change = ContentChange::insert(Position::new(1, 7), "beautiful ".to_string(), 6);
        let event = registry.apply_edit("test://file.txt", &change);

        assert!(event.is_some());
        let content = registry.get_buffer_content("test://file.txt");
        assert_eq!(content, Some("hello beautiful world".to_string()));
    }

    #[test]
    fn registry_set_content() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://file.txt".to_string(), "old content");

        let event = registry.set_buffer_content("test://file.txt", "new content");

        assert!(event.is_some());
        let content = registry.get_buffer_content("test://file.txt");
        assert_eq!(content, Some("new content".to_string()));
    }

    #[test]
    fn registry_version_tracking() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://file.txt".to_string(), "initial");

        assert_eq!(registry.get_buffer_version("test://file.txt"), Some(1));

        let change = ContentChange::insert(Position::new(1, 8), "!".to_string(), 7);
        registry.apply_edit("test://file.txt", &change);

        assert_eq!(registry.get_buffer_version("test://file.txt"), Some(2));
    }

    #[test]
    fn registry_dirty_tracking() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://file.txt".to_string(), "content");

        assert_eq!(registry.is_buffer_dirty("test://file.txt"), Some(false));

        let change = ContentChange::insert(Position::new(1, 8), "!".to_string(), 7);
        registry.apply_edit("test://file.txt", &change);

        assert_eq!(registry.is_buffer_dirty("test://file.txt"), Some(true));

        registry.mark_buffer_saved("test://file.txt");
        assert_eq!(registry.is_buffer_dirty("test://file.txt"), Some(false));
    }

    #[test]
    fn registry_undo_redo() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://file.txt".to_string(), "hello world");

        // Insert "beautiful " after "hello " (position 7 is after the space)
        let change = ContentChange::insert(Position::new(1, 7), "beautiful ".to_string(), 6);
        registry.apply_edit("test://file.txt", &change);

        assert_eq!(registry.can_undo("test://file.txt"), Some(true));

        registry.undo("test://file.txt");
        let content = registry.get_buffer_content("test://file.txt");
        assert_eq!(content, Some("hello world".to_string()));

        assert_eq!(registry.can_redo("test://file.txt"), Some(true));

        registry.redo("test://file.txt");
        let content = registry.get_buffer_content("test://file.txt");
        assert_eq!(content, Some("hello beautiful world".to_string()));
    }

    #[test]
    fn registry_open_resources() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://a.txt".to_string(), "a");
        registry.open_buffer("test://b.txt".to_string(), "b");
        registry.open_buffer("test://c.txt".to_string(), "c");

        let resources = registry.open_resources();
        assert_eq!(resources.len(), 3);
        assert!(resources.contains(&"test://a.txt".to_string()));
        assert!(resources.contains(&"test://b.txt".to_string()));
        assert!(resources.contains(&"test://c.txt".to_string()));
    }

    #[test]
    fn registry_clear() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://a.txt".to_string(), "a");
        registry.open_buffer("test://b.txt".to_string(), "b");

        assert_eq!(registry.buffer_count(), 2);

        registry.clear();

        assert_eq!(registry.buffer_count(), 0);
        assert!(!registry.has_buffer("test://a.txt"));
        assert!(!registry.has_buffer("test://b.txt"));
    }

    #[test]
    fn registry_get_snapshot() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://file.txt".to_string(), "content");

        let change = ContentChange::insert(Position::new(1, 8), "!".to_string(), 7);
        registry.apply_edit("test://file.txt", &change);

        let snapshot = registry.get_buffer_snapshot("test://file.txt");
        assert!(snapshot.is_some());

        let snapshot = snapshot.unwrap();
        assert_eq!(snapshot.content_utf8, b"content!");
        assert_eq!(snapshot.version_id, 2);
        assert!(snapshot.is_dirty);
    }

    #[test]
    fn registry_get_content_range() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://file.txt".to_string(), "line1\nline2\nline3");

        let range = registry.get_buffer_content_range("test://file.txt", 0, 2);
        assert_eq!(range, Some("line1\nline2\n".to_string()));

        let range = registry.get_buffer_content_range("test://file.txt", 1, 3);
        assert_eq!(range, Some("line2\nline3".to_string()));
    }

    #[test]
    fn registry_dirty_lines_tracking() {
        use crate::buffer::LineRange;
        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://file.txt".to_string(), "a\nb\nc");

        let change = ContentChange::insert(Position::new(2, 2), "x".to_string(), 1);
        registry.apply_edit("test://file.txt", &change);

        let dirty = registry.get_buffer_dirty_lines("test://file.txt");
        assert!(dirty.is_some());
        assert_eq!(dirty.unwrap(), vec![LineRange { start: 1, end: 2 }]);
    }

    #[test]
    fn registry_clear_dirty_lines() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://file.txt".to_string(), "hello");

        let change = ContentChange::insert(Position::new(1, 6), "!".to_string(), 5);
        registry.apply_edit("test://file.txt", &change);

        assert!(registry.get_buffer_dirty_lines("test://file.txt").unwrap().len() > 0);
        registry.clear_buffer_dirty_lines("test://file.txt");
        assert!(registry.get_buffer_dirty_lines("test://file.txt").unwrap().is_empty());
    }
}
