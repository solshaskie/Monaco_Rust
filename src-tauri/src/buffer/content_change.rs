use crate::buffer::Position;

/// A content change operation, mirroring Monaco's IModelContentChange.
///
/// This represents a single edit operation that can be applied to a text buffer.
#[derive(Debug, Clone)]
pub struct ContentChange {
    /// Start position of the range to replace (1-indexed).
    pub start_position: Position,
    /// End position of the range to replace (1-indexed).
    pub end_position: Position,
    /// The text to insert (UTF-8 encoded).
    pub text: String,
    /// The offset in the document where this change occurs.
    pub range_offset: u64,
    /// The length of the text being replaced.
    pub range_length: u64,
}

impl ContentChange {
    /// Creates a new content change.
    pub fn new(
        start_position: Position,
        end_position: Position,
        text: String,
        range_offset: u64,
        range_length: u64,
    ) -> Self {
        Self {
            start_position,
            end_position,
            text,
            range_offset,
            range_length,
        }
    }

    /// Creates a content change for inserting text at a position.
    pub fn insert(position: Position, text: String, offset: u64) -> Self {
        Self {
            start_position: position,
            end_position: position,
            text,
            range_offset: offset,
            range_length: 0,
        }
    }

    /// Creates a content change for deleting a range.
    pub fn delete(
        start_position: Position,
        end_position: Position,
        range_offset: u64,
        range_length: u64,
    ) -> Self {
        Self {
            start_position,
            end_position,
            text: String::new(),
            range_offset,
            range_length,
        }
    }

    /// Creates a content change for replacing a range with new text.
    pub fn replace(
        start_position: Position,
        end_position: Position,
        text: String,
        range_offset: u64,
        range_length: u64,
    ) -> Self {
        Self {
            start_position,
            end_position,
            text,
            range_offset,
            range_length,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_change_new() {
        let change = ContentChange::new(
            Position::new(1, 1),
            Position::new(1, 5),
            "hello".to_string(),
            0,
            4,
        );

        assert_eq!(change.start_position.line, 1);
        assert_eq!(change.start_position.column, 1);
        assert_eq!(change.end_position.line, 1);
        assert_eq!(change.end_position.column, 5);
        assert_eq!(change.text, "hello");
        assert_eq!(change.range_offset, 0);
        assert_eq!(change.range_length, 4);
    }

    #[test]
    fn content_change_insert() {
        let change = ContentChange::insert(Position::new(2, 3), "test".to_string(), 10);

        assert_eq!(change.start_position.line, 2);
        assert_eq!(change.start_position.column, 3);
        assert_eq!(change.end_position, change.start_position);
        assert_eq!(change.text, "test");
        assert_eq!(change.range_offset, 10);
        assert_eq!(change.range_length, 0);
    }

    #[test]
    fn content_change_delete() {
        let change = ContentChange::delete(Position::new(1, 1), Position::new(1, 5), 0, 4);

        assert_eq!(change.text, "");
        assert_eq!(change.range_length, 4);
    }

    #[test]
    fn content_change_replace() {
        let change = ContentChange::replace(
            Position::new(1, 1),
            Position::new(1, 5),
            "new".to_string(),
            0,
            4,
        );

        assert_eq!(change.text, "new");
        assert_eq!(change.range_length, 4);
    }
}
