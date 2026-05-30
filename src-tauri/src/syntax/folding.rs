use tree_sitter::{Node, Tree};

/// A foldable code range.
#[derive(Debug, Clone, PartialEq)]
pub struct FoldingRange {
    pub start_line: u32,
    pub end_line: u32,
    pub kind: Option<String>,
}

/// Node kinds that represent foldable blocks in Rust.
const FOLDABLE_KINDS: &[&str] = &[
    "function_item",
    "struct_item",
    "enum_item",
    "impl_item",
    "trait_item",
    "mod_item",
    "match_expression",
    "if_expression",
    "while_expression",
    "for_expression",
    "loop_expression",
    "block",
    "use_declaration",
    "const_item",
    "static_item",
];

/// Extract foldable ranges from a parsed Tree-sitter tree.
pub fn extract_folding_ranges(tree: &Tree) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();
    let root = tree.root_node();
    traverse_for_folding(&root, &mut ranges);
    ranges.sort_by_key(|r| (r.start_line, r.end_line));
    ranges.dedup_by(|a, b| a.start_line == b.start_line && a.end_line == b.end_line);
    ranges
}

fn traverse_for_folding(node: &Node, ranges: &mut Vec<FoldingRange>) {
    if node.is_error() || node.is_missing() {
        return;
    }

    if FOLDABLE_KINDS.contains(&node.kind()) {
        let start_line = node.start_position().row as u32 + 1;
        let end_line = node.end_position().row as u32 + 1;
        if end_line > start_line {
            ranges.push(FoldingRange {
                start_line,
                end_line,
                kind: None,
            });
        }
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            traverse_for_folding(&cursor.node(), ranges);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::parser::SyntaxParser;

    #[test]
    fn fold_function_item() {
        let source = "fn main() {\n    println!(\"hello\");\n}";
        let mut parser = SyntaxParser::for_rust().unwrap();
        let parsed = parser.parse(source).unwrap();
        let ranges = extract_folding_ranges(&parsed.tree);

        assert!(!ranges.is_empty());
        let function_fold = ranges.iter().find(|r| r.start_line == 1 && r.end_line == 3);
        assert!(function_fold.is_some(), "Expected function fold range");
    }

    #[test]
    fn fold_struct_item() {
        let source = "struct Point {\n    x: i32,\n    y: i32,\n}";
        let mut parser = SyntaxParser::for_rust().unwrap();
        let parsed = parser.parse(source).unwrap();
        let ranges = extract_folding_ranges(&parsed.tree);

        let struct_fold = ranges.iter().find(|r| r.start_line == 1 && r.end_line == 4);
        assert!(struct_fold.is_some(), "Expected struct fold range");
    }

    #[test]
    fn fold_impl_block() {
        let source = "impl Point {\n    fn new() -> Self {\n        Self { x: 0, y: 0 }\n    }\n}";
        let mut parser = SyntaxParser::for_rust().unwrap();
        let parsed = parser.parse(source).unwrap();
        let ranges = extract_folding_ranges(&parsed.tree);

        // Should find both impl and inner function folds
        let impl_fold = ranges.iter().find(|r| r.start_line == 1);
        assert!(impl_fold.is_some(), "Expected impl fold range");
    }

    #[test]
    fn no_fold_single_line() {
        let source = "fn main() {}";
        let mut parser = SyntaxParser::for_rust().unwrap();
        let parsed = parser.parse(source).unwrap();
        let ranges = extract_folding_ranges(&parsed.tree);

        // Single-line blocks should not produce folds
        assert!(ranges.is_empty() || !ranges.iter().any(|r| r.start_line == 1));
    }
}
