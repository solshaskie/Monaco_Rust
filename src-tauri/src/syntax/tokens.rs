use tree_sitter::{Tree, TreeCursor};

/// A syntax token representing a span of text with a classified token type.
#[derive(Debug, Clone, PartialEq)]
pub struct SyntaxToken {
    pub token_type: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub text: String,
}

/// Maps a Tree-sitter node kind to a Monaco-compatible token type.
fn node_kind_to_token_type(kind: &str) -> Option<&'static str> {
    match kind {
        // Rust keywords
        "use" | "fn" | "struct" | "enum" | "impl" | "trait" | "type" | "where" | "let" | "mut"
        | "const" | "static" | "pub" | "crate" | "mod" | "if" | "else" | "match" | "for"
        | "while" | "loop" | "break" | "continue" | "return" | "async" | "await" | "move"
        | "ref" | "self" | "super" | "in" | "as" | "dyn" | "box" | "yield" | "try" | "macro" => {
            Some("keyword")
        }

        // JavaScript / TypeScript keywords
        "function" | "class" | "extends" | "import" | "export" | "from" | "var" | "switch"
        | "case" | "default" | "do" | "catch" | "finally" | "throw" | "new" | "this" | "typeof"
        | "instanceof" | "void" | "delete" | "debugger" | "with" | "get" | "set" | "of"
        | "interface" | "namespace" | "module" | "declare" | "abstract" | "implements"
        | "public" | "private" | "protected" | "readonly" | "override" => Some("keyword"),

        // Identifiers
        "identifier"
        | "type_identifier"
        | "field_identifier"
        | "property_identifier"
        | "shorthand_property_identifier"
        | "shorthand_property_identifier_pattern"
        | "statement_identifier" => Some("identifier"),

        // Literals
        "string_literal" | "raw_string_literal" | "char_literal" | "string" | "template_string" => {
            Some("string")
        }
        "integer_literal" | "float_literal" | "number" => Some("number"),
        "boolean_literal" | "true" | "false" => Some("keyword"),
        "lifetime" => Some("keyword"),
        "null" | "undefined" => Some("keyword"),
        "regex" => Some("string"),

        // Comments
        "line_comment" | "block_comment" | "comment" => Some("comment"),

        // Operators and punctuation
        "+" | "-" | "*" | "/" | "%" | "&" | "|" | "^" | "!" | "~" | "=" | "<" | ">" | "&&"
        | "||" | "==" | "!=" | "<=" | ">=" | "<<" | ">>" | "+=" | "-=" | "*=" | "/=" | "%="
        | "&=" | "|=" | "^=" | "<<=" | ">>=" | "=>" | "->" | ".." | "..=" | "::" | "??" | "?."
        | "**" | "++" | "--" | "===" | "!==" | "||=" | "&&=" | "??=" | "<<<" | ">>>" => {
            Some("operator")
        }

        // Delimiters
        "(" | ")" | "{" | "}" | "[" | "]" | ";" | "," | "." => Some("delimiter"),

        // String interpolation components
        "format_specifier" | "escape_sequence" | "escape" => Some("string"),

        // Attributes / macros
        "attribute" | "attribute_item" | "macro_invocation" | "macro_rule" => Some("macro"),

        // Type-related
        "primitive_type" | "predefined_type" => Some("type"),

        _ => None,
    }
}

/// Extracts syntax tokens from a parsed Tree-sitter tree.
pub fn tokenize_tree(tree: &Tree, source: &str) -> Vec<SyntaxToken> {
    let mut tokens = Vec::new();
    let mut cursor = tree.root_node().walk();
    traverse_tree(&mut cursor, source, &mut tokens);
    tokens
}

/// Extracts syntax tokens for a specific line range [start_line, end_line) (1-indexed).
/// Tokens that overlap the range are included.
pub fn tokenize_tree_range(
    tree: &Tree,
    source: &str,
    start_line: u32,
    end_line: u32,
) -> Vec<SyntaxToken> {
    let all_tokens = tokenize_tree(tree, source);
    all_tokens
        .into_iter()
        .filter(|t| t.end_line >= start_line && t.start_line < end_line)
        .collect()
}

fn traverse_tree(cursor: &mut TreeCursor, source: &str, tokens: &mut Vec<SyntaxToken>) {
    let node = cursor.node();

    // Skip error nodes but still traverse children
    if node.is_error() || node.is_missing() {
        if cursor.goto_first_child() {
            loop {
                traverse_tree(cursor, source, tokens);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
        return;
    }

    // If this node has a token type mapping and is a leaf (or meaningful token),
    // record it. For leaf nodes, always try to map.
    // For named nodes that contain children, we recurse.
    let has_children = cursor.goto_first_child();

    if !has_children {
        // Leaf node - try to classify
        if let Some(token_type) = node_kind_to_token_type(node.kind()) {
            let text = &source[node.byte_range()];
            tokens.push(SyntaxToken {
                token_type: token_type.to_string(),
                start_line: node.start_position().row as u32 + 1, // 1-indexed
                start_column: node.start_position().column as u32 + 1,
                end_line: node.end_position().row as u32 + 1,
                end_column: node.end_position().column as u32 + 1,
                text: text.to_string(),
            });
        }
    } else {
        // Has children - if this node itself maps to a token type, we could emit
        // a single token for the whole span. Otherwise recurse into children.
        if let Some(token_type) = node_kind_to_token_type(node.kind()) {
            // Only emit a token for the whole node if it's a string/comment literal
            // that might contain child escape sequences we don't want to split
            let text = &source[node.byte_range()];
            tokens.push(SyntaxToken {
                token_type: token_type.to_string(),
                start_line: node.start_position().row as u32 + 1,
                start_column: node.start_position().column as u32 + 1,
                end_line: node.end_position().row as u32 + 1,
                end_column: node.end_position().column as u32 + 1,
                text: text.to_string(),
            });
        } else {
            // Recurse into children
            loop {
                traverse_tree(cursor, source, tokens);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        cursor.goto_parent();
    }
}

/// A tokenizer that maintains parsed state for incremental updates.
pub struct Tokenizer {
    parser: super::parser::SyntaxParser,
    last_tree: Option<Tree>,
    last_source: String,
}

impl Tokenizer {
    pub fn for_rust() -> Result<Self, String> {
        Ok(Self {
            parser: super::parser::SyntaxParser::for_rust()?,
            last_tree: None,
            last_source: String::new(),
        })
    }

    pub fn tokenize(&mut self, source: &str) -> Result<Vec<SyntaxToken>, String> {
        // Full parse for now. Incremental parsing requires calling Tree::edit()
        // with precise byte-range changes, which we'll add as an optimization later.
        let parsed = self.parser.parse(source)?;
        let tokens = tokenize_tree(&parsed.tree, source);
        self.last_tree = Some(parsed.tree);
        self.last_source = source.to_string();
        Ok(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_simple_rust() {
        let source = "fn main() { let s = \"hello\"; }";
        let mut parser = super::super::parser::SyntaxParser::for_rust().unwrap();
        let parsed = parser.parse(source).unwrap();
        let tokens = tokenize_tree(&parsed.tree, source);

        assert!(!tokens.is_empty());

        // Should have keyword "fn"
        let fn_token = tokens.iter().find(|t| t.text == "fn");
        assert!(fn_token.is_some(), "Expected 'fn' token");
        assert_eq!(fn_token.unwrap().token_type, "keyword");

        // Should have identifier "main"
        let main_token = tokens.iter().find(|t| t.text == "main");
        assert!(main_token.is_some(), "Expected 'main' token");
        assert_eq!(main_token.unwrap().token_type, "identifier");

        // Should have string literal
        let string_token = tokens.iter().find(|t| t.text == "\"hello\"");
        assert!(string_token.is_some(), "Expected string literal token");
        assert_eq!(string_token.unwrap().token_type, "string");
    }

    #[test]
    fn tokenize_rust_with_comments() {
        let source = "// A comment\nfn foo() {}";
        let mut parser = super::super::parser::SyntaxParser::for_rust().unwrap();
        let parsed = parser.parse(source).unwrap();
        let tokens = tokenize_tree(&parsed.tree, source);

        let comment_token = tokens.iter().find(|t| t.text == "// A comment");
        assert!(comment_token.is_some(), "Expected comment token");
        assert_eq!(comment_token.unwrap().token_type, "comment");
    }

    #[test]
    fn tokenize_rust_numbers() {
        let source = "let x = 42;";
        let mut parser = super::super::parser::SyntaxParser::for_rust().unwrap();
        let parsed = parser.parse(source).unwrap();
        let tokens = tokenize_tree(&parsed.tree, source);

        let num_token = tokens.iter().find(|t| t.text == "42");
        assert!(num_token.is_some(), "Expected number token");
        assert_eq!(num_token.unwrap().token_type, "number");
    }

    #[test]
    fn incremental_tokenizer_works() {
        let mut tokenizer = Tokenizer::for_rust().unwrap();

        let tokens1 = tokenizer.tokenize("fn main() {}").unwrap();
        assert!(!tokens1.is_empty());

        let tokens2 = tokenizer.tokenize("fn main() { let x = 1; }").unwrap();
        assert!(!tokens2.is_empty());
        assert!(tokens2.iter().any(|t| t.text == "let"));
    }

    #[test]
    fn tokenize_tree_range_filters_by_lines() {
        let source = "fn main() {\n  let x = 1;\n  let y = 2;\n}";
        let mut parser = super::super::parser::SyntaxParser::for_rust().unwrap();
        let parsed = parser.parse(source).unwrap();

        // All tokens
        let all = tokenize_tree(&parsed.tree, source);
        assert!(all.iter().any(|t| t.text == "fn"));
        assert!(all.iter().any(|t| t.text == "x"));
        assert!(all.iter().any(|t| t.text == "y"));

        // Range line 1-2 (1-indexed): should include "fn" on line 1 but not "y" on line 3
        let range = tokenize_tree_range(&parsed.tree, source, 1, 3);
        assert!(range.iter().any(|t| t.text == "fn"));
        assert!(range.iter().any(|t| t.text == "x"));
        assert!(!range.iter().any(|t| t.text == "y"));
    }
}
