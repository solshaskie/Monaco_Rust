use tree_sitter::{Node, Tree};

/// Severity levels matching the LSP diagnostic severity specification.
#[derive(Debug, Clone, PartialEq)]
pub enum DiagnosticSeverity {
    Error = 1,
    Warning = 2,
    Information = 3,
    Hint = 4,
}

impl DiagnosticSeverity {
    pub fn as_lsp_value(&self) -> i32 {
        match self {
            DiagnosticSeverity::Error => 1,
            DiagnosticSeverity::Warning => 2,
            DiagnosticSeverity::Information => 3,
            DiagnosticSeverity::Hint => 4,
        }
    }
}

/// A diagnostic extracted from Tree-sitter parse errors.
#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxDiagnostic {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub message: String,
    pub severity: DiagnosticSeverity,
    pub source: String,
}

/// Collects syntax error diagnostics from a parsed Tree-sitter tree.
pub fn collect_syntax_diagnostics(tree: &Tree, source: &str) -> Vec<SyntaxDiagnostic> {
    let mut diagnostics = Vec::new();
    let root = tree.root_node();
    traverse_for_errors(&root, &mut diagnostics, source);
    diagnostics
}

fn traverse_for_errors(node: &Node, diagnostics: &mut Vec<SyntaxDiagnostic>, source: &str) {
    if node.is_error() || node.is_missing() {
        diagnostics.push(SyntaxDiagnostic {
            start_line: node.start_position().row as u32 + 1,
            start_column: node.start_position().column as u32 + 1,
            end_line: node.end_position().row as u32 + 1,
            end_column: node.end_position().column as u32 + 1,
            message: if node.is_missing() {
                format!(
                    "Missing {} in {}",
                    node.kind(),
                    node.parent().map(|p| p.kind()).unwrap_or("program")
                )
            } else {
                format!("Unexpected '{}'", node.kind())
            },
            severity: DiagnosticSeverity::Error,
            source: source.to_string(),
        });
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            traverse_for_errors(&cursor.node(), diagnostics, source);
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
    fn no_errors_on_valid_rust() {
        let source = "fn main() { let x = 1; }";
        let mut parser = SyntaxParser::for_rust().unwrap();
        let parsed = parser.parse(source).unwrap();
        let diagnostics = collect_syntax_diagnostics(&parsed.tree, "tree-sitter-rust");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn detects_missing_brace() {
        let source = "fn main() { let x = 1; ";
        let mut parser = SyntaxParser::for_rust().unwrap();
        let parsed = parser.parse(source).unwrap();
        let diagnostics = collect_syntax_diagnostics(&parsed.tree, "tree-sitter-rust");
        assert!(!diagnostics.is_empty());
    }

    #[test]
    fn detects_syntax_error() {
        let source = "fn main() { @ }";
        let mut parser = SyntaxParser::for_rust().unwrap();
        let parsed = parser.parse(source).unwrap();
        let diagnostics = collect_syntax_diagnostics(&parsed.tree, "tree-sitter-rust");
        assert!(!diagnostics.is_empty());
        assert!(diagnostics.iter().any(|d| d.message.contains("Unexpected")));
    }

    #[test]
    fn diagnostics_include_source() {
        let source = "fn main() { @ }";
        let mut parser = SyntaxParser::for_javascript().unwrap();
        let parsed = parser.parse(source).unwrap();
        let diagnostics = collect_syntax_diagnostics(&parsed.tree, "tree-sitter-javascript");
        assert!(!diagnostics.is_empty());
        assert_eq!(diagnostics[0].source, "tree-sitter-javascript");
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
    }
}
