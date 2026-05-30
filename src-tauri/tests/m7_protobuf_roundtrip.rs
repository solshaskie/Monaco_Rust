use prost::Message;
use std::path::PathBuf;

use monaco_tauri::proto::code::ipc::editor;
use monaco_tauri::proto::code::ipc::editor::host;
use monaco_tauri::proto::code::ipc::editor::language;
use monaco_tauri::proto::code::ipc::file;

fn file_uri(path: &PathBuf) -> file::Uri {
    file::Uri {
        scheme: "file".to_string(),
        authority: String::new(),
        path: path.to_string_lossy().to_string(),
        query: String::new(),
        fragment: String::new(),
    }
}

// ---------------------------------------------------------------------------
// ipc_editor_host roundtrips
// ---------------------------------------------------------------------------

#[test]
fn protobuf_open_document_request_roundtrip() {
    let original = host::OpenDocumentRequest {
        resource: Some(file_uri(&PathBuf::from("/tmp/test.rs"))),
        create_if_missing: true,
        preferred_language_id: "rust".to_string(),
    };

    let encoded = original.encode_to_vec();
    let decoded = host::OpenDocumentRequest::decode(encoded.as_slice()).unwrap();

    assert_eq!(decoded.create_if_missing, original.create_if_missing);
    assert_eq!(decoded.preferred_language_id, original.preferred_language_id);
    let r = decoded.resource.unwrap();
    assert_eq!(r.path, "/tmp/test.rs");
}

#[test]
fn protobuf_apply_edits_request_roundtrip() {
    let original = host::ApplyEditsRequest {
        resource: Some(file_uri(&PathBuf::from("/tmp/test.rs"))),
        edits: vec![
            editor::ModelContentChange {
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 5,
                range_offset: 0,
                range_length: 4,
                text_utf8: b"hello".to_vec(),
            },
            editor::ModelContentChange {
                start_line: 2,
                start_column: 3,
                end_line: 2,
                end_column: 3,
                range_offset: 10,
                range_length: 0,
                text_utf8: b"!".to_vec(),
            },
        ],
    };

    let encoded = original.encode_to_vec();
    let decoded = host::ApplyEditsRequest::decode(encoded.as_slice()).unwrap();

    assert_eq!(decoded.edits.len(), 2);
    assert_eq!(decoded.edits[0].text_utf8, b"hello");
    assert_eq!(decoded.edits[1].range_offset, 10);
}

#[test]
fn protobuf_workspace_root_roundtrip() {
    let original = host::WorkspaceRoot {
        resource: Some(file_uri(&PathBuf::from("/workspace"))),
        name: "my-project".to_string(),
        is_primary: true,
    };

    let encoded = original.encode_to_vec();
    let decoded = host::WorkspaceRoot::decode(encoded.as_slice()).unwrap();

    assert_eq!(decoded.name, "my-project");
    assert!(decoded.is_primary);
}

#[test]
fn protobuf_close_document_request_roundtrip() {
    let original = host::CloseDocumentRequest {
        resource: Some(file_uri(&PathBuf::from("/tmp/close.rs"))),
        save_if_dirty: true,
    };

    let encoded = original.encode_to_vec();
    let decoded = host::CloseDocumentRequest::decode(encoded.as_slice()).unwrap();

    assert!(decoded.save_if_dirty);
    assert_eq!(decoded.resource.unwrap().path, "/tmp/close.rs");
}

// ---------------------------------------------------------------------------
// ipc_editor_language roundtrips
// ---------------------------------------------------------------------------

#[test]
fn protobuf_tokenization_request_roundtrip() {
    let original = language::TokenizationRequest {
        resource: Some(file_uri(&PathBuf::from("/tmp/main.rs"))),
        version_id: 42,
    };

    let encoded = original.encode_to_vec();
    let decoded = language::TokenizationRequest::decode(encoded.as_slice()).unwrap();

    assert_eq!(decoded.version_id, 42);
    assert_eq!(decoded.resource.unwrap().path, "/tmp/main.rs");
}

#[test]
fn protobuf_diagnostic_request_roundtrip() {
    let original = language::DiagnosticRequest {
        resource: Some(file_uri(&PathBuf::from("/tmp/main.rs"))),
        version_id: 7,
    };

    let encoded = original.encode_to_vec();
    let decoded = language::DiagnosticRequest::decode(encoded.as_slice()).unwrap();

    assert_eq!(decoded.version_id, 7);
}

#[test]
fn protobuf_diagnostic_response_roundtrip() {
    let original = language::DiagnosticResponse {
        resource: Some(file_uri(&PathBuf::from("/tmp/main.rs"))),
        version_id: 7,
        diagnostics: vec![language::Diagnostic {
            start_line: 3,
            start_column: 5,
            end_line: 3,
            end_column: 10,
            message: "unexpected token".to_string(),
            severity: language::DiagnosticSeverity::Error as i32,
            code: "E0001".to_string(),
            source: "tree-sitter".to_string(),
            related_information: vec![],
        }],
    };

    let encoded = original.encode_to_vec();
    let decoded = language::DiagnosticResponse::decode(encoded.as_slice()).unwrap();

    assert_eq!(decoded.version_id, 7);
    assert_eq!(decoded.diagnostics.len(), 1);
    assert_eq!(decoded.diagnostics[0].message, "unexpected token");
    assert_eq!(decoded.diagnostics[0].severity, 1); // Error = 1
}

#[test]
fn protobuf_completion_request_roundtrip() {
    let original = language::CompletionRequest {
        resource: Some(file_uri(&PathBuf::from("/tmp/main.rs"))),
        line: 10,
        column: 5,
        version_id: 3,
        trigger_character: ".".to_string(),
    };

    let encoded = original.encode_to_vec();
    let decoded = language::CompletionRequest::decode(encoded.as_slice()).unwrap();

    assert_eq!(decoded.line, 10);
    assert_eq!(decoded.column, 5);
    assert_eq!(decoded.trigger_character, ".");
}

#[test]
fn protobuf_completion_response_roundtrip() {
    let original = language::CompletionResponse {
        items: vec![
            language::CompletionItem {
                label: "println".to_string(),
                kind: "function".to_string(),
                detail: " Prints to stdout".to_string(),
                documentation_utf8: vec![],
                insert_text: "println!(\"$0\")".to_string(),
                sort_text: "a".to_string(),
                filter_text: "println".to_string(),
                is_snippet: true,
                additional_text_edits: vec![],
            },
        ],
        is_incomplete: false,
        version_id: 3,
    };

    let encoded = original.encode_to_vec();
    let decoded = language::CompletionResponse::decode(encoded.as_slice()).unwrap();

    assert_eq!(decoded.items.len(), 1);
    assert_eq!(decoded.items[0].label, "println");
    assert!(decoded.items[0].is_snippet);
}

#[test]
fn protobuf_document_symbol_request_roundtrip() {
    let original = language::DocumentSymbolRequest {
        resource: Some(file_uri(&PathBuf::from("/tmp/main.rs"))),
        version_id: 1,
    };

    let encoded = original.encode_to_vec();
    let decoded = language::DocumentSymbolRequest::decode(encoded.as_slice()).unwrap();

    assert_eq!(decoded.version_id, 1);
}

#[test]
fn protobuf_hover_request_roundtrip() {
    let original = language::HoverRequest {
        resource: Some(file_uri(&PathBuf::from("/tmp/main.rs"))),
        line: 5,
        column: 10,
        version_id: 2,
    };

    let encoded = original.encode_to_vec();
    let decoded = language::HoverRequest::decode(encoded.as_slice()).unwrap();

    assert_eq!(decoded.line, 5);
    assert_eq!(decoded.column, 10);
}

// ---------------------------------------------------------------------------
// ipc_editor_host response roundtrips
// ---------------------------------------------------------------------------

#[test]
fn protobuf_apply_edits_response_roundtrip() {
    let original = host::ApplyEditsResponse {
        success: true,
        new_version_id: 5,
        applied_event: Some(editor::ModelContentChangedEvent {
            resource: Some(file_uri(&PathBuf::from("/tmp/main.rs"))),
            version_id: 5,
            is_flush: false,
            is_undoing: false,
            is_redoing: false,
            changes: vec![editor::ModelContentChange {
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1,
                range_offset: 0,
                range_length: 0,
                text_utf8: b"x".to_vec(),
            }],
        }),
    };

    let encoded = original.encode_to_vec();
    let decoded = host::ApplyEditsResponse::decode(encoded.as_slice()).unwrap();

    assert!(decoded.success);
    assert_eq!(decoded.new_version_id, 5);
    let event = decoded.applied_event.unwrap();
    assert_eq!(event.changes.len(), 1);
    assert_eq!(event.changes[0].text_utf8, b"x");
}

#[test]
fn protobuf_editor_host_event_roundtrip() {
    let original = host::EditorHostEvent {
        r#type: 2, // DocumentSaved
        roots: vec![],
        snapshot: None,
        stat: None,
        previous_resource: None,
    };

    let encoded = original.encode_to_vec();
    let decoded = host::EditorHostEvent::decode(encoded.as_slice()).unwrap();

    assert_eq!(decoded.r#type, 2);
}

#[test]
fn protobuf_buffer_registry_event_roundtrip() {
    let original = host::BufferRegistryEvent {
        r#type: 1, // ContentChanged
        resource: Some(file_uri(&PathBuf::from("/tmp/main.rs"))),
        version_id: 7,
        content_change: Some(host::BufferContentChangeEvent {
            resource: Some(file_uri(&PathBuf::from("/tmp/main.rs"))),
            version_id: 7,
            changes: vec![editor::ModelContentChange {
                start_line: 2,
                start_column: 3,
                end_line: 2,
                end_column: 4,
                range_offset: 5,
                range_length: 1,
                text_utf8: b"y".to_vec(),
            }],
            is_undoing: false,
            is_redoing: false,
        }),
    };

    let encoded = original.encode_to_vec();
    let decoded = host::BufferRegistryEvent::decode(encoded.as_slice()).unwrap();

    assert_eq!(decoded.version_id, 7);
    let cc = decoded.content_change.unwrap();
    assert_eq!(cc.changes.len(), 1);
    assert_eq!(cc.changes[0].text_utf8, b"y");
}
