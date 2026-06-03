use crate::buffer::undo::{UndoEntry, UndoStack, UndoTransaction};
use crate::buffer::{ContentChange, LineIndex, Position};
use ropey::Rope;

/// Events emitted by the text buffer when content changes.
#[derive(Debug, Clone)]
pub struct ModelContentChangedEvent {
    /// The resource URI of the buffer.
    pub resource: String,
    /// The new version ID after the change.
    pub version_id: u64,
    /// Whether this was a flush (full content replacement).
    pub is_flush: bool,
    /// Whether this change is part of an undo operation.
    pub is_undoing: bool,
    /// Whether this change is part of a redo operation.
    pub is_redoing: bool,
    /// The individual changes that occurred.
    pub changes: Vec<ContentChange>,
}

impl ModelContentChangedEvent {
    /// Creates a new content changed event.
    pub fn new(resource: String, version_id: u64, changes: Vec<ContentChange>) -> Self {
        Self {
            resource,
            version_id,
            is_flush: false,
            is_undoing: false,
            is_redoing: false,
            changes,
        }
    }

    /// Marks this as a flush event.
    pub fn with_flush(mut self, is_flush: bool) -> Self {
        self.is_flush = is_flush;
        self
    }

    /// Marks this as an undo event.
    pub fn with_undoing(mut self, is_undoing: bool) -> Self {
        self.is_undoing = is_undoing;
        self
    }

    /// Marks this as a redo event.
    pub fn with_redoing(mut self, is_redoing: bool) -> Self {
        self.is_redoing = is_redoing;
        self
    }
}

/// A range of lines that were affected by an edit (0-indexed, inclusive start, exclusive end).
#[derive(Debug, Clone, PartialEq)]
pub struct LineRange {
    pub start: usize,
    pub end: usize,
}

/// A rope-based text buffer that mirrors Monaco's ITextModel.
///
/// This buffer stores text efficiently using a rope data structure,
/// supports incremental edits, undo/redo, and provides line indexing.
#[derive(Debug)]
pub struct TextBuffer {
    /// The rope storing the text content.
    rope: Rope,
    /// The resource URI for this buffer.
    resource: String,
    /// Current version ID (incremented on each change).
    version_id: u64,
    /// Alternative version ID for tracking dirty state.
    alternative_version_id: u64,
    /// Whether the buffer has unsaved changes.
    is_dirty: bool,
    /// End-of-line sequence ("\n" or "\r\n").
    eol: String,
    /// Line index for fast line/column operations.
    line_index: LineIndex,
    /// Undo/redo stack.
    undo_stack: UndoStack,
    /// Maximum undo stack size.
    max_undo_size: usize,
    /// Dirty line ranges affected by the most recent edit (0-indexed).
    dirty_line_ranges: Vec<LineRange>,
}

impl TextBuffer {
    /// Creates a new text buffer with the given content.
    pub fn new(resource: String, content: &str) -> Self {
        let rope = Rope::from_str(content);
        let eol = Self::detect_eol(content);
        let line_index = LineIndex::new(content);

        Self {
            rope,
            resource,
            version_id: 1,
            alternative_version_id: 1,
            is_dirty: false,
            eol,
            line_index,
            undo_stack: UndoStack::new(),
            max_undo_size: 0, // unlimited
            dirty_line_ranges: Vec::new(),
        }
    }

    /// Creates a new text buffer with custom undo stack size.
    pub fn with_max_undo(resource: String, content: &str, max_undo_size: usize) -> Self {
        let mut buffer = Self::new(resource, content);
        buffer.max_undo_size = max_undo_size;
        buffer.undo_stack = UndoStack::with_max_size(max_undo_size);
        buffer
    }

    /// Detects the end-of-line sequence used in the content.
    fn detect_eol(content: &str) -> String {
        if content.contains("\r\n") {
            "\r\n".to_string()
        } else {
            "\n".to_string()
        }
    }

    /// Returns the resource URI.
    pub fn resource(&self) -> &str {
        &self.resource
    }

    /// Returns the current version ID.
    pub fn version_id(&self) -> u64 {
        self.version_id
    }

    /// Returns the alternative version ID (for dirty tracking).
    pub fn alternative_version_id(&self) -> u64 {
        self.alternative_version_id
    }

    /// Returns whether the buffer is dirty (has unsaved changes).
    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    /// Returns the end-of-line sequence.
    pub fn eol(&self) -> &str {
        &self.eol
    }

    /// Returns the total number of lines.
    pub fn line_count(&self) -> usize {
        self.line_index.line_count()
    }

    /// Returns the total character count (in UTF-16 code units).
    pub fn char_count(&self) -> usize {
        self.rope.chars().count()
    }

    /// Returns the byte length of the content.
    pub fn byte_length(&self) -> usize {
        self.rope.len_bytes()
    }

    /// Returns the entire content as a string.
    pub fn get_value(&self) -> String {
        self.rope.to_string()
    }

    /// Returns the entire content as bytes (UTF-8).
    pub fn get_value_bytes(&self) -> Vec<u8> {
        self.rope.to_string().into_bytes()
    }

    /// Returns the text of a specific line (0-indexed).
    pub fn get_line(&self, line: usize) -> Option<String> {
        if line >= self.line_index.line_count() {
            return None;
        }

        let start = self.line_index.line_starts[line];
        let end = if line + 1 < self.line_index.line_starts.len() {
            // Exclude the newline character
            self.line_index.line_starts[line + 1].saturating_sub(1)
        } else {
            self.rope.len_bytes()
        };

        if start < end {
            Some(self.rope.get_slice(start..end)?.to_string())
        } else if start == self.rope.len_bytes() {
            Some(String::new()) // Empty last line
        } else {
            None
        }
    }

    /// Returns the content for a range of lines [start_line, end_line) (0-indexed).
    /// If end_line is beyond the buffer, returns up to the last line.
    pub fn get_value_in_line_range(&self, start_line: usize, end_line: usize) -> String {
        let line_count = self.line_index.line_count();
        if start_line >= line_count {
            return String::new();
        }
        let end_line = end_line.min(line_count);
        if start_line >= end_line {
            return String::new();
        }

        let start = self.line_index.line_starts[start_line];
        let end = if end_line < self.line_index.line_starts.len() {
            // Include the newline at the end of the last requested line
            self.line_index.line_starts[end_line]
        } else {
            self.rope.len_bytes()
        };

        self.rope
            .get_slice(start..end)
            .map(|s| s.to_string())
            .unwrap_or_default()
    }

    /// Returns the dirty line ranges from the most recent edit (0-indexed).
    pub fn dirty_line_ranges(&self) -> &[LineRange] {
        &self.dirty_line_ranges
    }

    /// Clears the dirty line ranges (call after processing).
    pub fn clear_dirty_lines(&mut self) {
        self.dirty_line_ranges.clear();
    }

    /// Returns the text at a specific position.
    pub fn get_value_in_range(&self, start: Position, end: Position) -> Option<String> {
        let start_offset = self.position_to_offset(start)?;
        let end_offset = self.position_to_offset(end)?;

        if start_offset <= end_offset && end_offset <= self.rope.len_bytes() {
            Some(self.rope.get_slice(start_offset..end_offset)?.to_string())
        } else {
            None
        }
    }

    /// Converts a position to a byte offset.
    pub fn position_to_offset(&self, position: Position) -> Option<usize> {
        self.line_index
            .position_to_offset(&self.rope.to_string(), position)
    }

    /// Converts a byte offset to a position.
    pub fn offset_to_position(&self, offset: usize) -> Option<Position> {
        self.line_index
            .offset_to_position(&self.rope.to_string(), offset)
    }

    /// Applies a single content change and returns the event.
    pub fn apply_change(&mut self, change: &ContentChange) -> Option<ModelContentChangedEvent> {
        let start_offset = self.position_to_offset(change.start_position)?;
        let end_offset = self.position_to_offset(change.end_position)?;

        // Get the text being replaced (for undo)
        let old_text = self.rope.get_slice(start_offset..end_offset)?.to_string();

        // Calculate the length of the replaced text in UTF-16 code units
        let range_length = old_text.encode_utf16().count() as u64;

        // Apply the change to the rope
        if !change.text.is_empty() || range_length > 0 {
            self.rope.remove(start_offset..end_offset);
            if !change.text.is_empty() {
                self.rope.insert(start_offset, &change.text);
            }
        }

        // Create undo entry
        let undo_entry = if old_text.is_empty() && !change.text.is_empty() {
            UndoEntry::insert(change.clone())
        } else if !old_text.is_empty() && change.text.is_empty() {
            UndoEntry::delete(change.clone(), old_text.clone())
        } else {
            UndoEntry::replace(change.clone(), old_text.clone())
        };

        // Create a transaction for this single change
        let mut transaction = UndoTransaction::new();
        transaction.push(undo_entry);
        self.undo_stack.push(transaction);

        // Update version IDs
        self.version_id += 1;
        self.alternative_version_id += 1;
        self.is_dirty = true;

        // Track dirty lines for incremental updates
        let start_line = change.start_position.line as usize - 1;
        let end_line = change.end_position.line as usize - 1;
        let inserted_lines = change.text.matches(&self.eol).count();
        let affected_end = start_line + inserted_lines + 1;
        self.dirty_line_ranges.push(LineRange {
            start: start_line,
            end: affected_end.max(end_line + 1),
        });

        // Rebuild line index
        let content = self.rope.to_string();
        self.line_index = LineIndex::new(&content);

        // Create and return the event
        let event = ModelContentChangedEvent::new(
            self.resource.clone(),
            self.version_id,
            vec![change.clone()],
        );

        Some(event)
    }

    /// Applies multiple content changes in a batch.
    pub fn apply_changes(&mut self, changes: &[ContentChange]) -> Option<ModelContentChangedEvent> {
        if changes.is_empty() {
            return None;
        }

        let mut transaction = UndoTransaction::new();
        let mut applied_changes = Vec::new();

        // We need to apply changes in reverse order of their position
        // to avoid offset shifts, or recalculate offsets after each change.
        // For simplicity, we'll apply them one by one and let the positions be relative
        // to the current state.

        for change in changes {
            let start_offset = match self.position_to_offset(change.start_position) {
                Some(offset) => offset,
                None => continue,
            };
            let end_offset = match self.position_to_offset(change.end_position) {
                Some(offset) => offset,
                None => continue,
            };

            if start_offset > end_offset || end_offset > self.rope.len_bytes() {
                continue;
            }

            // Get the text being replaced
            let old_text = self.rope.get_slice(start_offset..end_offset)?.to_string();

            // Apply the change
            if !change.text.is_empty() || !old_text.is_empty() {
                self.rope.remove(start_offset..end_offset);
                if !change.text.is_empty() {
                    self.rope.insert(start_offset, &change.text);
                }
            }

            // Create undo entry
            let undo_entry = if old_text.is_empty() && !change.text.is_empty() {
                UndoEntry::insert(change.clone())
            } else if !old_text.is_empty() && change.text.is_empty() {
                UndoEntry::delete(change.clone(), old_text.clone())
            } else {
                UndoEntry::replace(change.clone(), old_text.clone())
            };

            transaction.push(undo_entry);
            applied_changes.push(change.clone());
        }

        if applied_changes.is_empty() {
            return None;
        }

        self.undo_stack.push(transaction);

        // Update version IDs
        self.version_id += 1;
        self.alternative_version_id += 1;
        self.is_dirty = true;

        // Track dirty lines for incremental updates
        for change in &applied_changes {
            let start_line = change.start_position.line as usize - 1;
            let end_line = change.end_position.line as usize - 1;
            let inserted_lines = change.text.matches(&self.eol).count();
            let affected_end = start_line + inserted_lines + 1;
            self.dirty_line_ranges.push(LineRange {
                start: start_line,
                end: affected_end.max(end_line + 1),
            });
        }

        // Rebuild line index
        let content = self.rope.to_string();
        self.line_index = LineIndex::new(&content);

        let event =
            ModelContentChangedEvent::new(self.resource.clone(), self.version_id, applied_changes);

        Some(event)
    }

    /// Sets the content to a new value (flush operation).
    pub fn set_value(&mut self, new_content: &str) -> Option<ModelContentChangedEvent> {
        let old_content = self.rope.to_string();

        if old_content == new_content {
            return None;
        }

        // Create a change representing the full replacement
        let change = ContentChange {
            start_position: Position::new(1, 1),
            end_position: self
                .offset_to_position(old_content.len())
                .unwrap_or(Position::new(1, 1)),
            text: new_content.to_string(),
            range_offset: 0,
            range_length: old_content.encode_utf16().count() as u64,
        };

        let mut transaction = UndoTransaction::with_label("Set Value".to_string());
        transaction.push(UndoEntry::replace(change.clone(), old_content.clone()));
        self.undo_stack.push(transaction);

        // Replace content
        self.rope = Rope::from_str(new_content);
        self.eol = Self::detect_eol(new_content);

        // Update version IDs
        self.version_id += 1;
        self.alternative_version_id += 1;
        self.is_dirty = true;

        // Rebuild line index
        self.line_index = LineIndex::new(new_content);

        let event =
            ModelContentChangedEvent::new(self.resource.clone(), self.version_id, vec![change])
                .with_flush(true);

        Some(event)
    }

    /// Marks the buffer as saved (not dirty).
    pub fn mark_as_saved(&mut self) {
        self.is_dirty = false;
        self.alternative_version_id = self.version_id;
    }

    /// Attempts to undo the last operation.
    pub fn undo(&mut self) -> Option<ModelContentChangedEvent> {
        let transaction = self.undo_stack.undo()?;

        // Apply undo operations in reverse order
        let mut changes = Vec::new();
        for entry in transaction.entries.iter().rev() {
            if let Some(change) = self.apply_undo_entry(entry) {
                changes.push(change);
            }
        }

        if changes.is_empty() {
            return None;
        }

        self.version_id += 1;
        self.is_dirty = true;

        let event = ModelContentChangedEvent::new(self.resource.clone(), self.version_id, changes)
            .with_undoing(true);

        Some(event)
    }

    /// Attempts to redo the last undone operation.
    pub fn redo(&mut self) -> Option<ModelContentChangedEvent> {
        let transaction = self.undo_stack.redo()?;

        // Apply redo operations in forward order
        let mut changes = Vec::new();
        for entry in &transaction.entries {
            if let Some(change) = self.apply_redo_entry(entry) {
                changes.push(change);
            }
        }

        if changes.is_empty() {
            return None;
        }

        self.version_id += 1;
        self.is_dirty = true;

        let event = ModelContentChangedEvent::new(self.resource.clone(), self.version_id, changes)
            .with_redoing(true);

        Some(event)
    }

    /// Applies an undo entry (reverse of the original operation).
    fn apply_undo_entry(&mut self, entry: &UndoEntry) -> Option<ContentChange> {
        match entry {
            UndoEntry::Insert { position } => {
                // Undo insert = delete the inserted text
                let start_offset = self.position_to_offset(position.start_position)?;
                // Use byte length, not UTF-16 length
                let text_byte_len = position.text.len();
                let end_offset = start_offset + text_byte_len;

                let _deleted_text = self.rope.get_slice(start_offset..end_offset)?.to_string();
                self.rope.remove(start_offset..end_offset);

                // Rebuild line index
                let content = self.rope.to_string();
                self.line_index = LineIndex::new(&content);

                Some(ContentChange::delete(
                    position.start_position,
                    self.offset_to_position(start_offset)?,
                    start_offset as u64,
                    text_byte_len as u64,
                ))
            }
            UndoEntry::Delete {
                position,
                deleted_text,
            } => {
                // Undo delete = re-insert the deleted text
                let start_offset = self.position_to_offset(position.start_position)?;
                self.rope.insert(start_offset, deleted_text);

                // Rebuild line index
                let content = self.rope.to_string();
                self.line_index = LineIndex::new(&content);

                Some(ContentChange::insert(
                    position.start_position,
                    deleted_text.clone(),
                    start_offset as u64,
                ))
            }
            UndoEntry::Replace { position, old_text } => {
                // Undo replace = replace current text with old text
                let start_offset = self.position_to_offset(position.start_position)?;
                let current_text =
                    self.get_value_in_range(position.start_position, position.end_position)?;

                self.rope
                    .remove(start_offset..start_offset + current_text.len());
                self.rope.insert(start_offset, old_text);

                // Rebuild line index
                let content = self.rope.to_string();
                self.line_index = LineIndex::new(&content);

                Some(ContentChange::replace(
                    position.start_position,
                    self.offset_to_position(start_offset + old_text.len())?,
                    old_text.clone(),
                    start_offset as u64,
                    current_text.encode_utf16().count() as u64,
                ))
            }
        }
    }

    /// Applies a redo entry (re-apply the original operation).
    fn apply_redo_entry(&mut self, entry: &UndoEntry) -> Option<ContentChange> {
        match entry {
            UndoEntry::Insert { position } => {
                // Redo insert = re-insert the text
                let start_offset = self.position_to_offset(position.start_position)?;
                self.rope.insert(start_offset, &position.text);

                // Rebuild line index
                let content = self.rope.to_string();
                self.line_index = LineIndex::new(&content);

                Some(ContentChange::insert(
                    position.start_position,
                    position.text.clone(),
                    start_offset as u64,
                ))
            }
            UndoEntry::Delete {
                position,
                deleted_text: _,
            } => {
                // Redo delete = delete the text again
                let start_offset = self.position_to_offset(position.start_position)?;
                let end_offset = self.position_to_offset(position.end_position)?;
                let deleted_text = self.rope.get_slice(start_offset..end_offset)?.to_string();
                self.rope.remove(start_offset..end_offset);

                // Rebuild line index
                let content = self.rope.to_string();
                self.line_index = LineIndex::new(&content);

                Some(ContentChange::delete(
                    position.start_position,
                    position.end_position,
                    start_offset as u64,
                    deleted_text.encode_utf16().count() as u64,
                ))
            }
            UndoEntry::Replace { position, old_text } => {
                // Redo replace = replace old text with new text
                let start_offset = self.position_to_offset(position.start_position)?;
                let old_text_len = old_text.len();

                self.rope.remove(start_offset..start_offset + old_text_len);
                self.rope.insert(start_offset, &position.text);

                // Rebuild line index
                let content = self.rope.to_string();
                self.line_index = LineIndex::new(&content);

                Some(ContentChange::replace(
                    position.start_position,
                    self.offset_to_position(start_offset + position.text.len())?,
                    position.text.clone(),
                    start_offset as u64,
                    old_text.encode_utf16().count() as u64,
                ))
            }
        }
    }

    /// Returns whether undo is available.
    pub fn can_undo(&self) -> bool {
        self.undo_stack.can_undo()
    }

    /// Returns whether redo is available.
    pub fn can_redo(&self) -> bool {
        self.undo_stack.can_redo()
    }

    /// Returns the number of available undo operations.
    pub fn undo_count(&self) -> usize {
        self.undo_stack.undo_count()
    }

    /// Returns the number of available redo operations.
    pub fn redo_count(&self) -> usize {
        self.undo_stack.redo_count()
    }

    /// Gets a snapshot of the current buffer state.
    pub fn get_snapshot(&self) -> BufferSnapshot {
        BufferSnapshot {
            resource: self.resource.clone(),
            version_id: self.version_id,
            content_utf8: self.get_value_bytes(),
            eol: self.eol.clone(),
            is_dirty: self.is_dirty,
        }
    }
}

/// A snapshot of the buffer state at a point in time.
#[derive(Debug, Clone)]
pub struct BufferSnapshot {
    pub resource: String,
    pub version_id: u64,
    pub content_utf8: Vec<u8>,
    pub eol: String,
    pub is_dirty: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_buffer_creation() {
        let buffer = TextBuffer::new("test://file.txt".to_string(), "hello\nworld");
        assert_eq!(buffer.get_value(), "hello\nworld");
        assert_eq!(buffer.line_count(), 2);
        assert_eq!(buffer.version_id(), 1);
        assert!(!buffer.is_dirty());
    }

    #[test]
    fn text_buffer_insert() {
        let mut buffer = TextBuffer::new("test://file.txt".to_string(), "hello world");
        // Insert "beautiful " after "hello " (position 7 is after the space)
        let change = ContentChange::insert(Position::new(1, 7), "beautiful ".to_string(), 6);
        let event = buffer.apply_change(&change);

        assert!(event.is_some());
        assert_eq!(buffer.get_value(), "hello beautiful world");
        assert!(buffer.is_dirty());
        assert_eq!(buffer.version_id(), 2);
    }

    #[test]
    fn text_buffer_delete() {
        let mut buffer = TextBuffer::new("test://file.txt".to_string(), "hello beautiful world");
        let change = ContentChange::delete(Position::new(1, 6), Position::new(1, 16), 6, 10);
        let event = buffer.apply_change(&change);

        assert!(event.is_some());
        assert_eq!(buffer.get_value(), "hello world");
        assert!(buffer.is_dirty());
    }

    #[test]
    fn text_buffer_replace() {
        let mut buffer = TextBuffer::new("test://file.txt".to_string(), "hello world");
        let change = ContentChange::replace(
            Position::new(1, 7),
            Position::new(1, 12),
            "universe".to_string(),
            6,
            5,
        );
        let event = buffer.apply_change(&change);

        assert!(event.is_some());
        assert_eq!(buffer.get_value(), "hello universe");
    }

    #[test]
    fn text_buffer_set_value() {
        let mut buffer = TextBuffer::new("test://file.txt".to_string(), "old content");
        let event = buffer.set_value("new content");

        assert!(event.is_some());
        assert!(event.unwrap().is_flush);
        assert_eq!(buffer.get_value(), "new content");
    }

    #[test]
    fn text_buffer_undo_insert() {
        let mut buffer = TextBuffer::new("test://file.txt".to_string(), "hello world");
        // Insert "beautiful " after "hello " (position 7 is after the space)
        let change = ContentChange::insert(Position::new(1, 7), "beautiful ".to_string(), 6);
        buffer.apply_change(&change);

        assert_eq!(buffer.get_value(), "hello beautiful world");

        let undo_event = buffer.undo();
        assert!(undo_event.is_some());
        assert_eq!(buffer.get_value(), "hello world");
    }

    #[test]
    fn text_buffer_redo() {
        let mut buffer = TextBuffer::new("test://file.txt".to_string(), "hello world");
        // Insert "beautiful " after "hello " (position 7 is after the space)
        let change = ContentChange::insert(Position::new(1, 7), "beautiful ".to_string(), 6);
        buffer.apply_change(&change);
        buffer.undo();

        assert_eq!(buffer.get_value(), "hello world");

        let redo_event = buffer.redo();
        assert!(redo_event.is_some());
        assert_eq!(buffer.get_value(), "hello beautiful world");
    }

    #[test]
    fn text_buffer_mark_as_saved() {
        let mut buffer = TextBuffer::new("test://file.txt".to_string(), "content");
        let change = ContentChange::insert(Position::new(1, 8), "!".to_string(), 7);
        buffer.apply_change(&change);

        assert!(buffer.is_dirty());
        buffer.mark_as_saved();
        assert!(!buffer.is_dirty());
    }

    #[test]
    fn text_buffer_line_operations() {
        let buffer = TextBuffer::new("test://file.txt".to_string(), "line1\nline2\nline3");
        assert_eq!(buffer.line_count(), 3);
        assert_eq!(buffer.get_line(0), Some("line1".to_string()));
        assert_eq!(buffer.get_line(1), Some("line2".to_string()));
        assert_eq!(buffer.get_line(2), Some("line3".to_string()));
    }

    #[test]
    fn text_buffer_position_offset_conversion() {
        let buffer = TextBuffer::new("test://file.txt".to_string(), "hello\nworld");

        let offset = buffer.position_to_offset(Position::new(1, 1));
        assert_eq!(offset, Some(0));

        let offset = buffer.position_to_offset(Position::new(2, 3));
        assert_eq!(offset, Some(8));

        let pos = buffer.offset_to_position(0);
        assert_eq!(pos, Some(Position::new(1, 1)));

        let pos = buffer.offset_to_position(8);
        assert_eq!(pos, Some(Position::new(2, 3)));
    }

    #[test]
    fn text_buffer_snapshot() {
        let mut buffer = TextBuffer::new("test://file.txt".to_string(), "content");
        let change = ContentChange::insert(Position::new(1, 8), "!".to_string(), 7);
        buffer.apply_change(&change);

        let snapshot = buffer.get_snapshot();
        assert_eq!(snapshot.content_utf8, b"content!");
        assert_eq!(snapshot.version_id, 2);
        assert!(snapshot.is_dirty);
    }

    #[test]
    fn text_buffer_get_value_in_line_range() {
        let buffer = TextBuffer::new("test://file.txt".to_string(), "line1\nline2\nline3\nline4");
        assert_eq!(buffer.get_value_in_line_range(0, 2), "line1\nline2\n");
        assert_eq!(buffer.get_value_in_line_range(1, 3), "line2\nline3\n");
        assert_eq!(buffer.get_value_in_line_range(2, 4), "line3\nline4");
        assert_eq!(
            buffer.get_value_in_line_range(0, 10),
            "line1\nline2\nline3\nline4"
        );
        assert_eq!(buffer.get_value_in_line_range(4, 5), "");
    }

    #[test]
    fn text_buffer_dirty_line_tracking() {
        let mut buffer = TextBuffer::new("test://file.txt".to_string(), "line1\nline2\nline3");
        assert!(buffer.dirty_line_ranges().is_empty());

        // Insert on line 1 (0-indexed: line 0)
        let change = ContentChange::insert(Position::new(1, 6), "x".to_string(), 5);
        buffer.apply_change(&change);

        let dirty = buffer.dirty_line_ranges();
        assert_eq!(dirty.len(), 1);
        assert_eq!(dirty[0], LineRange { start: 0, end: 1 });
    }

    #[test]
    fn text_buffer_dirty_lines_multiline_insert() {
        let mut buffer = TextBuffer::new("test://file.txt".to_string(), "a\nb\nc");
        // Insert multi-line text at line 1
        let change = ContentChange::insert(Position::new(2, 2), "x\ny".to_string(), 2);
        buffer.apply_change(&change);

        let dirty = buffer.dirty_line_ranges();
        assert_eq!(dirty.len(), 1);
        assert_eq!(dirty[0], LineRange { start: 1, end: 3 });
    }

    #[test]
    fn text_buffer_clear_dirty_lines() {
        let mut buffer = TextBuffer::new("test://file.txt".to_string(), "hello");
        let change = ContentChange::insert(Position::new(1, 6), "!".to_string(), 5);
        buffer.apply_change(&change);
        assert!(!buffer.dirty_line_ranges().is_empty());

        buffer.clear_dirty_lines();
        assert!(buffer.dirty_line_ranges().is_empty());
    }
}
