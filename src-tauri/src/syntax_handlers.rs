use crate::buffer::BufferRegistry;
use crate::lsp::convert::{
    lsp_code_action_to_proto, lsp_completion_to_proto, lsp_diagnostic_to_proto,
};
use crate::lsp::registry::LspRegistry;
use crate::proto::code::ipc::editor::language;
use crate::syntax::{
    collect_completions, collect_syntax_diagnostics, extract_document_symbols,
    extract_folding_ranges, hover_at_position, pack_semantic_tokens, prefix_at_position,
    tokenize_tree, tokenize_tree_range, SyntaxParser,
};

/// Selects the appropriate Tree-sitter parser based on a file path's extension.
fn parser_for_path(path: &str) -> Result<SyntaxParser, String> {
    let ext = path.rfind('.').map(|i| &path[i..]);
    match ext {
        Some(".js") | Some(".mjs") | Some(".cjs") => SyntaxParser::for_javascript(),
        Some(".ts") | Some(".mts") | Some(".cts") => SyntaxParser::for_typescript(),
        Some(".tsx") => SyntaxParser::for_typescript(),
        _ => SyntaxParser::for_rust(),
    }
}

/// Returns the diagnostic source label for a given file path.
fn diagnostic_source_for_path(path: &str) -> &'static str {
    let ext = path.rfind('.').map(|i| &path[i..]);
    match ext {
        Some(".js") | Some(".mjs") | Some(".cjs") => "tree-sitter-javascript",
        Some(".ts") | Some(".mts") | Some(".cts") | Some(".tsx") => "tree-sitter-typescript",
        Some(".rs") => "tree-sitter-rust",
        _ => "tree-sitter",
    }
}

/// Tokenizes the content of a buffer using Tree-sitter.
pub fn tokenize_document(
    registry: &BufferRegistry,
    request: language::TokenizationRequest,
) -> Result<language::TokenizationResponse, String> {
    let resource = request
        .resource
        .ok_or_else(|| "missing document resource".to_string())?;

    let content = registry
        .get_buffer_content(&resource.path)
        .ok_or_else(|| format!("No buffer found for resource: {}", resource.path))?;

    let version_id = registry.get_buffer_version(&resource.path).unwrap_or(1);

    let mut parser = parser_for_path(&resource.path)?;
    let parsed = parser
        .parse(&content)
        .map_err(|e| format!("Parse failed: {}", e))?;

    let has_errors = parsed.tree.root_node().has_error();
    let tokens = tokenize_tree(&parsed.tree, &content);

    let proto_tokens = tokens
        .into_iter()
        .map(|t| language::SyntaxToken {
            token_type: t.token_type,
            start_line: t.start_line,
            start_column: t.start_column,
            end_line: t.end_line,
            end_column: t.end_column,
            text: t.text,
        })
        .collect();

    Ok(language::TokenizationResponse {
        resource: Some(resource),
        version_id,
        tokens: proto_tokens,
        has_errors,
    })
}

/// Tokenizes a viewport range of lines from a buffer using Tree-sitter.
/// This is optimized for visible content only, avoiding full-file parsing overhead.
pub fn tokenize_document_range(
    registry: &BufferRegistry,
    request: language::TokenizationRequest,
    start_line: u32,
    end_line: u32,
) -> Result<language::TokenizationResponse, String> {
    let resource = request
        .resource
        .ok_or_else(|| "missing document resource".to_string())?;

    let content = registry
        .get_buffer_content(&resource.path)
        .ok_or_else(|| format!("No buffer found for resource: {}", resource.path))?;

    let version_id = registry.get_buffer_version(&resource.path).unwrap_or(1);

    let mut parser = parser_for_path(&resource.path)?;
    let parsed = parser
        .parse(&content)
        .map_err(|e| format!("Parse failed: {}", e))?;

    let has_errors = parsed.tree.root_node().has_error();
    let tokens = tokenize_tree_range(&parsed.tree, &content, start_line, end_line);

    let proto_tokens = tokens
        .into_iter()
        .map(|t| language::SyntaxToken {
            token_type: t.token_type,
            start_line: t.start_line,
            start_column: t.start_column,
            end_line: t.end_line,
            end_column: t.end_column,
            text: t.text,
        })
        .collect();

    Ok(language::TokenizationResponse {
        resource: Some(resource),
        version_id,
        tokens: proto_tokens,
        has_errors,
    })
}

/// Extracts folding ranges from the content of a buffer using Tree-sitter.
pub fn fold_document(
    registry: &BufferRegistry,
    request: language::FoldingRangeRequest,
) -> Result<language::FoldingRangeResponse, String> {
    let resource = request
        .resource
        .ok_or_else(|| "missing document resource".to_string())?;

    let content = registry
        .get_buffer_content(&resource.path)
        .ok_or_else(|| format!("No buffer found for resource: {}", resource.path))?;

    let version_id = registry.get_buffer_version(&resource.path).unwrap_or(1);

    let mut parser = parser_for_path(&resource.path)?;
    let parsed = parser
        .parse(&content)
        .map_err(|e| format!("Parse failed: {}", e))?;

    let ranges = extract_folding_ranges(&parsed.tree);

    let proto_ranges = ranges
        .into_iter()
        .map(|r| language::FoldingRange {
            start_line: r.start_line,
            end_line: r.end_line,
            kind: r.kind.unwrap_or_default(),
        })
        .collect();

    Ok(language::FoldingRangeResponse {
        resource: Some(resource),
        version_id,
        ranges: proto_ranges,
    })
}

/// Produces LSP-style packed semantic tokens from a buffer using Tree-sitter.
pub fn semantic_tokens_document(
    registry: &BufferRegistry,
    request: language::SemanticTokensRequest,
) -> Result<language::SemanticTokensResponse, String> {
    let resource = request
        .resource
        .ok_or_else(|| "missing document resource".to_string())?;

    let content = registry
        .get_buffer_content(&resource.path)
        .ok_or_else(|| format!("No buffer found for resource: {}", resource.path))?;

    let version_id = registry.get_buffer_version(&resource.path).unwrap_or(1);

    let mut parser = parser_for_path(&resource.path)?;
    let parsed = parser
        .parse(&content)
        .map_err(|e| format!("Parse failed: {}", e))?;

    let tokens = tokenize_tree(&parsed.tree, &content);
    let data = pack_semantic_tokens(&tokens);

    Ok(language::SemanticTokensResponse {
        resource: Some(resource),
        version_id,
        data,
    })
}

/// Produces LSP-style delta semantic tokens.
/// Returns empty data if the buffer version matches previous_result_id,
/// otherwise returns the full packed token array.
pub fn semantic_tokens_delta_document(
    registry: &BufferRegistry,
    request: language::SemanticTokensDeltaRequest,
) -> Result<language::SemanticTokensDeltaResponse, String> {
    let resource = request
        .resource
        .ok_or_else(|| "missing document resource".to_string())?;

    // Check buffer existence first
    let content = registry
        .get_buffer_content(&resource.path)
        .ok_or_else(|| format!("No buffer found for resource: {}", resource.path))?;

    let version_id = registry.get_buffer_version(&resource.path).unwrap_or(1);

    if request.previous_result_id == version_id {
        // No changes since the client last fetched
        return Ok(language::SemanticTokensDeltaResponse {
            resource: Some(resource),
            version_id,
            result_id: version_id,
            data: Vec::new(),
            removed: Vec::new(),
        });
    }

    let mut parser = parser_for_path(&resource.path)?;
    let parsed = parser
        .parse(&content)
        .map_err(|e| format!("Parse failed: {}", e))?;

    let tokens = tokenize_tree(&parsed.tree, &content);
    let data = pack_semantic_tokens(&tokens);

    Ok(language::SemanticTokensDeltaResponse {
        resource: Some(resource),
        version_id,
        result_id: version_id,
        data,
        removed: Vec::new(),
    })
}

/// Produces hover information at a given line/column using Tree-sitter.
pub fn hover_document(
    registry: &BufferRegistry,
    request: language::HoverRequest,
) -> Result<language::HoverResponse, String> {
    let resource = request
        .resource
        .ok_or_else(|| "missing document resource".to_string())?;

    let content = registry
        .get_buffer_content(&resource.path)
        .ok_or_else(|| format!("No buffer found for resource: {}", resource.path))?;

    let mut parser = parser_for_path(&resource.path)?;
    let parsed = parser
        .parse(&content)
        .map_err(|e| format!("Parse failed: {}", e))?;

    let hover = hover_at_position(&parsed.tree, &content, request.line, request.column)
        .ok_or_else(|| "No node found at position".to_string())?;

    Ok(language::HoverResponse {
        contents_utf8: hover.contents.into_bytes(),
        start_line: hover.start_line,
        start_column: hover.start_column,
        end_line: hover.end_line,
        end_column: hover.end_column,
    })
}

/// Collects diagnostics from a buffer.
/// Merges external LSP diagnostics with Tree-sitter parse errors.
pub fn diagnostics_document(
    registry: &BufferRegistry,
    lsp_registry: Option<&mut LspRegistry>,
    request: language::DiagnosticRequest,
) -> Result<language::DiagnosticResponse, String> {
    let resource = request
        .resource
        .ok_or_else(|| "missing document resource".to_string())?;

    let content = registry
        .get_buffer_content(&resource.path)
        .ok_or_else(|| format!("No buffer found for resource: {}", resource.path))?;

    let version_id = registry.get_buffer_version(&resource.path).unwrap_or(1);

    let mut proto_diagnostics = Vec::new();

    // Try external LSP diagnostics first
    if let Some(lsp) = lsp_registry {
        if let Some(client) = lsp.client_for_path(&resource.path) {
            let uri = if resource.path.starts_with("file://") {
                resource.path.clone()
            } else {
                format!("file://{}", resource.path)
            };
            let params = serde_json::json!({
                "textDocument": { "uri": uri },
            });
            if let Ok(result) = client.request("textDocument/diagnostic", params) {
                // Parse raw JSON to avoid lsp-types version-compatibility issues
                let items = if let Some(obj) = result.as_object() {
                    if let Some(items) = obj.get("items").and_then(|v| v.as_array()) {
                        items.clone()
                    } else if let Some(report) = obj
                        .get("fullDocumentDiagnosticReport")
                        .and_then(|v| v.as_object())
                    {
                        report
                            .get("items")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default()
                    } else if let Some(report) = obj
                        .get("relatedDocumentDiagnosticReport")
                        .and_then(|v| v.as_object())
                    {
                        report
                            .get("items")
                            .and_then(|v| v.as_array())
                            .cloned()
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    }
                } else if let Some(array) = result.as_array() {
                    array.clone()
                } else {
                    Vec::new()
                };
                for v in items {
                    if let Ok(d) = serde_json::from_value::<lsp_types::Diagnostic>(v) {
                        proto_diagnostics.push(lsp_diagnostic_to_proto(d));
                    }
                }
            }
        }
    }

    // Always include Tree-sitter parse errors
    let mut parser = parser_for_path(&resource.path)?;
    let parsed = parser
        .parse(&content)
        .map_err(|e| format!("Parse failed: {}", e))?;

    let source_label = diagnostic_source_for_path(&resource.path);
    let syntax_diagnostics = collect_syntax_diagnostics(&parsed.tree, source_label);
    for d in syntax_diagnostics {
        proto_diagnostics.push(language::Diagnostic {
            start_line: d.start_line,
            start_column: d.start_column,
            end_line: d.end_line,
            end_column: d.end_column,
            message: d.message,
            severity: d.severity.as_lsp_value(),
            code: "parse".to_string(),
            source: d.source,
            related_information: Vec::new(),
        });
    }

    Ok(language::DiagnosticResponse {
        resource: Some(resource),
        version_id,
        diagnostics: proto_diagnostics,
    })
}

/// Extracts document symbols from a buffer using Tree-sitter.
pub fn document_symbols_document(
    registry: &BufferRegistry,
    request: language::DocumentSymbolRequest,
) -> Result<language::DocumentSymbolResponse, String> {
    let resource = request
        .resource
        .ok_or_else(|| "missing document resource".to_string())?;

    let content = registry
        .get_buffer_content(&resource.path)
        .ok_or_else(|| format!("No buffer found for resource: {}", resource.path))?;

    let version_id = registry.get_buffer_version(&resource.path).unwrap_or(1);

    let mut parser = parser_for_path(&resource.path)?;
    let parsed = parser
        .parse(&content)
        .map_err(|e| format!("Parse failed: {}", e))?;

    let symbols = extract_document_symbols(&parsed.tree, &content);

    fn convert_symbol(s: crate::syntax::DocumentSymbol) -> language::DocumentSymbol {
        language::DocumentSymbol {
            name: s.name,
            detail: s.detail,
            kind: s.kind,
            start_line: s.start_line,
            start_column: s.start_column,
            end_line: s.end_line,
            end_column: s.end_column,
            children: s.children.into_iter().map(convert_symbol).collect(),
            is_deprecated: false,
        }
    }

    let proto_symbols = symbols.into_iter().map(convert_symbol).collect();

    Ok(language::DocumentSymbolResponse {
        resource: Some(resource),
        version_id,
        symbols: proto_symbols,
    })
}

/// Produces completion items for a buffer.
/// Queries an external LSP server if available, otherwise falls back to Tree-sitter document symbols.
pub fn completion_document(
    registry: &BufferRegistry,
    lsp_registry: Option<&mut LspRegistry>,
    request: language::CompletionRequest,
) -> Result<language::CompletionResponse, String> {
    let resource = request
        .resource
        .ok_or_else(|| "missing document resource".to_string())?;

    let content = registry
        .get_buffer_content(&resource.path)
        .ok_or_else(|| format!("No buffer found for resource: {}", resource.path))?;

    let version_id = registry.get_buffer_version(&resource.path).unwrap_or(1);

    // Try external LSP first
    if let Some(lsp) = lsp_registry {
        if let Some(client) = lsp.client_for_path(&resource.path) {
            let uri = if resource.path.starts_with("file://") {
                resource.path.clone()
            } else {
                format!("file://{}", resource.path)
            };
            let params = serde_json::json!({
                "textDocument": { "uri": uri },
                "position": {
                    "line": request.line.saturating_sub(1),
                    "character": request.column.saturating_sub(1),
                },
            });
            if let Ok(result) = client.request("textDocument/completion", params) {
                let items: Vec<language::CompletionItem> = if let Some(array) = result.as_array() {
                    array
                        .iter()
                        .filter_map(|v| {
                            serde_json::from_value::<lsp_types::CompletionItem>(v.clone())
                                .ok()
                                .map(lsp_completion_to_proto)
                        })
                        .collect()
                } else if let Ok(list) =
                    serde_json::from_value::<lsp_types::CompletionList>(result.clone())
                {
                    list.items
                        .into_iter()
                        .map(lsp_completion_to_proto)
                        .collect()
                } else {
                    Vec::new()
                };
                if !items.is_empty() {
                    return Ok(language::CompletionResponse {
                        items,
                        is_incomplete: false,
                        version_id,
                    });
                }
            }
        }
    }

    // Fallback to Tree-sitter document symbols
    let mut parser = parser_for_path(&resource.path)?;
    let parsed = parser
        .parse(&content)
        .map_err(|e| format!("Parse failed: {}", e))?;

    let prefix = if request.trigger_character.is_empty() {
        prefix_at_position(&content, request.line, request.column)
    } else {
        None
    };

    let completions = collect_completions(&parsed.tree, &content, prefix.as_deref());

    let proto_items = completions
        .into_iter()
        .map(|c| language::CompletionItem {
            label: c.label,
            kind: c.kind,
            detail: c.detail,
            documentation_utf8: c.documentation.into_bytes(),
            insert_text: c.insert_text,
            sort_text: c.sort_text,
            filter_text: c.filter_text,
            is_snippet: false,
            additional_text_edits: Vec::new(),
        })
        .collect();

    Ok(language::CompletionResponse {
        items: proto_items,
        is_incomplete: false,
        version_id,
    })
}

/// Produces code actions for a buffer.
/// Queries an external LSP server if available.
pub fn code_actions_document(
    registry: &BufferRegistry,
    lsp_registry: Option<&mut LspRegistry>,
    request: language::CodeActionRequest,
) -> Result<language::CodeActionResponse, String> {
    let resource = request
        .resource
        .ok_or_else(|| "missing document resource".to_string())?;

    let version_id = registry.get_buffer_version(&resource.path).unwrap_or(1);

    let mut proto_actions = Vec::new();

    if let Some(lsp) = lsp_registry {
        if let Some(client) = lsp.client_for_path(&resource.path) {
            let uri = if resource.path.starts_with("file://") {
                resource.path.clone()
            } else {
                format!("file://{}", resource.path)
            };
            let mut params = serde_json::json!({
                "textDocument": { "uri": uri },
                "range": {
                    "start": {
                        "line": request.start_line.saturating_sub(1),
                        "character": request.start_column.saturating_sub(1),
                    },
                    "end": {
                        "line": request.end_line.saturating_sub(1),
                        "character": request.end_column.saturating_sub(1),
                    },
                },
            });
            if !request.kind_filter.is_empty() {
                params["context"] = serde_json::json!({
                    "only": [request.kind_filter],
                    "diagnostics": [],
                });
            }
            if let Ok(result) = client.request("textDocument/codeAction", params) {
                if let Some(array) = result.as_array() {
                    for v in array {
                        if let Ok(action) =
                            serde_json::from_value::<lsp_types::CodeAction>(v.clone())
                        {
                            proto_actions.push(lsp_code_action_to_proto(action));
                        } else if let Ok(command) =
                            serde_json::from_value::<lsp_types::Command>(v.clone())
                        {
                            // Some LSP servers return Command objects instead of CodeAction
                            proto_actions.push(language::CodeAction {
                                title: command.title,
                                kind: String::new(),
                                diagnostics_json: Vec::new(),
                                edit_json: Vec::new(),
                                is_preferred: false,
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(language::CodeActionResponse {
        resource: Some(resource),
        version_id,
        actions: proto_actions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::BufferRegistry;
    use crate::proto::code::ipc::file;

    fn path_to_uri(path: &str) -> file::Uri {
        file::Uri {
            scheme: "file".to_string(),
            authority: String::new(),
            path: path.to_string(),
            query: String::new(),
            fragment: String::new(),
        }
    }

    #[test]
    fn tokenize_buffer_content() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://main.rs".to_string(), "fn main() { let x = 1; }");

        let response = tokenize_document(
            &registry,
            language::TokenizationRequest {
                resource: Some(path_to_uri("test://main.rs")),
                version_id: 0,
            },
        )
        .unwrap();

        assert!(!response.tokens.is_empty());
        assert!(response
            .tokens
            .iter()
            .any(|t| t.text == "fn" && t.token_type == "keyword"));
        assert!(response
            .tokens
            .iter()
            .any(|t| t.text == "main" && t.token_type == "identifier"));
        assert!(response
            .tokens
            .iter()
            .any(|t| t.text == "let" && t.token_type == "keyword"));
        assert!(response
            .tokens
            .iter()
            .any(|t| t.text == "1" && t.token_type == "number"));
    }

    #[test]
    fn tokenize_missing_buffer_fails() {
        let registry = BufferRegistry::new();

        let result = tokenize_document(
            &registry,
            language::TokenizationRequest {
                resource: Some(path_to_uri("test://missing.rs")),
                version_id: 0,
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn fold_buffer_content() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer(
            "test://main.rs".to_string(),
            "fn main() {\n    println!(\"hello\");\n}\n\nstruct Point {\n    x: i32,\n    y: i32,\n}",
        );

        let response = fold_document(
            &registry,
            language::FoldingRangeRequest {
                resource: Some(path_to_uri("test://main.rs")),
                version_id: 0,
            },
        )
        .unwrap();

        assert!(!response.ranges.is_empty());
        // Should find function fold (lines 1-3) and struct fold (lines 5-8)
        assert!(response
            .ranges
            .iter()
            .any(|r| r.start_line == 1 && r.end_line == 3));
        assert!(response
            .ranges
            .iter()
            .any(|r| r.start_line == 5 && r.end_line == 8));
    }

    #[test]
    fn fold_missing_buffer_fails() {
        let registry = BufferRegistry::new();

        let result = fold_document(
            &registry,
            language::FoldingRangeRequest {
                resource: Some(path_to_uri("test://missing.rs")),
                version_id: 0,
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn semantic_tokens_buffer_content() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://main.rs".to_string(), "fn main() { let x = 1; }");

        let response = semantic_tokens_document(
            &registry,
            language::SemanticTokensRequest {
                resource: Some(path_to_uri("test://main.rs")),
                version_id: 0,
            },
        )
        .unwrap();

        // Data should be a multiple of 5 (LSP packed format)
        assert!(!response.data.is_empty());
        assert_eq!(response.data.len() % 5, 0);

        // First token: "fn" keyword at line 1, col 1
        assert_eq!(response.data[0], 1); // deltaLine
        assert_eq!(response.data[1], 0); // deltaStart
        assert_eq!(response.data[2], 2); // length
        assert_eq!(response.data[3], 0); // keyword type index
        assert_eq!(response.data[4], 0); // modifiers
    }

    #[test]
    fn semantic_tokens_missing_buffer_fails() {
        let registry = BufferRegistry::new();

        let result = semantic_tokens_document(
            &registry,
            language::SemanticTokensRequest {
                resource: Some(path_to_uri("test://missing.rs")),
                version_id: 0,
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn semantic_tokens_delta_no_change() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://main.rs".to_string(), "fn main() { let x = 1; }");

        // Version is 1 after open_buffer. Requesting delta with result_id == version_id
        // should return empty data.
        let response = semantic_tokens_delta_document(
            &registry,
            language::SemanticTokensDeltaRequest {
                resource: Some(path_to_uri("test://main.rs")),
                version_id: 0,
                previous_result_id: 1,
            },
        )
        .unwrap();

        assert!(response.data.is_empty());
        assert!(response.removed.is_empty());
        assert_eq!(response.result_id, 1);
    }

    #[test]
    fn semantic_tokens_delta_changed() {
        use crate::buffer::{ContentChange, Position};

        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://main.rs".to_string(), "fn main() { let x = 1; }");
        // Apply an edit to bump the version
        let change = ContentChange::insert(Position::new(1, 1), "// comment\n".to_string(), 0);
        registry.apply_edit("test://main.rs", &change);

        let response = semantic_tokens_delta_document(
            &registry,
            language::SemanticTokensDeltaRequest {
                resource: Some(path_to_uri("test://main.rs")),
                version_id: 0,
                previous_result_id: 1,
            },
        )
        .unwrap();

        // Version should have changed, so we get full data
        assert!(!response.data.is_empty());
        assert_eq!(response.data.len() % 5, 0);
        assert!(response.removed.is_empty());
    }

    #[test]
    fn semantic_tokens_delta_missing_buffer_fails() {
        let registry = BufferRegistry::new();

        let result = semantic_tokens_delta_document(
            &registry,
            language::SemanticTokensDeltaRequest {
                resource: Some(path_to_uri("test://missing.rs")),
                version_id: 0,
                previous_result_id: 1,
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn hover_buffer_content() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://main.rs".to_string(), "fn main() { let x = 1; }");

        let response = hover_document(
            &registry,
            language::HoverRequest {
                resource: Some(path_to_uri("test://main.rs")),
                line: 1,
                column: 4,
                version_id: 0,
            },
        )
        .unwrap();

        let contents = String::from_utf8_lossy(&response.contents_utf8);
        assert!(contents.contains("identifier"));
        assert!(contents.contains("main"));
    }

    #[test]
    fn hover_missing_buffer_fails() {
        let registry = BufferRegistry::new();

        let result = hover_document(
            &registry,
            language::HoverRequest {
                resource: Some(path_to_uri("test://missing.rs")),
                line: 1,
                column: 1,
                version_id: 0,
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn diagnostics_no_errors_on_valid_rust() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://main.rs".to_string(), "fn main() { let x = 1; }");

        let response = diagnostics_document(
            &registry,
            None,
            language::DiagnosticRequest {
                resource: Some(path_to_uri("test://main.rs")),
                version_id: 0,
            },
        )
        .unwrap();

        assert!(response.diagnostics.is_empty());
    }

    #[test]
    fn diagnostics_detects_syntax_error() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://main.rs".to_string(), "fn main() { @ }");

        let response = diagnostics_document(
            &registry,
            None,
            language::DiagnosticRequest {
                resource: Some(path_to_uri("test://main.rs")),
                version_id: 0,
            },
        )
        .unwrap();

        assert!(!response.diagnostics.is_empty());
        assert!(response
            .diagnostics
            .iter()
            .any(|d| d.severity == language::DiagnosticSeverity::Error as i32));
    }

    #[test]
    fn diagnostics_missing_buffer_fails() {
        let registry = BufferRegistry::new();

        let result = diagnostics_document(
            &registry,
            None,
            language::DiagnosticRequest {
                resource: Some(path_to_uri("test://missing.rs")),
                version_id: 0,
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn document_symbols_buffer_content() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer(
            "test://main.rs".to_string(),
            "fn main() { let x = 1; }\nstruct Point { x: i32, y: i32 }",
        );

        let response = document_symbols_document(
            &registry,
            language::DocumentSymbolRequest {
                resource: Some(path_to_uri("test://main.rs")),
                version_id: 0,
            },
        )
        .unwrap();

        assert_eq!(response.symbols.len(), 2);
        assert!(response
            .symbols
            .iter()
            .any(|s| s.name == "main" && s.kind == "function"));
        assert!(response
            .symbols
            .iter()
            .any(|s| s.name == "Point" && s.kind == "struct"));

        // Point should have field children
        let point = response.symbols.iter().find(|s| s.name == "Point").unwrap();
        assert_eq!(point.children.len(), 2);
    }

    #[test]
    fn document_symbols_missing_buffer_fails() {
        let registry = BufferRegistry::new();

        let result = document_symbols_document(
            &registry,
            language::DocumentSymbolRequest {
                resource: Some(path_to_uri("test://missing.rs")),
                version_id: 0,
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn document_symbols_buffer_content_for_typescript() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer(
            "test://main.ts".to_string(),
            "interface Point { x: number; y: number; }\nfunction main() { return 1; }",
        );

        let response = document_symbols_document(
            &registry,
            language::DocumentSymbolRequest {
                resource: Some(path_to_uri("test://main.ts")),
                version_id: 0,
            },
        )
        .unwrap();

        assert!(response.symbols.iter().any(|s| s.name == "Point"));
        assert!(response.symbols.iter().any(|s| s.name == "main"));
    }

    #[test]
    fn completion_document_returns_symbols() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer(
            "test://main.rs".to_string(),
            "fn main() { let x = 1; }\nstruct Point { x: i32, y: i32 }",
        );

        let response = completion_document(
            &registry,
            None,
            language::CompletionRequest {
                resource: Some(path_to_uri("test://main.rs")),
                line: 1,
                column: 1,
                version_id: 0,
                trigger_character: String::new(),
            },
        )
        .unwrap();

        assert!(!response.items.is_empty());
        assert!(response.items.iter().any(|i| i.label == "main"));
        assert!(response.items.iter().any(|i| i.label == "Point"));
    }

    #[test]
    fn completion_document_filters_by_prefix() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://main.rs".to_string(), "fn main() {}\nfn make() {}");

        let response = completion_document(
            &registry,
            None,
            language::CompletionRequest {
                resource: Some(path_to_uri("test://main.rs")),
                line: 2,
                column: 5, // typing "ma"
                version_id: 0,
                trigger_character: String::new(),
            },
        )
        .unwrap();

        assert!(response.items.iter().all(|i| i.label.starts_with("ma")));
    }

    #[test]
    fn completion_missing_buffer_fails() {
        let registry = BufferRegistry::new();

        let result = completion_document(
            &registry,
            None,
            language::CompletionRequest {
                resource: Some(path_to_uri("test://missing.rs")),
                line: 1,
                column: 1,
                version_id: 0,
                trigger_character: String::new(),
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn diagnostics_detects_syntax_error_in_typescript() {
        let mut registry = BufferRegistry::new();
        registry.open_buffer("test://main.ts".to_string(), "function main( { return 1; }");

        let response = diagnostics_document(
            &registry,
            None,
            language::DiagnosticRequest {
                resource: Some(path_to_uri("test://main.ts")),
                version_id: 0,
            },
        )
        .unwrap();

        assert!(!response.diagnostics.is_empty());
        assert!(response
            .diagnostics
            .iter()
            .any(|d| d.severity == language::DiagnosticSeverity::Error as i32));
    }
}
