use monaco_tauri::buffer::{ContentChange, LineIndex, Position, TextBuffer};
use proptest::prelude::*;

/// Property: LineIndex::offset_to_line and line_start_offset are consistent.
/// For every line in the index, the line's start offset maps back to that line.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn line_index_round_trip_consistency(content in "[\x00-\x7F]{0,1000}") {
        let index = LineIndex::new(&content);

        for line in 1..=index.line_count() {
            let start = index.line_start_offset(line as u32);
            prop_assert!(start.is_some(), "line {} should have a start offset", line);
            let offset = start.unwrap();
            let back = index.offset_to_line(offset);
            prop_assert_eq!(back, Some(line as u32),
                "offset {} should map back to line {}", offset, line);
        }
    }

    #[test]
    fn line_index_line_count_matches_newlines(content in "[\x00-\x7F]{0,1000}") {
        let index = LineIndex::new(&content);
        let expected = if content.is_empty() {
            1
        } else {
            content.matches('\n').count() + 1
        };
        prop_assert_eq!(index.line_count(), expected);
    }

    #[test]
    fn text_buffer_apply_change_then_line_index_consistent(
        initial in "[\x00-\x7F]{1,500}",
        insert_text in "[\x00-\x7F]{0,100}",
    ) {
        let mut buffer = TextBuffer::new("prop://test.rs".to_string(), &initial);

        // Apply an insert at a valid position (line 1, column 1)
        let change = ContentChange::insert(
            Position::new(1, 1),
            insert_text,
            0,
        );
        let _ = buffer.apply_change(&change);

        let content = buffer.get_value();
        let index = LineIndex::new(&content);

        // Round-trip: every line start offset maps back to its line
        for line in 1..=index.line_count() {
            let start = index.line_start_offset(line as u32).unwrap();
            prop_assert_eq!(index.offset_to_line(start), Some(line as u32));
        }

        // Buffer line count should match LineIndex line count
        prop_assert_eq!(buffer.line_count(), index.line_count());
    }

    #[test]
    fn text_buffer_undo_redo_preserves_line_index(
        initial in "[\x00-\x7F]{1,500}",
        insert_text in "[\x00-\x7F]{0,100}",
    ) {
        let mut buffer = TextBuffer::new("prop://test.rs".to_string(), &initial);

        let change = ContentChange::insert(
            Position::new(1, 1),
            insert_text,
            0,
        );
        let _ = buffer.apply_change(&change);

        let after_edit = buffer.get_value();
        let index_after = LineIndex::new(&after_edit);

        // Undo
        let _ = buffer.undo();
        let after_undo = buffer.get_value();
        let index_undo = LineIndex::new(&after_undo);

        prop_assert_eq!(after_undo, initial.clone());
        prop_assert_eq!(index_undo.line_count(), LineIndex::new(&initial).line_count());

        // Redo
        let _ = buffer.redo();
        let after_redo = buffer.get_value();
        let index_redo = LineIndex::new(&after_redo);

        prop_assert_eq!(after_redo, after_edit);
        prop_assert_eq!(index_redo.line_count(), index_after.line_count());
    }
}
