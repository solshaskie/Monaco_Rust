//! B.7 — Tree-sitter backed WASM tokenizer.
//!
//! This module is the WASM-side parity counterpart of `src-tauri/src/syntax/`.
//! It uses the *same* `tree-sitter` and language crate versions as the native
//! Rust backend, so identical input produces identical token streams.
//!
//! ## Why not web-tree-sitter?
//!
//! The JS-only `web-tree-sitter` package loads grammar WASM blobs from the
//! network at runtime. That breaks:
//!   1. Offline operation (CI, air-gapped dev machines)
//!   2. Determinism (network timing affects parse result ordering)
//!   3. Bundle size control (we already ship our own `.wasm`)
//!
//! Compiling the Rust tree-sitter runtime into our existing WASM module
//! eliminates all three.
//!
//! ## Token output contract
//!
//! Tokens are emitted in **byte-offset** form against the source slice that
//! was parsed, then mapped to (line, start_column, end_column) by walking
//! newlines. This matches `LspSemanticToken` semantics used by the native
//! `semantic_tokens.rs` module.

use serde::{Deserialize, Serialize};
use tree_sitter::{Node, Parser, Tree};

/// A semantic token emitted by the WASM tokenizer.
///
/// The shape is intentionally identical to the JS-side
/// `monaco.languages.SemanticTokensProvider` token object so the frontend can
/// consume WASM and native output through the same code path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WasmToken {
    pub text: String,
    pub token_type: String,
    pub line: usize,
    pub start: usize,
    pub end: usize,
    /// 1-indexed start column (matches Monaco's column convention).
    pub start_column: usize,
    /// 1-indexed end column (matches Monaco's column convention).
    pub end_column: usize,
}

/// Map a tree-sitter node type to a Monaco semantic token type.
///
/// The mapping is aligned with the native `semantic_tokens.rs` classifier so
/// the same source produces the same colors in both renderers.
///
/// Semantic token types follow the LSP specification:
/// namespace, type, class, enum, interface, struct, typeParameter,
/// parameter, variable, property, enumMember, event, function, method,
/// macro, keyword, modifier, comment, string, number, regexp, operator.
pub fn classify_node_for_semantic_tokens(node: Node, _source: &[u8], language: &str) -> Option<&'static str> {
    classify_node(node, _source, language)
}

/// Native parity mapping — matches `node_kind_to_token_type` in
/// `src-tauri/src/syntax/tokens.rs` so the same source produces identical
/// token streams in both backends.
fn classify_node(node: Node, _source: &[u8], _language: &str) -> Option<&'static str> {
    let kind = node.kind();

    match kind {
        // Rust keywords
        "use" | "fn" | "struct" | "enum" | "impl" | "trait" | "type" | "where" | "let"
        | "mut" | "const" | "static" | "pub" | "crate" | "mod" | "if" | "else" | "match"
        | "for" | "while" | "loop" | "break" | "continue" | "return" | "async" | "await"
        | "move" | "ref" | "self" | "super" | "in" | "as" | "dyn" | "box" | "yield"
        | "try" | "macro" => Some("keyword"),

        // JavaScript / TypeScript keywords
        "function" | "class" | "extends" | "import" | "export" | "from" | "var" | "switch"
        | "case" | "default" | "do" | "catch" | "finally" | "throw" | "new" | "this"
        | "typeof" | "instanceof" | "void" | "delete" | "debugger" | "with" | "get" | "set"
        | "of" | "interface" | "namespace" | "module" | "declare" | "abstract"
        | "implements" | "public" | "private" | "protected" | "readonly" | "override" => {
            Some("keyword")
        }

        // Identifiers
        "identifier"
        | "type_identifier"
        | "field_identifier"
        | "property_identifier"
        | "shorthand_property_identifier"
        | "shorthand_property_identifier_pattern"
        | "statement_identifier" => Some("identifier"),

        // String literals
        "string_literal" | "raw_string_literal" | "char_literal" | "string"
        | "template_string" | "format_specifier" | "escape_sequence" | "escape" => Some("string"),
        "regex" => Some("string"),

        // Number literals
        "integer_literal" | "float_literal" | "number" => Some("number"),
        "boolean_literal" | "true" | "false" => Some("keyword"),
        "lifetime" => Some("keyword"),
        "null" | "undefined" => Some("keyword"),

        // Comments
        "line_comment" | "block_comment" | "comment" => Some("comment"),

        // Operators
        "+" | "-" | "*" | "/" | "%" | "&" | "|" | "^" | "!" | "~" | "=" | "<" | ">"
        | "&&" | "||" | "==" | "!=" | "<=" | ">=" | "<<" | ">>" | "+=" | "-=" | "*="
        | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" | "=>" | "->" | ".."
        | "..=" | "::" | "??" | "?." | "**" | "++" | "--" | "===" | "!==" | "||="
        | "&&=" | "??=" | "<<<" | ">>>" => Some("operator"),

        // Delimiters
        "(" | ")" | "{" | "}" | "[" | "]" | ";" | "," | "." => Some("delimiter"),

        // Attributes / macros
        "attribute" | "attribute_item" | "macro_invocation" | "macro_rule" => Some("macro"),

        // Type-related
        "primitive_type" | "predefined_type" => Some("type"),

        _ => None,
    }
}

/// Build a tree-sitter parser for the given language id.
fn build_parser(language: &str) -> Option<Parser> {
    let mut parser = Parser::new();
    let lang = match language {
        "rust" | "rs" => tree_sitter_rust::LANGUAGE,
        "javascript" | "js" | "jsx" => tree_sitter_javascript::LANGUAGE,
        "typescript" | "ts" | "tsx" | "typescriptreact" | "javascriptreact" => {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT
        }
        _ => return None,
    };
    parser.set_language(&lang.into()).ok()?;
    Some(parser)
}

/// Walk a tree-sitter tree in source order, emitting one token per classified
/// node. We walk preorder to match the native `semantic_tokens.rs` cursor
/// (which follows TreeCursor::walk_preorder).
fn walk_tree(tree: &Tree, source: &[u8], language: &str, out: &mut Vec<WasmToken>) {
    let root = tree.root_node();
    let mut cursor = root.walk();

    // Stack of (node, enter) so we can do iterative DFS.
    let mut stack: Vec<(tree_sitter::Node, bool)> = Vec::new();
    stack.push((root, true));

    while let Some((node, enter)) = stack.pop() {
        if enter {
            // Try to classify this node.
            if let Some(token_type) = classify_node(node, source, language) {
                let start_byte = node.start_byte();
                let end_byte = node.end_byte();
                if end_byte > start_byte && end_byte <= source.len() {
                    let text =
                        String::from_utf8_lossy(&source[start_byte..end_byte]).to_string();
                    let (line, start_column, end_column) =
                        map_byte_range_to_line_col(source, start_byte, end_byte);
                    if line > 0 {
                        out.push(WasmToken {
                            text,
                            token_type: token_type.to_string(),
                            line: line - 1, // 0-indexed for Monaco semantic tokens
                            start: start_byte,
                            end: end_byte,
                            start_column,
                            end_column,
                        });
                    }
                }
                // Native parity: mapped nodes prune their subtree.
                continue;
            }
            // Push close marker, then children in reverse so we visit in source order.
            stack.push((node, false));
            let mut children: Vec<_> = node.children(&mut cursor).collect();
            children.reverse();
            for child in children {
                stack.push((child, true));
            }
        } else {
            // Leaving node — no-op, but advances the cursor so siblings can be walked.
            let _ = cursor.goto_next_sibling();
        }
    }
}

/// Map a [start_byte, end_byte) range to (1-indexed line, 1-indexed start_col,
/// 1-indexed end_col) by walking newlines.
fn map_byte_range_to_line_col(
    source: &[u8],
    start_byte: usize,
    end_byte: usize,
) -> (usize, usize, usize) {
    let mut line = 1usize;
    let mut line_start = 0usize;
    let mut start_col = 1usize;
    let mut end_col;
    let mut found_start = false;
    let mut line_at_token = 1usize;
    let mut line_start_at_token = 0usize;

    let mut i = 0usize;
    while i < source.len() {
        let b = source[i];
        if b == b'\n' {
            if !found_start && i + 1 > start_byte {
                start_col = (start_byte - line_start) + 1;
                found_start = true;
                line_at_token = line;
                line_start_at_token = line_start;
            } else if !found_start && i + 1 == start_byte {
                start_col = 1;
                found_start = true;
                line_at_token = line;
                line_start_at_token = line_start;
            }
            if found_start && i < end_byte {
                end_col = (end_byte - line_start) + 1;
                return (line, start_col, end_col);
            }
            line += 1;
            line_start = i + 1;
        } else if !found_start && i == start_byte {
            start_col = (start_byte - line_start) + 1;
            found_start = true;
            line_at_token = line;
            line_start_at_token = line_start;
        }
        i += 1;
    }

    if !found_start {
        start_col = start_byte + 1;
    }
    end_col = end_byte.saturating_sub(line_start_at_token) + 1;
    (line_at_token, start_col, end_col)
}

/// Tokenize a full source string using tree-sitter. Falls back to an empty
/// token vector if the language is unsupported.
pub fn tokenize_source(source: &str, language: &str) -> Vec<WasmToken> {
    let mut parser = match build_parser(language) {
        Some(p) => p,
        None => return Vec::new(),
    };
    let bytes = source.as_bytes();
    let tree = match parser.parse(bytes, None) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut tokens = Vec::new();
    walk_tree(&tree, bytes, language, &mut tokens);
    tokens
}

/// Tokenize only the lines in `[start_line, end_line)` (0-indexed) by
/// adjusting the input slice to span only those lines. This matches the
/// native `tokenize_document_range` behavior in the cache.
pub fn tokenize_source_range(
    source: &str,
    language: &str,
    start_line: usize,
    end_line: usize,
) -> Vec<WasmToken> {
    let lines: Vec<&str> = source.lines().collect();
    if start_line >= lines.len() {
        return Vec::new();
    }
    let end = end_line.min(lines.len());
    let slice = lines[start_line..end].join("\n");
    let mut tokens = tokenize_source(&slice, language);
    // Re-base line numbers to the original source.
    for token in &mut tokens {
        token.line += start_line;
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_hello_world_produces_keyword_and_macro_tokens() {
        let src = r#"fn main() {
    println!("hello");
}"#;
        let tokens = tokenize_source(src, "rust");
        assert!(!tokens.is_empty(), "expected non-empty token stream");

        // Should classify `fn` keyword
        let fn_kw = tokens.iter().find(|t| t.text == "fn");
        assert_eq!(fn_kw.map(|t| t.token_type.as_str()), Some("keyword"));

        // Macro invocation (println!("hello")) is emitted as a single macro
        // token for native parity; the string literal inside is not separately
        // emitted because macro_invocation classifies and prunes children.
        let macro_tok = tokens
            .iter()
            .find(|t| t.token_type == "macro" && t.text.contains("println"));
        assert!(macro_tok.is_some(), "expected macro token for println! invocation");
    }

    #[test]
    fn rust_comments_classified_as_comment() {
        let src = "// hello\nfn main() {}\n";
        let tokens = tokenize_source(src, "rust");
        let comment = tokens.iter().find(|t| t.token_type == "comment");
        assert!(comment.is_some(), "expected at least one comment token");
    }

    #[test]
    fn rust_numbers_classified_as_number() {
        let src = "fn main() { let x = 42; let y = 3.14; }";
        let tokens = tokenize_source(src, "rust");
        let has_number = tokens.iter().any(|t| t.token_type == "number");
        assert!(has_number, "expected at least one number token");
    }

    #[test]
    fn javascript_function_classification() {
        let src = "function main() { return 1; }";
        let tokens = tokenize_source(src, "javascript");
        assert!(!tokens.is_empty());

        let has_kw = tokens.iter().any(|t| t.token_type == "keyword");
        let has_num = tokens.iter().any(|t| t.token_type == "number");
        assert!(has_kw, "expected keyword token");
        assert!(has_num, "expected number token");
    }

    #[test]
    fn typescript_imports_classified() {
        let src = "import { foo } from 'bar';\nconst x: number = 1;";
        let tokens = tokenize_source(src, "typescript");
        let has_kw = tokens.iter().any(|t| t.token_type == "keyword");
        let has_str = tokens.iter().any(|t| t.token_type == "string");
        assert!(has_kw);
        assert!(has_str);
    }

    #[test]
    fn range_tokenization_rebases_line_numbers() {
        let src = "line1\nline2\nfn main() {}\nline4";
        let tokens = tokenize_source_range(src, "rust", 2, 3);
        for t in &tokens {
            assert!(t.line >= 2 && t.line < 3, "line out of range: {}", t.line);
        }
    }

    #[test]
    fn unsupported_language_returns_empty() {
        let tokens = tokenize_source("hello", "klingon");
        assert!(tokens.is_empty());
    }

    #[test]
    fn parity_against_native_keyword_set() {
        // The native semantic_tokens classifier tags `fn`, `let`, `if`, `else`
        // as keywords. This guards against accidental drift.
        let src = "fn main() { if true { let x = 1; } else { let y = 2; } }";
        let tokens = tokenize_source(src, "rust");
        let keyword_texts: Vec<&str> = tokens
            .iter()
            .filter(|t| t.token_type == "keyword")
            .map(|t| t.text.as_str())
            .collect();
        assert!(keyword_texts.contains(&"fn"));
        assert!(keyword_texts.contains(&"if"));
        assert!(keyword_texts.contains(&"else"));
        assert!(keyword_texts.contains(&"let"));
    }

    #[test]
    fn column_offsets_are_one_indexed() {
        let src = "fn main() {}";
        let tokens = tokenize_source(src, "rust");
        let fn_tok = tokens.iter().find(|t| t.text == "fn").expect("fn token");
        assert_eq!(fn_tok.start_column, 1, "fn should start at column 1");
        assert!(fn_tok.end_column >= 3, "fn should end past column 2");
    }
}
