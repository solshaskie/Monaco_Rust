use tree_sitter::{Node, Point, Tree};

/// Hover information for a symbol at a position.
#[derive(Debug, Clone, PartialEq)]
pub struct HoverInfo {
    pub contents: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// Finds the node at the given 1-indexed line/column and produces hover info.
pub fn hover_at_position(tree: &Tree, source: &str, line: u32, column: u32) -> Option<HoverInfo> {
    let point = Point::new((line.saturating_sub(1)) as usize, (column.saturating_sub(1)) as usize);
    let root = tree.root_node();
    let node = find_node_at_point(&root, point)?;

    let kind = node.kind();
    let text = &source[node.byte_range()];

    let contents = format!("**{}**\n\n`{}`", kind, text);

    Some(HoverInfo {
        contents,
        start_line: node.start_position().row as u32 + 1,
        start_column: node.start_position().column as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        end_column: node.end_position().column as u32 + 1,
    })
}

fn find_node_at_point<'tree>(node: &Node<'tree>, point: Point) -> Option<Node<'tree>> {
    if !node_range_contains_point(node, point) {
        return None;
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            if node_range_contains_point(&cursor.node(), point) {
                return find_node_at_point(&cursor.node(), point);
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    Some(*node)
}

fn node_range_contains_point(node: &Node, point: Point) -> bool {
    let start = node.start_position();
    let end = node.end_position();

    if point.row < start.row || point.row > end.row {
        return false;
    }
    if point.row == start.row && point.column < start.column {
        return false;
    }
    if point.row == end.row && point.column > end.column {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::parser::SyntaxParser;

    #[test]
    fn hover_on_function_name() {
        let source = "fn main() { let x = 1; }";
        let mut parser = SyntaxParser::for_rust().unwrap();
        let parsed = parser.parse(source).unwrap();

        let hover = hover_at_position(&parsed.tree, source, 1, 4).unwrap();
        assert!(hover.contents.contains("identifier"));
        assert!(hover.contents.contains("main"));
    }

    #[test]
    fn hover_on_keyword() {
        let source = "fn main() { let x = 1; }";
        let mut parser = SyntaxParser::for_rust().unwrap();
        let parsed = parser.parse(source).unwrap();

        let hover = hover_at_position(&parsed.tree, source, 1, 1).unwrap();
        assert!(hover.contents.contains("function_item") || hover.contents.contains("fn"));
    }

    #[test]
    fn hover_out_of_range() {
        let source = "fn main() {}";
        let mut parser = SyntaxParser::for_rust().unwrap();
        let parsed = parser.parse(source).unwrap();

        let hover = hover_at_position(&parsed.tree, source, 999, 1);
        assert!(hover.is_none());
    }
}
