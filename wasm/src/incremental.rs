//! B.7 — Incremental tree-sitter tokenization.
//!
//! Maintains a per-resource `Parser` + `Tree` so that edits do not require a
//! full re-parse.  After the JS side applies an edit it calls
//! `incremental_edit` with the byte/row-column bounds; the old tree is
//! mutated via `Tree::edit` and the parser is run again with the updated
//! source and the old tree as a base.
//!
//! This is the WASM-side counterpart of the native Rust incremental
//! `SyntaxParser` cache in `src-tauri/src/syntax_handlers.rs`.

use std::cell::RefCell;
use std::collections::HashMap;

use tree_sitter::{InputEdit, Parser, Tree};

use crate::tree_sitter_tokenizer::{classify_node_for_semantic_tokens, WasmToken};

/// Per-resource incremental parse state.
struct IncrementalState {
    parser: Parser,
    tree: Option<Tree>,
    language: String,
    source: String,
}

thread_local! {
    static INCREMENTAL_STATES: RefCell<HashMap<String, IncrementalState>> = RefCell::new(HashMap::new());
}

/// Initialise incremental parsing for a resource.
/// Returns `true` on success, `false` if the language is unsupported.
pub fn init(resource: &str, source: &str, language: &str) -> bool {
    let mut parser = match build_parser(language) {
        Some(p) => p,
        None => return false,
    };

    let tree = parser.parse(source, None);

    INCREMENTAL_STATES.with(|states| {
        states.borrow_mut().insert(
            resource.to_string(),
            IncrementalState {
                parser,
                tree,
                language: language.to_string(),
                source: source.to_string(),
            },
        );
    });
    true
}

/// Apply an edit to the cached tree for a resource.
///
/// `start_byte`, `old_end_byte`, `new_end_byte` are byte offsets into the
/// **previous** source text.  Row/column values are 0-indexed.
pub fn edit(
    resource: &str,
    start_byte: usize,
    old_end_byte: usize,
    new_end_byte: usize,
    start_row: usize,
    start_col: usize,
    old_end_row: usize,
    old_end_col: usize,
    new_end_row: usize,
    new_end_col: usize,
    replacement_text: &str,
) -> bool {
    INCREMENTAL_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let state = match states.get_mut(resource) {
            Some(s) => s,
            None => return false,
        };

        // Update the stored source text.
        let mut new_source = String::with_capacity(
            state.source.len() - (old_end_byte - start_byte) + replacement_text.len(),
        );
        new_source.push_str(&state.source[..start_byte]);
        new_source.push_str(replacement_text);
        new_source.push_str(&state.source[old_end_byte..]);
        state.source = new_source;

        // Apply the edit to the old tree so the parser can reuse unchanged
        // regions.
        if let Some(ref mut tree) = state.tree {
            tree.edit(&InputEdit {
                start_byte,
                old_end_byte,
                new_end_byte,
                start_position: tree_sitter::Point::new(start_row, start_col),
                old_end_position: tree_sitter::Point::new(old_end_row, old_end_col),
                new_end_position: tree_sitter::Point::new(new_end_row, new_end_col),
            });
        }
        true
    })
}

/// Re-tokenize the current source for a resource, re-using the edited tree.
/// Falls back to a fresh parse if no incremental state exists.
pub fn tokenize(resource: &str) -> Vec<WasmToken> {
    INCREMENTAL_STATES.with(|states| {
        let mut states = states.borrow_mut();
        let state = match states.get_mut(resource) {
            Some(s) => s,
            None => return Vec::new(),
        };

        let bytes = state.source.as_bytes();
        let new_tree = state.parser.parse(bytes, state.tree.as_ref());

        let mut tokens = Vec::new();
        if let Some(ref tree) = new_tree {
            walk_tree(tree, bytes, &state.language, &mut tokens);
        }
        state.tree = new_tree;
        tokens
    })
}

/// Drop the incremental state for a resource (e.g. on buffer close).
pub fn invalidate(resource: &str) {
    INCREMENTAL_STATES.with(|states| {
        states.borrow_mut().remove(resource);
    });
}

/// Drop all incremental states.
pub fn clear() {
    INCREMENTAL_STATES.with(|states| {
        states.borrow_mut().clear();
    });
}

/// Number of resources with active incremental state.
pub fn len() -> usize {
    INCREMENTAL_STATES.with(|states| states.borrow().len())
}

// ---------------------------------------------------------------------------
// Helpers (duplicated from tree_sitter_tokenizer.rs to keep module self-contained)
// ---------------------------------------------------------------------------

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

fn walk_tree(tree: &Tree, source: &[u8], language: &str, out: &mut Vec<WasmToken>) {
    let root = tree.root_node();
    let mut cursor = root.walk();
    let mut stack: Vec<(tree_sitter::Node, bool)> = Vec::new();
    stack.push((root, true));

    while let Some((node, enter)) = stack.pop() {
        if enter {
            if let Some(token_type) = classify_node_for_semantic_tokens(node, source, language) {
                let start_byte = node.start_byte();
                let end_byte = node.end_byte();
                if end_byte > start_byte && end_byte <= source.len() {
                    let text = String::from_utf8_lossy(&source[start_byte..end_byte]).to_string();
                    let (line, start_column, end_column) =
                        map_byte_range_to_line_col(source, start_byte, end_byte);
                    if line > 0 {
                        out.push(WasmToken {
                            text,
                            token_type: token_type.to_string(),
                            line: line - 1,
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
            stack.push((node, false));
            let mut children: Vec<_> = node.children(&mut cursor).collect();
            children.reverse();
            for child in children {
                stack.push((child, true));
            }
        } else {
            let _ = cursor.goto_next_sibling();
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incremental_init_and_tokenize() {
        clear();
        let ok = init("test.rs", "fn main() {}", "rust");
        assert!(ok);
        let tokens = tokenize("test.rs");
        assert!(!tokens.is_empty());
        assert!(tokens.iter().any(|t| t.text == "fn" && t.token_type == "keyword"));
    }

    #[test]
    fn incremental_edit_updates_tokens() {
        clear();
        init("test.rs", "fn main() {}", "rust");
        let before = tokenize("test.rs");

        // Replace "main" with "foo" at byte offset 3..7
        let replaced = edit("test.rs", 3, 7, 7, 0, 3, 0, 7, 0, 7, "foo");
        assert!(replaced);

        let after = tokenize("test.rs");
        assert!(after.iter().any(|t| t.text == "foo"));
        assert!(!after.iter().any(|t| t.text == "main"));
    }

    #[test]
    fn incremental_edit_inserts_line() {
        clear();
        init("test.rs", "fn main() {}", "rust");

        // Insert a comment before the function: 0,0 -> 0,0 with "// hello\n"
        let ok = edit("test.rs", 0, 0, 0, 0, 0, 0, 0, 1, 0, "// hello\n");
        assert!(ok);

        let tokens = tokenize("test.rs");
        assert!(tokens.iter().any(|t| t.token_type == "comment" && t.text == "// hello"));
    }

    #[test]
    fn unsupported_language_returns_false() {
        clear();
        let ok = init("test.klingon", "hello", "klingon");
        assert!(!ok);
    }
}
