use tree_sitter::Tree;

use crate::syntax::symbols::extract_document_symbols;

/// A completion item derived from document symbols.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: String,
    pub detail: String,
    pub documentation: String,
    pub insert_text: String,
    pub sort_text: String,
    pub filter_text: String,
}

/// Collects completion candidates from document symbols.
/// If `prefix` is provided, only symbols whose name starts with the prefix are returned.
pub fn collect_completions(tree: &Tree, source: &str, prefix: Option<&str>) -> Vec<CompletionItem> {
    let symbols = extract_document_symbols(tree, source);
    let mut items = Vec::new();
    for symbol in symbols {
        if let Some(p) = prefix {
            if !symbol.name.starts_with(p) {
                continue;
            }
        }
        items.push(CompletionItem {
            label: symbol.name.clone(),
            kind: map_symbol_kind_to_completion_kind(&symbol.kind),
            detail: symbol.detail.clone(),
            documentation: format!("`{}` — {}", symbol.name, symbol.kind),
            insert_text: symbol.name.clone(),
            sort_text: symbol.name.to_lowercase(),
            filter_text: symbol.name.clone(),
        });
        // Include children (fields, variants) as completion candidates
        for child in symbol.children {
            if let Some(p) = prefix {
                if !child.name.starts_with(p) {
                    continue;
                }
            }
            items.push(CompletionItem {
                label: child.name.clone(),
                kind: map_symbol_kind_to_completion_kind(&child.kind),
                detail: child.detail.clone(),
                documentation: format!("`{}` — {}", child.name, child.kind),
                insert_text: child.name.clone(),
                sort_text: child.name.to_lowercase(),
                filter_text: child.name.clone(),
            });
        }
    }
    // Deduplicate by label
    items.sort_by(|a, b| a.label.cmp(&b.label));
    items.dedup_by(|a, b| a.label == b.label);
    items
}

/// Extracts the word prefix at a given 1-indexed line and column in the source.
pub fn prefix_at_position(source: &str, line: u32, column: u32) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    let line_idx = (line as usize).saturating_sub(1);
    let line_text = lines.get(line_idx)?;
    let col_idx = (column as usize).saturating_sub(1);
    let prefix = &line_text[..col_idx.min(line_text.len())];
    // Find the start of the current identifier/word
    let start = prefix
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let word = &prefix[start..];
    if word.is_empty() {
        None
    } else {
        Some(word.to_string())
    }
}

fn map_symbol_kind_to_completion_kind(kind: &str) -> String {
    match kind {
        "function" | "method" => "function",
        "struct" | "class" => "class",
        "enum" => "enum",
        "trait" | "interface" => "interface",
        "field" | "property" => "field",
        "variable" | "const" | "static" => "variable",
        "module" => "module",
        "type" => "type",
        "macro" => "value",
        _ => "text",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::parser::SyntaxParser;

    #[test]
    fn completions_include_symbols() {
        let source = "fn main() { let x = 1; }\nstruct Point { x: i32, y: i32 }";
        let mut parser = SyntaxParser::for_rust().unwrap();
        let parsed = parser.parse(source).unwrap();
        let items = collect_completions(&parsed.tree, source, None);
        assert!(!items.is_empty());
        assert!(items.iter().any(|i| i.label == "main"));
        assert!(items.iter().any(|i| i.label == "Point"));
        assert!(items.iter().any(|i| i.label == "x"));
    }

    #[test]
    fn completions_filter_by_prefix() {
        let source = "fn main() { let abc = 1; }\nfn abc_def() {}";
        let mut parser = SyntaxParser::for_rust().unwrap();
        let parsed = parser.parse(source).unwrap();
        let items = collect_completions(&parsed.tree, source, Some("abc"));
        assert!(items.iter().all(|i| i.label.starts_with("abc")));
    }

    #[test]
    fn prefix_at_position_finds_word() {
        let source = "fn main() { let abc = 1; }";
        assert_eq!(prefix_at_position(source, 1, 1), None);
        assert_eq!(prefix_at_position(source, 1, 3), Some("fn".to_string()));
        assert_eq!(prefix_at_position(source, 1, 8), Some("main".to_string()));
    }
}
