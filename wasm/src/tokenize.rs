use serde::{Deserialize, Serialize};

/// A syntax token produced by the WASM tokenizer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WasmToken {
    pub text: String,
    pub token_type: String,
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

/// Tokenize an entire source string using lightweight heuristics.
/// This is a stopgap until web-tree-sitter or compiled grammar WASM modules
/// are integrated. It handles Rust, JavaScript, and TypeScript basics.
pub fn tokenize_source(source: &str, language: &str) -> Vec<WasmToken> {
    let mut tokens = Vec::new();
    let is_rust = language == "rust";
    let is_js = language == "javascript" || language == "typescript" || language == "js" || language == "ts";

    for (line_idx, line) in source.lines().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let ch = chars[i];
            let start = i;

            // Skip whitespace
            if ch.is_whitespace() {
                i += 1;
                continue;
            }

            // Comments
            if ch == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
                let text: String = chars[i..].iter().collect();
                tokens.push(WasmToken {
                    text,
                    token_type: "comment".to_string(),
                    line: line_idx,
                    start,
                    end: chars.len(),
                });
                break;
            }
            if is_rust && ch == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
                let text: String = chars[i..].iter().collect();
                tokens.push(WasmToken {
                    text,
                    token_type: "comment".to_string(),
                    line: line_idx,
                    start,
                    end: chars.len(),
                });
                break;
            }

            // String literals
            if ch == '"' || ch == '\'' || (is_rust && ch == 'b' && i + 1 < chars.len() && chars[i + 1] == '"') {
                let quote = if is_rust && ch == 'b' { chars[i + 1] } else { ch };
                let mut j = if is_rust && ch == 'b' { i + 2 } else { i + 1 };
                while j < chars.len() {
                    if chars[j] == '\\' && j + 1 < chars.len() {
                        j += 2;
                    } else if chars[j] == quote {
                        j += 1;
                        break;
                    } else {
                        j += 1;
                    }
                }
                let text: String = chars[i..j].iter().collect();
                tokens.push(WasmToken {
                    text,
                    token_type: "string".to_string(),
                    line: line_idx,
                    start,
                    end: j,
                });
                i = j;
                continue;
            }

            // Raw string literals in Rust
            if is_rust && ch == 'r' && i + 1 < chars.len() && chars[i + 1] == '#' {
                let mut j = i + 1;
                let mut hashes = 0;
                while j < chars.len() && chars[j] == '#' {
                    hashes += 1;
                    j += 1;
                }
                if j < chars.len() && (chars[j] == '"' || chars[j] == '\'') {
                    let quote = chars[j];
                    j += 1;
                    while j < chars.len() {
                        if chars[j] == quote {
                            let mut end_hashes = 0;
                            let mut k = j + 1;
                            while k < chars.len() && end_hashes < hashes && chars[k] == '#' {
                                end_hashes += 1;
                                k += 1;
                            }
                            if end_hashes == hashes {
                                j = k;
                                break;
                            }
                        }
                        j += 1;
                    }
                    let text: String = chars[i..j].iter().collect();
                    tokens.push(WasmToken {
                        text,
                        token_type: "string".to_string(),
                        line: line_idx,
                        start,
                        end: j,
                    });
                    i = j;
                    continue;
                }
            }

            // Numbers
            if ch.is_ascii_digit() || (ch == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()) {
                let mut j = i;
                while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '.' || chars[j] == '_') {
                    j += 1;
                }
                let text: String = chars[i..j].iter().collect();
                tokens.push(WasmToken {
                    text,
                    token_type: "number".to_string(),
                    line: line_idx,
                    start,
                    end: j,
                });
                i = j;
                continue;
            }

            // Keywords and identifiers
            if ch.is_alphabetic() || ch == '_' {
                let mut j = i;
                while j < chars.len() && (chars[j].is_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
                let text: String = chars[i..j].iter().collect();
                let token_type = if is_keyword(&text, is_rust, is_js) {
                    "keyword"
                } else {
                    "identifier"
                };
                tokens.push(WasmToken {
                    text,
                    token_type: token_type.to_string(),
                    line: line_idx,
                    start,
                    end: j,
                });
                i = j;
                continue;
            }

            // Operators and punctuation (single-char or multi-char)
            if is_operator_char(ch) {
                let mut j = i + 1;
                while j < chars.len() && is_operator_char(chars[j]) {
                    j += 1;
                }
                let text: String = chars[i..j].iter().collect();
                tokens.push(WasmToken {
                    text,
                    token_type: "operator".to_string(),
                    line: line_idx,
                    start,
                    end: j,
                });
                i = j;
                continue;
            }

            // Single punctuation
            let text = ch.to_string();
            let token_type = if "{}[]()<>;:,.?".contains(ch) {
                "punctuation"
            } else {
                "operator"
            };
            tokens.push(WasmToken {
                text,
                token_type: token_type.to_string(),
                line: line_idx,
                start,
                end: i + 1,
            });
            i += 1;
        }
    }

    tokens
}

/// Tokenize only a range of lines [start_line, end_line).
pub fn tokenize_source_range(
    source: &str,
    language: &str,
    start_line: usize,
    end_line: usize,
) -> Vec<WasmToken> {
    let lines: Vec<&str> = source.lines().collect();
    let start = start_line.min(lines.len());
    let end = end_line.min(lines.len());
    let slice = lines[start..end].join("\n");
    let mut tokens = tokenize_source(&slice, language);
    // Adjust line numbers to be relative to the original source
    for token in &mut tokens {
        token.line += start_line;
    }
    tokens
}

fn is_operator_char(ch: char) -> bool {
    "+-*/%=!&|^~<>".contains(ch)
}

fn is_keyword(word: &str, is_rust: bool, is_js: bool) -> bool {
    if is_rust {
        matches!(
            word,
            "use"
                | "mod"
                | "fn"
                | "let"
                | "mut"
                | "const"
                | "static"
                | "struct"
                | "enum"
                | "trait"
                | "impl"
                | "pub"
                | "crate"
                | "self"
                | "Self"
                | "super"
                | "where"
                | "for"
                | "if"
                | "else"
                | "match"
                | "while"
                | "loop"
                | "break"
                | "continue"
                | "return"
                | "async"
                | "await"
                | "move"
                | "unsafe"
                | "type"
                | "as"
                | "ref"
                | "box"
                | "dyn"
                | "macro_rules"
        )
    } else if is_js {
        matches!(
            word,
            "function"
                | "var"
                | "let"
                | "const"
                | "if"
                | "else"
                | "for"
                | "while"
                | "do"
                | "switch"
                | "case"
                | "break"
                | "continue"
                | "return"
                | "try"
                | "catch"
                | "finally"
                | "throw"
                | "new"
                | "this"
                | "typeof"
                | "instanceof"
                | "void"
                | "delete"
                | "in"
                | "of"
                | "with"
                | "yield"
                | "await"
                | "async"
                | "class"
                | "extends"
                | "super"
                | "import"
                | "export"
                | "from"
                | "default"
                | "static"
                | "get"
                | "set"
                | "constructor"
                | "interface"
                | "type"
                | "enum"
                | "namespace"
                | "module"
                | "declare"
                | "abstract"
                | "readonly"
                | "public"
                | "private"
                | "protected"
        )
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_rust_hello() {
        let tokens = tokenize_source("fn main() {\n    println!(\"hello\");\n}", "rust");
        assert!(!tokens.is_empty());
        let keywords: Vec<_> = tokens.iter().filter(|t| t.token_type == "keyword").collect();
        assert!(keywords.iter().any(|t| t.text == "fn"));
        assert!(keywords.iter().any(|t| t.text == "let" || !keywords.iter().any(|t| t.text == "println")));
    }

    #[test]
    fn tokenize_range_returns_relative_lines() {
        let source = "line1\nline2\nline3\nline4";
        let tokens = tokenize_source_range(source, "rust", 1, 3);
        for t in &tokens {
            assert!(t.line >= 1 && t.line < 3);
        }
    }

    #[test]
    fn tokenize_rust_raw_string() {
        let tokens = tokenize_source("let x = r#\"hello world\"#;", "rust");
        let string_tok = tokens.iter().find(|t| t.token_type == "string");
        assert!(string_tok.is_some());
        assert_eq!(string_tok.unwrap().text, "r#\"hello world\"#");
    }
}
