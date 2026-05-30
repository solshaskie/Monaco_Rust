/// A position in the text buffer, mirroring Monaco's IPosition.
///
/// Positions are 1-indexed (line 1, column 1 is the start of the document).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Position {
    /// Line number (1-indexed)
    pub line: u32,
    /// Column number (1-indexed, in UTF-16 code units for Monaco compatibility)
    pub column: u32,
}

impl Position {
    /// Creates a new position.
    pub fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }

    /// Returns the start position (1, 1).
    pub fn start() -> Self {
        Self::new(1, 1)
    }

    /// Converts to 0-indexed line and UTF-8 byte offset.
    pub fn to_zero_indexed(&self) -> (usize, usize) {
        // Column is in UTF-16 code units; for ASCII this equals byte offset
        // For non-ASCII, we need to convert. For now, treat as byte offset
        // and let the caller handle UTF-16 vs UTF-8 conversion.
        ((self.line - 1) as usize, (self.column - 1) as usize)
    }

    /// Creates from 0-indexed line and column.
    pub fn from_zero_indexed(line: usize, column: usize) -> Self {
        Self::new(line as u32 + 1, column as u32 + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_creation() {
        let pos = Position::new(1, 1);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 1);
    }

    #[test]
    fn position_start() {
        let pos = Position::start();
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 1);
    }

    #[test]
    fn position_zero_indexed_conversion() {
        let pos = Position::new(3, 5);
        let (line, col) = pos.to_zero_indexed();
        assert_eq!(line, 2);
        assert_eq!(col, 4);
    }

    #[test]
    fn position_from_zero_indexed() {
        let pos = Position::from_zero_indexed(2, 4);
        assert_eq!(pos.line, 3);
        assert_eq!(pos.column, 5);
    }
}
