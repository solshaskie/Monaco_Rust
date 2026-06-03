//! Conversions between LSP types (`lsp-types` crate) and internal protobuf types.

use crate::proto::code::ipc::editor::language;
use crate::syntax::DiagnosticSeverity;

/// Converts an LSP `CompletionItem` to our protobuf `CompletionItem`.
pub fn lsp_completion_to_proto(item: lsp_types::CompletionItem) -> language::CompletionItem {
    language::CompletionItem {
        label: item.label,
        kind: item
            .kind
            .map(|k| format!("{:?}", k).to_lowercase())
            .unwrap_or_else(|| "text".to_string()),
        detail: item.detail.unwrap_or_default(),
        documentation_utf8: item
            .documentation
            .map(|d| match d {
                lsp_types::Documentation::String(s) => s,
                lsp_types::Documentation::MarkupContent(m) => m.value,
            })
            .unwrap_or_default()
            .into_bytes(),
        insert_text: item.insert_text.unwrap_or_default(),
        sort_text: item.sort_text.unwrap_or_default(),
        filter_text: item.filter_text.unwrap_or_default(),
        is_snippet: item.insert_text_format == Some(lsp_types::InsertTextFormat::SNIPPET),
        additional_text_edits: Vec::new(), // Simplified for MVP
    }
}

/// Converts an LSP `Diagnostic` to our protobuf `Diagnostic`.
pub fn lsp_diagnostic_to_proto(d: lsp_types::Diagnostic) -> language::Diagnostic {
    language::Diagnostic {
        start_line: d.range.start.line + 1,
        start_column: d.range.start.character + 1,
        end_line: d.range.end.line + 1,
        end_column: d.range.end.character + 1,
        message: d.message,
        severity: d
            .severity
            .map(|s| match s {
                lsp_types::DiagnosticSeverity::ERROR => language::DiagnosticSeverity::Error as i32,
                lsp_types::DiagnosticSeverity::WARNING => {
                    language::DiagnosticSeverity::Warning as i32
                }
                lsp_types::DiagnosticSeverity::INFORMATION => {
                    language::DiagnosticSeverity::Information as i32
                }
                lsp_types::DiagnosticSeverity::HINT => language::DiagnosticSeverity::Hint as i32,
                _ => language::DiagnosticSeverity::Error as i32,
            })
            .unwrap_or(language::DiagnosticSeverity::Error as i32),
        code: d
            .code
            .map(|c| match c {
                lsp_types::NumberOrString::Number(n) => n.to_string(),
                lsp_types::NumberOrString::String(s) => s,
            })
            .unwrap_or_default(),
        source: d.source.unwrap_or_else(|| "lsp".to_string()),
        related_information: Vec::new(),
    }
}

/// Converts an LSP `CodeAction` to our protobuf `CodeAction`.
pub fn lsp_code_action_to_proto(action: lsp_types::CodeAction) -> language::CodeAction {
    let edit_json = action
        .edit
        .map(|e| serde_json::to_string(&e).unwrap_or_default());
    language::CodeAction {
        title: action.title,
        kind: action
            .kind
            .map(|k| k.as_str().to_string())
            .unwrap_or_default(),
        diagnostics_json: Vec::new(),
        edit_json: edit_json.unwrap_or_default().into_bytes(),
        is_preferred: action.is_preferred.unwrap_or(false),
    }
}

/// Converts our internal `DiagnosticSeverity` to the protobuf enum value.
pub fn severity_to_proto(sev: &DiagnosticSeverity) -> i32 {
    match sev {
        DiagnosticSeverity::Error => language::DiagnosticSeverity::Error as i32,
        DiagnosticSeverity::Warning => language::DiagnosticSeverity::Warning as i32,
        DiagnosticSeverity::Information => language::DiagnosticSeverity::Information as i32,
        DiagnosticSeverity::Hint => language::DiagnosticSeverity::Hint as i32,
    }
}
