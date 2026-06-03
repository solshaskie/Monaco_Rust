use std::fs;
use std::path::Path;

use monaco_tauri::host_handlers::MonacoHostState;
use monaco_tauri::proto::code::ipc::editor::language;
use monaco_tauri::proto::code::ipc::file;
use monaco_tauri::syntax_handlers;

fn file_uri(path: &Path) -> file::Uri {
    file::Uri {
        scheme: "file".to_string(),
        authority: String::new(),
        path: path.to_string_lossy().to_string(),
        query: String::new(),
        fragment: String::new(),
    }
}

/// E2E: LSP completion falls back to Tree-sitter document symbols when no LSP server is available.
#[test]
fn e2e_completion_lsp_fallback_to_tree_sitter() {
    let temp_dir = std::env::temp_dir().join("monaco-e2e-completion");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let path = temp_dir.join("test.rs");
    fs::write(
        &path,
        "fn main() { let x = 1; }\nstruct Point { x: i32, y: i32 }\n",
    )
    .unwrap();

    let state = MonacoHostState::new();
    {
        let mut registry = state.buffer_registry().write().unwrap();
        registry
            .open_buffer_from_bytes(
                file_uri(&path).path.clone(),
                b"fn main() { let x = 1; }\nstruct Point { x: i32, y: i32 }\n",
            )
            .unwrap();
    }

    let registry = state.buffer_registry().read().unwrap();
    let response = syntax_handlers::completion_document(
        &registry,
        None, // No LSP registry
        language::CompletionRequest {
            resource: Some(file_uri(&path)),
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

    let _ = fs::remove_dir_all(&temp_dir);
}

/// E2E: LSP diagnostics falls back to Tree-sitter parse errors when no LSP server is available.
#[test]
fn e2e_diagnostics_lsp_fallback_to_tree_sitter() {
    let temp_dir = std::env::temp_dir().join("monaco-e2e-diagnostics");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let path = temp_dir.join("test.rs");
    fs::write(&path, "fn main() { @ }\n").unwrap();

    let state = MonacoHostState::new();
    {
        let mut registry = state.buffer_registry().write().unwrap();
        registry
            .open_buffer_from_bytes(file_uri(&path).path.clone(), b"fn main() { @ }\n")
            .unwrap();
    }

    let registry = state.buffer_registry().read().unwrap();
    let response = syntax_handlers::diagnostics_document(
        &registry,
        None, // No LSP registry
        language::DiagnosticRequest {
            resource: Some(file_uri(&path)),
            version_id: 0,
        },
    )
    .unwrap();

    assert!(!response.diagnostics.is_empty());
    assert!(response
        .diagnostics
        .iter()
        .any(|d| d.severity == language::DiagnosticSeverity::Error as i32));

    let _ = fs::remove_dir_all(&temp_dir);
}

/// E2E: LSP code actions returns empty when no LSP server is available.
#[test]
fn e2e_code_actions_empty_when_no_lsp() {
    let temp_dir = std::env::temp_dir().join("monaco-e2e-code-actions");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let path = temp_dir.join("test.rs");
    fs::write(&path, "fn main() {}\n").unwrap();

    let state = MonacoHostState::new();
    {
        let mut registry = state.buffer_registry().write().unwrap();
        registry
            .open_buffer_from_bytes(file_uri(&path).path.clone(), b"fn main() {}\n")
            .unwrap();
    }

    let registry = state.buffer_registry().read().unwrap();
    let response = syntax_handlers::code_actions_document(
        &registry,
        None, // No LSP registry
        language::CodeActionRequest {
            resource: Some(file_uri(&path)),
            version_id: 0,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 10,
            kind_filter: String::new(),
        },
    )
    .unwrap();

    assert!(response.actions.is_empty());

    let _ = fs::remove_dir_all(&temp_dir);
}

/// E2E: Document open/close triggers LSP lifecycle notifications without crashing.
#[test]
fn e2e_lsp_document_lifecycle_notifications() {
    let temp_dir = std::env::temp_dir().join("monaco-e2e-lsp-lifecycle");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let path = temp_dir.join("test.rs");
    fs::write(&path, "fn main() {}\n").unwrap();

    let state = MonacoHostState::new();

    // open_document should trigger LSP didOpen
    let opened = monaco_tauri::host_handlers::open_document(
        &state,
        monaco_tauri::proto::code::ipc::editor::host::OpenDocumentRequest {
            resource: Some(file_uri(&path)),
            create_if_missing: false,
            preferred_language_id: String::new(),
        },
    )
    .unwrap();
    assert!(opened.snapshot.is_some());

    // apply_edits should trigger LSP didChange
    let edited = monaco_tauri::host_handlers::apply_edits(
        &state,
        monaco_tauri::proto::code::ipc::editor::host::ApplyEditsRequest {
            resource: Some(file_uri(&path)),
            edits: vec![monaco_tauri::proto::code::ipc::editor::ModelContentChange {
                start_line: 1,
                start_column: 11,
                end_line: 1,
                end_column: 11,
                range_offset: 10,
                range_length: 0,
                text_utf8: b"// edit".to_vec(),
            }],
        },
    )
    .unwrap();
    assert!(edited.success);

    // close_document should trigger LSP didClose
    let closed = monaco_tauri::host_handlers::close_document(
        &state,
        monaco_tauri::proto::code::ipc::editor::host::CloseDocumentRequest {
            resource: Some(file_uri(&path)),
            save_if_dirty: false,
        },
    )
    .unwrap();
    assert!(closed.closed);

    let _ = fs::remove_dir_all(&temp_dir);
}
