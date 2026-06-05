use tree_sitter::{Node, Tree};

/// A document symbol extracted from the CST.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentSymbol {
    pub name: String,
    pub detail: String,
    pub kind: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub children: Vec<DocumentSymbol>,
}

/// Extracts document symbols from a parsed Tree-sitter tree.
pub fn extract_document_symbols(tree: &Tree, source: &str) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();
    let root = tree.root_node();
    traverse_for_symbols(&root, source, &mut symbols);
    symbols
}

fn traverse_for_symbols(node: &Node, source: &str, symbols: &mut Vec<DocumentSymbol>) {
    match node.kind() {
        "function_item" | "function_declaration" => {
            push_named_symbol(node, source, symbols, "function", "function", Vec::new());
        }
        "struct_item" => {
            let children = extract_field_children(node, source);
            push_named_symbol(node, source, symbols, "struct", "struct", children);
        }
        "enum_item" | "enum_declaration" => {
            let children = extract_enum_variant_children(node, source);
            push_named_symbol(node, source, symbols, "enum", "enum", children);
        }
        "trait_item" | "interface_declaration" => {
            push_named_symbol(node, source, symbols, "trait", "interface", Vec::new());
        }
        "class_declaration" => {
            push_named_symbol(node, source, symbols, "class", "class", Vec::new());
        }
        "impl_item" => {
            if let Some(type_node) = node.child_by_field_name("type") {
                let name = type_node
                    .utf8_text(source.as_bytes())
                    .unwrap_or("")
                    .to_string();
                symbols.push(make_symbol(node, name, "impl", "class", Vec::new()));
            }
        }
        "mod_item" | "module" | "internal_module" => {
            push_named_symbol(node, source, symbols, "mod", "module", Vec::new());
        }
        "const_item" | "variable_declarator" => {
            push_named_symbol(node, source, symbols, "const", "constant", Vec::new());
        }
        "lexical_declaration" => {
            extract_variable_declaration_children(node, source, symbols);
        }
        "static_item" => {
            push_named_symbol(node, source, symbols, "static", "constant", Vec::new());
        }
        "type_item" | "type_alias_declaration" => {
            push_named_symbol(node, source, symbols, "type", "type", Vec::new());
        }
        "macro_definition" => {
            push_named_symbol(node, source, symbols, "macro", "function", Vec::new());
        }
        "use_declaration" => {
            if let Some(name) = extract_use_name(node, source) {
                symbols.push(make_symbol(node, name, "use", "module", Vec::new()));
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            traverse_for_symbols(&cursor.node(), source, symbols);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

fn make_symbol(
    node: &Node,
    name: String,
    detail: &str,
    kind: &str,
    children: Vec<DocumentSymbol>,
) -> DocumentSymbol {
    DocumentSymbol {
        name,
        detail: detail.to_string(),
        kind: kind.to_string(),
        start_line: node.start_position().row as u32 + 1,
        start_column: node.start_position().column as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        end_column: node.end_position().column as u32 + 1,
        children,
    }
}

fn push_named_symbol(
    node: &Node,
    source: &str,
    symbols: &mut Vec<DocumentSymbol>,
    detail: &str,
    kind: &str,
    children: Vec<DocumentSymbol>,
) {
    if let Some(name_node) = node.child_by_field_name("name") {
        let name = name_node
            .utf8_text(source.as_bytes())
            .unwrap_or("")
            .to_string();
        symbols.push(make_symbol(node, name, detail, kind, children));
    }
}

fn extract_variable_declaration_children(node: &Node, source: &str, symbols: &mut Vec<DocumentSymbol>) {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "variable_declarator" {
                push_named_symbol(&child, source, symbols, "const", "constant", Vec::new());
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

fn extract_field_children(node: &Node, source: &str) -> Vec<DocumentSymbol> {
    let mut children = Vec::new();
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let field = cursor.node();
                if field.kind() == "field_declaration" {
                    if let Some(name_node) = field.child_by_field_name("name") {
                        let name = name_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or("")
                            .to_string();
                        children.push(DocumentSymbol {
                            name,
                            detail: "field".to_string(),
                            kind: "property".to_string(),
                            start_line: field.start_position().row as u32 + 1,
                            start_column: field.start_position().column as u32 + 1,
                            end_line: field.end_position().row as u32 + 1,
                            end_column: field.end_position().column as u32 + 1,
                            children: Vec::new(),
                        });
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }
    children
}

fn extract_enum_variant_children(node: &Node, source: &str) -> Vec<DocumentSymbol> {
    let mut children = Vec::new();
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        if cursor.goto_first_child() {
            loop {
                let variant = cursor.node();
                if variant.kind() == "enum_variant" {
                    if let Some(name_node) = variant.child_by_field_name("name") {
                        let name = name_node
                            .utf8_text(source.as_bytes())
                            .unwrap_or("")
                            .to_string();
                        children.push(DocumentSymbol {
                            name,
                            detail: "variant".to_string(),
                            kind: "enum".to_string(),
                            start_line: variant.start_position().row as u32 + 1,
                            start_column: variant.start_position().column as u32 + 1,
                            end_line: variant.end_position().row as u32 + 1,
                            end_column: variant.end_position().column as u32 + 1,
                            children: Vec::new(),
                        });
                    }
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }
    children
}

fn extract_use_name(node: &Node, source: &str) -> Option<String> {
    if let Some(arg) = node.child_by_field_name("argument") {
        let name = arg.utf8_text(source.as_bytes()).unwrap_or("").to_string();
        // Take the last segment of a path
        let segments: Vec<&str> = name.split("::").collect();
        return segments.last().map(|s| s.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::parser::SyntaxParser;

    #[test]
    fn extract_function_symbol() {
        let source = "fn main() { let x = 1; }";
        let mut parser = SyntaxParser::for_rust().unwrap();
        let parsed = parser.parse(source).unwrap();
        let symbols = extract_document_symbols(&parsed.tree, source);

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "main");
        assert_eq!(symbols[0].kind, "function");
    }

    #[test]
    fn extract_struct_symbol() {
        let source = "struct Point { x: i32, y: i32 }";
        let mut parser = SyntaxParser::for_rust().unwrap();
        let parsed = parser.parse(source).unwrap();
        let symbols = extract_document_symbols(&parsed.tree, source);

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Point");
        assert_eq!(symbols[0].kind, "struct");
        assert_eq!(symbols[0].children.len(), 2);
    }

    #[test]
    fn extract_enum_symbol() {
        let source = "enum Color { Red, Green, Blue }";
        let mut parser = SyntaxParser::for_rust().unwrap();
        let parsed = parser.parse(source).unwrap();
        let symbols = extract_document_symbols(&parsed.tree, source);

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "Color");
        assert_eq!(symbols[0].children.len(), 3);
    }

    #[test]
    fn extract_multiple_symbols() {
        let source = r#"
fn foo() {}
struct Bar;
enum Baz { A, B }
"#;
        let mut parser = SyntaxParser::for_rust().unwrap();
        let parsed = parser.parse(source).unwrap();
        let symbols = extract_document_symbols(&parsed.tree, source);

        assert_eq!(symbols.len(), 3);
        assert!(symbols
            .iter()
            .any(|s| s.name == "foo" && s.kind == "function"));
        assert!(symbols
            .iter()
            .any(|s| s.name == "Bar" && s.kind == "struct"));
        assert!(symbols.iter().any(|s| s.name == "Baz" && s.kind == "enum"));
    }

    #[test]
    fn extract_typescript_symbols() {
        let source = "interface Point { x: number; y: number; }\nfunction main() { return 1; }\ntype Alias = Point;";
        let mut parser = SyntaxParser::for_typescript().unwrap();
        let parsed = parser.parse(source).unwrap();
        let symbols = extract_document_symbols(&parsed.tree, source);

        assert!(symbols.iter().any(|s| s.name == "Point" && s.kind == "interface"));
        assert!(symbols.iter().any(|s| s.name == "main" && s.kind == "function"));
        assert!(symbols.iter().any(|s| s.name == "Alias" && s.kind == "type"));
    }
}
