/// Line index information, mirroring Monaco's ILineStarts.
#[derive(Debug, Clone, Default)]
pub struct LineIndex {
    /// Byte offsets for the start of each line (0-indexed).
    /// line_starts[0] = 0 (start of document)
    /// line_starts[1] = byte offset of line 2, etc.
    pub line_starts: Vec<usize>,
    /// Total number of UTF-16 code units (for Monaco column compatibility).
    pub total_utf16_units: usize,
}

impl LineIndex {
    /// Creates a new line index from content.
    pub fn new(content: &str) -> Self {
        let mut line_starts = Vec::new();
        let mut total_utf16_units = 0;

        if content.is_empty() {
            line_starts.push(0);
            return Self {
                line_starts,
                total_utf16_units,
            };
        }

        line_starts.push(0); // First line starts at byte 0

        // Iterate through content tracking line starts
        let mut pos = 0;
        while pos < content.len() {
            // Find the end of the current line
            let line_end = content[pos..].find('\n').map(|p| pos + p).unwrap_or(content.len());

            // Count UTF-16 units for this line (excluding newline)
            total_utf16_units += content[pos..line_end].encode_utf16().count();

            // Move past the line and newline
            if line_end < content.len() {
                // There's a newline character
                pos = line_end + 1;
                if pos <= content.len() {
                    line_starts.push(pos);
                }
            } else {
                // Last line, no trailing newline
                break;
            }
        }

        Self {
            line_starts,
            total_utf16_units,
        }
    }

    /// Returns the number of lines.
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Gets the byte offset for the start of a given line (1-indexed).
    pub fn line_start_offset(&self, line: u32) -> Option<usize> {
        if line == 0 || line as usize > self.line_starts.len() {
            return None;
        }
        Some(self.line_starts[(line - 1) as usize])
    }

    /// Gets the byte offset for the end of a given line (1-indexed).
    pub fn line_end_offset(&self, line: u32) -> Option<usize> {
        if line == 0 || line as usize > self.line_starts.len() {
            return None;
        }
        let idx = (line - 1) as usize;
        if idx + 1 < self.line_starts.len() {
            // End is start of next line minus 1 (for the newline)
            Some(self.line_starts[idx + 1].saturating_sub(1))
        } else {
            // Last line - we need the actual content length
            None // Caller should handle this case
        }
    }

    /// Converts a position (line, column) to a byte offset.
    /// Column is in UTF-16 code units for Monaco compatibility.
    pub fn position_to_offset(&self, content: &str, position: super::Position) -> Option<usize> {
        let line_start = self.line_start_offset(position.line)?;
        let line_content = &content[line_start..];

        // Get the line content (up to next newline or end)
        let line_end = line_content.find('\n').unwrap_or(line_content.len());
        let line_str = &line_content[..line_end];

        // Convert UTF-16 column to UTF-8 byte offset within the line
        let mut utf16_count = 0;
        let mut byte_offset = 0;

        for ch in line_str.chars() {
            if utf16_count >= (position.column - 1) as usize {
                break;
            }
            utf16_count += ch.len_utf16();
            byte_offset += ch.len_utf8();
        }

        // If column is beyond line length, clamp to line end
        if utf16_count < (position.column - 1) as usize {
            byte_offset = line_str.len();
        }

        Some(line_start + byte_offset)
    }

    /// Converts a byte offset to a position (line, column).
    /// Column is returned in UTF-16 code units.
    pub fn offset_to_position(&self, content: &str, offset: usize) -> Option<super::Position> {
        if offset > content.len() {
            return None;
        }

        // Find which line this offset is on
        let mut line = 1;
        for i in 0..self.line_starts.len() - 1 {
            if offset >= self.line_starts[i] && offset < self.line_starts[i + 1] {
                line = (i + 1) as u32;
                break;
            }
        }

        // Handle offset at or beyond last line start
        if offset >= *self.line_starts.last()? {
            line = self.line_starts.len() as u32;
        }

        let line_start = self.line_start_offset(line)?;
        let bytes_into_line = offset - line_start;

        // Count UTF-16 code units up to bytes_into_line
        let line_content = &content[line_start..];
        let line_end = line_content.find('\n').unwrap_or(line_content.len());
        let line_str = &line_content[..line_end.min(bytes_into_line)];

        let column = line_str.encode_utf16().count() as u32 + 1;

        Some(super::Position::new(line, column))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_index_simple() {
        let content = "hello\nworld\ntest";
        let index = LineIndex::new(content);

        assert_eq!(index.line_count(), 3);
        assert_eq!(index.line_starts, vec![0, 6, 12]);
    }

    #[test]
    fn line_index_with_crlf() {
        let content = "hello\r\nworld\r\ntest";
        let index = LineIndex::new(content);

        assert_eq!(index.line_count(), 3);
        assert_eq!(index.line_starts, vec![0, 7, 14]);
    }

    #[test]
    fn line_index_empty() {
        let content = "";
        let index = LineIndex::new(content);

        assert_eq!(index.line_count(), 1);
        assert_eq!(index.line_starts, vec![0]);
    }

    #[test]
    fn line_index_trailing_newline() {
        let content = "hello\n";
        let index = LineIndex::new(content);

        assert_eq!(index.line_count(), 2);
        assert_eq!(index.line_starts, vec![0, 6]);
    }

    #[test]
    fn line_start_offset() {
        let content = "abc\ndef\nghi";
        let index = LineIndex::new(content);

        assert_eq!(index.line_start_offset(1), Some(0));
        assert_eq!(index.line_start_offset(2), Some(4));
        assert_eq!(index.line_start_offset(3), Some(8));
        assert_eq!(index.line_start_offset(4), None);
        assert_eq!(index.line_start_offset(0), None);
    }

    #[test]
    fn position_to_offset() {
        let content = "hello\nworld";
        let index = LineIndex::new(content);

        assert_eq!(index.position_to_offset(content, crate::buffer::Position::new(1, 1)), Some(0));
        assert_eq!(index.position_to_offset(content, crate::buffer::Position::new(1, 3)), Some(2));
        assert_eq!(index.position_to_offset(content, crate::buffer::Position::new(2, 1)), Some(6));
        assert_eq!(index.position_to_offset(content, crate::buffer::Position::new(2, 4)), Some(9));
    }

    #[test]
    fn offset_to_position() {
        let content = "hello\nworld";
        let index = LineIndex::new(content);

        assert_eq!(index.offset_to_position(content, 0), Some(crate::buffer::Position::new(1, 1)));
        assert_eq!(index.offset_to_position(content, 2), Some(crate::buffer::Position::new(1, 3)));
        assert_eq!(index.offset_to_position(content, 6), Some(crate::buffer::Position::new(2, 1)));
        assert_eq!(index.offset_to_position(content, 9), Some(crate::buffer::Position::new(2, 4)));
    }
}
