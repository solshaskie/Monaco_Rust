use serde::{Deserialize, Serialize};

/// Layout information for a single line.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LineLayout {
    pub line: usize,
    pub char_count: usize,
    pub wrapped: bool,
    pub wrap_count: usize,
}

/// Viewport-visible line range.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VisibleLines {
    pub start_line: usize,
    pub end_line: usize,
}

/// Compute layout for each line given a maximum characters-per-line width.
pub fn compute_line_layout(source: &str, line_width: usize) -> Vec<LineLayout> {
    source
        .lines()
        .enumerate()
        .map(|(i, line)| {
            let char_count = line.chars().count();
            let wrap_count = if line_width == 0 {
                0
            } else {
                (char_count.saturating_sub(1)) / line_width
            };
            LineLayout {
                line: i,
                char_count,
                wrapped: wrap_count > 0,
                wrap_count,
            }
        })
        .collect()
}

/// Compute which lines are visible given scroll position and viewport height.
pub fn compute_visible_lines(
    source: &str,
    line_height_px: f64,
    scroll_top_px: f64,
    viewport_height_px: f64,
) -> VisibleLines {
    if line_height_px <= 0.0 {
        return VisibleLines {
            start_line: 0,
            end_line: 0,
        };
    }

    let total_lines = source.lines().count();
    let start_line = (scroll_top_px / line_height_px).floor() as usize;
    let visible_count = (viewport_height_px / line_height_px).ceil() as usize;
    let end_line = (start_line + visible_count).min(total_lines);

    VisibleLines {
        start_line,
        end_line,
    }
}

/// Compute the byte offset → screen position mapping for a single line.
/// Returns `(column, line_in_wrap)` for a given byte offset within a line.
pub fn position_in_wrapped_line(line: &str, byte_offset: usize, line_width: usize) -> (usize, usize) {
    let col = line[..byte_offset.min(line.len())].chars().count();
    if line_width == 0 {
        return (col, 0);
    }
    let wrap_line = col / line_width;
    let column_in_wrap = col % line_width;
    (column_in_wrap, wrap_line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_computes_char_counts() {
        let layouts = compute_line_layout("hello\nworld\n", 80);
        assert_eq!(layouts.len(), 2);
        assert_eq!(layouts[0].char_count, 5);
        assert_eq!(layouts[1].char_count, 5);
        assert!(!layouts[0].wrapped);
    }

    #[test]
    fn layout_computes_wrapping() {
        let layouts = compute_line_layout("hello world foo bar", 5);
        assert!(layouts[0].wrapped);
        assert_eq!(layouts[0].wrap_count, 3); // 18 chars / 5 = 3 wraps
    }

    #[test]
    fn visible_lines_basic() {
        let source = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj";
        let visible = compute_visible_lines(source, 20.0, 0.0, 60.0);
        assert_eq!(visible.start_line, 0);
        assert_eq!(visible.end_line, 3); // 60/20 = 3 lines visible
    }

    #[test]
    fn visible_lines_scrolled() {
        let source = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj";
        let visible = compute_visible_lines(source, 20.0, 40.0, 60.0);
        assert_eq!(visible.start_line, 2); // scrolled 40px = 2 lines
        assert_eq!(visible.end_line, 5);
    }

    #[test]
    fn position_in_wrap() {
        let (col, wrap) = position_in_wrapped_line("hello world", 6, 5);
        assert_eq!(col, 1);
        assert_eq!(wrap, 1);
    }
}
