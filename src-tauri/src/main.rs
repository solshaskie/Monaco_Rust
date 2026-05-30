#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use prost::Message;
use monaco_tauri::events::{
    BufferChangeEvent, BufferClosedEvent, BufferContentChangedEvent, BufferOpenedEvent,
    BufferSavedEvent, EventBroadcaster,
};
use monaco_tauri::host_handlers;
use monaco_tauri::host_handlers::MonacoHostState;
use monaco_tauri::proto::code::ipc::editor::host;
use monaco_tauri::proto::code::ipc::file;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

fn decode_message<M: Message + Default>(payload: Vec<u8>) -> Result<M, String> {
    M::decode(payload.as_slice()).map_err(|err| err.to_string())
}

fn encode_message<M: Message>(message: M) -> Result<Vec<u8>, String> {
    let mut buffer = Vec::new();
    message.encode(&mut buffer).map_err(|err| err.to_string())?;
    Ok(buffer)
}

#[derive(Serialize)]
struct WorkspaceRootJson {
    path: String,
    name: String,
    is_primary: bool,
}

#[derive(Serialize)]
struct DirectoryEntryJson {
    name: String,
    path: Option<String>,
    is_directory: bool,
    size: i64,
}

#[derive(Serialize)]
struct OpenDocumentJson {
    path: String,
    language_id: String,
    content: String,
    version_id: u64,
}

#[derive(Deserialize)]
struct SaveDocumentJsonRequest {
    path: String,
    content: String,
    version_id: u64,
}

#[derive(Deserialize)]
struct CloseDocumentJsonRequest {
    path: String,
    save_if_dirty: bool,
}

#[derive(Serialize)]
struct CloseDocumentJsonResponse {
    closed: bool,
    was_dirty: bool,
}

#[derive(Deserialize)]
struct UndoRedoJsonRequest {
    path: String,
}

#[derive(Serialize)]
struct UndoRedoJsonResponse {
    success: bool,
    version_id: u64,
    changes: Vec<EditChangeJson>,
}

#[derive(Deserialize, Serialize, Clone)]
struct EditChangeJson {
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
    range_offset: u64,
    range_length: u64,
    text: String,
}

#[derive(Deserialize)]
struct ApplyEditsJsonRequest {
    path: String,
    edits: Vec<EditChangeJson>,
}

#[derive(Serialize)]
struct ApplyEditsJsonResponse {
    success: bool,
    new_version_id: u64,
}

#[derive(Deserialize)]
struct TokenizeDocumentJsonRequest {
    path: String,
}

#[derive(Serialize)]
struct SyntaxTokenJson {
    token_type: String,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
    text: String,
}

#[derive(Serialize)]
struct TokenizeDocumentJsonResponse {
    version_id: u64,
    tokens: Vec<SyntaxTokenJson>,
    has_errors: bool,
}

#[derive(Deserialize)]
struct FoldingRangeJsonRequest {
    path: String,
}

#[derive(Serialize)]
struct FoldingRangeJson {
    start_line: u32,
    end_line: u32,
    kind: String,
}

#[derive(Serialize)]
struct FoldingRangeJsonResponse {
    version_id: u64,
    ranges: Vec<FoldingRangeJson>,
}

#[derive(Deserialize)]
struct SemanticTokensJsonRequest {
    path: String,
}

#[derive(Serialize)]
struct SemanticTokensJsonResponse {
    version_id: u64,
    data: Vec<u32>,
}

#[derive(Deserialize)]
struct SemanticTokensDeltaJsonRequest {
    path: String,
    previous_result_id: u64,
}

#[derive(Deserialize)]
struct TokenizeRangeJsonRequest {
    path: String,
    start_line: u32,
    end_line: u32,
}

#[derive(Serialize)]
struct SemanticTokensDeltaJsonResponse {
    version_id: u64,
    result_id: u64,
    data: Vec<u32>,
    removed: Vec<u32>,
}

#[derive(Deserialize)]
struct HoverJsonRequest {
    path: String,
    line: u32,
    column: u32,
}

#[derive(Serialize)]
struct HoverJsonResponse {
    contents: String,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
}

#[derive(Deserialize)]
struct DiagnosticJsonRequest {
    path: String,
}

#[derive(Serialize, Clone)]
struct DiagnosticJson {
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
    message: String,
    severity: i32,
    code: String,
    source: String,
}

#[derive(Serialize)]
struct DiagnosticJsonResponse {
    version_id: u64,
    diagnostics: Vec<DiagnosticJson>,
}

#[derive(Deserialize)]
struct DocumentSymbolJsonRequest {
    path: String,
}

#[derive(Serialize)]
struct DocumentSymbolJson {
    name: String,
    detail: String,
    kind: String,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
    children: Vec<DocumentSymbolJson>,
}

#[derive(Serialize)]
struct DocumentSymbolJsonResponse {
    version_id: u64,
    symbols: Vec<DocumentSymbolJson>,
}

#[derive(Deserialize)]
struct CompletionJsonRequest {
    path: String,
    line: u32,
    column: u32,
    trigger_character: String,
}

#[derive(Serialize)]
struct CompletionItemJson {
    label: String,
    kind: String,
    detail: String,
    documentation: String,
    insert_text: String,
    sort_text: String,
    filter_text: String,
}

#[derive(Serialize)]
struct CompletionJsonResponse {
    items: Vec<CompletionItemJson>,
    is_incomplete: bool,
    version_id: u64,
}

#[derive(Serialize, Clone)]
struct DiagnosticsChangedEvent {
    path: String,
    version_id: u64,
    diagnostics: Vec<DiagnosticJson>,
}

fn file_uri_from_path(path: &str) -> file::Uri {
    file::Uri {
        scheme: "file".to_string(),
        authority: String::new(),
        path: path.to_string(),
        query: String::new(),
        fragment: String::new(),
    }
}

#[tauri::command]
fn workspace_roots_json(state: State<MonacoHostState>) -> Result<Vec<WorkspaceRootJson>, String> {
    let response = host_handlers::get_workspace_roots(&state);
    Ok(response
        .roots
        .into_iter()
        .map(|root| WorkspaceRootJson {
            path: root
                .resource
                .as_ref()
                .map(|resource| resource.path.clone())
                .unwrap_or_default(),
            name: root.name,
            is_primary: root.is_primary,
        })
        .collect())
}

#[tauri::command]
fn list_directory_json(
    state: State<MonacoHostState>,
    path: String,
) -> Result<Vec<DirectoryEntryJson>, String> {
    // Security: check list directory permission
    let principal = monaco_tauri::security::Principal {
        id: "user".to_string(),
        kind: monaco_tauri::security::PrincipalKind::User,
    };
    if !state.capabilities().check(&principal, monaco_tauri::security::Permission::ListDirectory, &path) {
        return Err("permission denied: list_directory".to_string());
    }

    let response = host_handlers::list_directory(host::ListDirectoryRequest {
        resource: Some(file_uri_from_path(&path)),
        include_file_stats: true,
    })?;

    Ok(response
        .entries
        .into_iter()
        .map(|entry| {
            let stat = entry.stat;
            DirectoryEntryJson {
                name: entry.name,
                path: stat
                    .as_ref()
                    .and_then(|value| value.resource.as_ref())
                    .map(|resource| resource.path.clone()),
                is_directory: stat.as_ref().map(|value| value.is_directory).unwrap_or(false),
                size: stat.as_ref().map(|value| value.size).unwrap_or_default(),
            }
        })
        .collect())
}

#[tauri::command]
fn open_document_json(
    app: tauri::AppHandle,
    state: State<MonacoHostState>,
    path: String,
) -> Result<OpenDocumentJson, String> {
    let response = host_handlers::open_document(&state, host::OpenDocumentRequest {
        resource: Some(file_uri_from_path(&path)),
        create_if_missing: false,
        preferred_language_id: String::new(),
    })?;
    let snapshot = response
        .snapshot
        .ok_or_else(|| "missing snapshot in open document response".to_string())?;
    EventBroadcaster::emit_buffer_opened(
        &app,
        BufferOpenedEvent {
            path: path.clone(),
            version_id: snapshot.version_id,
            language_id: response.language_id.clone(),
        },
    );
    Ok(OpenDocumentJson {
        path,
        language_id: response.language_id,
        content: String::from_utf8(snapshot.content_utf8).map_err(|err| err.to_string())?,
        version_id: snapshot.version_id,
    })
}

#[tauri::command]
fn save_document_json(
    app: tauri::AppHandle,
    state: State<MonacoHostState>,
    request: SaveDocumentJsonRequest,
) -> Result<u64, String> {
    let response = host_handlers::save_document(&state, host::SaveDocumentRequest {
        snapshot: Some(monaco_tauri::proto::code::ipc::editor::BufferSnapshot {
            resource: Some(file_uri_from_path(&request.path)),
            version_id: request.version_id,
            content_utf8: request.content.into_bytes(),
            eol: "\n".to_string(),
            is_dirty: true,
        }),
        create: true,
        overwrite: true,
        etag: String::new(),
    })?;
    EventBroadcaster::emit_buffer_saved(
        &app,
        BufferSavedEvent {
            path: request.path.clone(),
            version_id: response.persisted_version_id,
        },
    );
    Ok(response.persisted_version_id)
}

#[tauri::command]
fn get_workspace_roots(state: State<MonacoHostState>) -> Result<Vec<u8>, String> {
    encode_message(host_handlers::get_workspace_roots(&state))
}

#[tauri::command]
fn set_primary_workspace_root(
    payload: Vec<u8>,
    state: State<MonacoHostState>,
) -> Result<Vec<u8>, String> {
    let request = decode_message::<host::SetPrimaryWorkspaceRootRequest>(payload)?;
    let response = host_handlers::set_primary_workspace_root(&state, request)?;
    encode_message(response)
}

#[tauri::command]
fn open_document(state: State<MonacoHostState>, payload: Vec<u8>) -> Result<Vec<u8>, String> {
    let request = decode_message::<host::OpenDocumentRequest>(payload)?;
    let response = host_handlers::open_document(&state, request)?;
    encode_message(response)
}

#[tauri::command]
fn save_document(state: State<MonacoHostState>, payload: Vec<u8>) -> Result<Vec<u8>, String> {
    let request = decode_message::<host::SaveDocumentRequest>(payload)?;
    let response = host_handlers::save_document(&state, request)?;
    encode_message(response)
}

#[tauri::command]
fn save_document_as(state: State<MonacoHostState>, payload: Vec<u8>) -> Result<Vec<u8>, String> {
    let request = decode_message::<host::SaveDocumentAsRequest>(payload)?;
    let response = host_handlers::save_document_as(&state, request)?;
    encode_message(response)
}

#[tauri::command]
fn list_directory(payload: Vec<u8>) -> Result<Vec<u8>, String> {
    let request = decode_message::<host::ListDirectoryRequest>(payload)?;
    let response = host_handlers::list_directory(request)?;
    encode_message(response)
}

#[tauri::command]
fn close_document(state: State<MonacoHostState>, payload: Vec<u8>) -> Result<Vec<u8>, String> {
    let request = decode_message::<host::CloseDocumentRequest>(payload)?;
    let response = host_handlers::close_document(&state, request)?;
    encode_message(response)
}

#[tauri::command]
fn close_document_json(
    app: tauri::AppHandle,
    state: State<MonacoHostState>,
    request: CloseDocumentJsonRequest,
) -> Result<CloseDocumentJsonResponse, String> {
    let response = host_handlers::close_document(&state, host::CloseDocumentRequest {
        resource: Some(file_uri_from_path(&request.path)),
        save_if_dirty: request.save_if_dirty,
    })?;
    EventBroadcaster::emit_buffer_closed(
        &app,
        BufferClosedEvent {
            path: request.path.clone(),
        },
    );
    Ok(CloseDocumentJsonResponse {
        closed: response.closed,
        was_dirty: response.was_dirty,
    })
}

#[tauri::command]
fn apply_edits(state: State<MonacoHostState>, payload: Vec<u8>) -> Result<Vec<u8>, String> {
    let request = decode_message::<host::ApplyEditsRequest>(payload)?;
    let response = host_handlers::apply_edits(&state, request)?;
    encode_message(response)
}

#[tauri::command]
fn apply_edits_json(
    app: tauri::AppHandle,
    state: State<MonacoHostState>,
    request: ApplyEditsJsonRequest,
) -> Result<ApplyEditsJsonResponse, String> {
    let buffer_changes: Vec<BufferChangeEvent> = request
        .edits
        .iter()
        .map(|e| BufferChangeEvent {
            start_line: e.start_line,
            start_column: e.start_column,
            end_line: e.end_line,
            end_column: e.end_column,
            range_offset: e.range_offset,
            range_length: e.range_length,
            text: e.text.clone(),
        })
        .collect();
    let edits: Vec<monaco_tauri::proto::code::ipc::editor::ModelContentChange> = request
        .edits
        .into_iter()
        .map(|edit| monaco_tauri::proto::code::ipc::editor::ModelContentChange {
            start_line: edit.start_line,
            start_column: edit.start_column,
            end_line: edit.end_line,
            end_column: edit.end_column,
            range_offset: edit.range_offset,
            range_length: edit.range_length,
            text_utf8: edit.text.into_bytes(),
        })
        .collect();
    let response = host_handlers::apply_edits(&state, host::ApplyEditsRequest {
        resource: Some(file_uri_from_path(&request.path)),
        edits,
    })?;

    // Emit buffer content changed event for multi-view sync
    EventBroadcaster::emit_buffer_content_changed(
        &app,
        BufferContentChangedEvent {
            path: request.path.clone(),
            version_id: response.new_version_id,
            changes: buffer_changes,
            is_undoing: false,
            is_redoing: false,
        },
    );

    // Push diagnostics after edit
    let registry = state.buffer_registry().read().map_err(|e| e.to_string())?;
    if let Ok(diag_response) = monaco_tauri::syntax_handlers::diagnostics_document(
        &registry,
        monaco_tauri::proto::code::ipc::editor::language::DiagnosticRequest {
            resource: Some(file_uri_from_path(&request.path)),
            version_id: 0,
        },
    ) {
        let diagnostics: Vec<DiagnosticJson> = diag_response
            .diagnostics
            .into_iter()
            .map(|d| DiagnosticJson {
                start_line: d.start_line,
                start_column: d.start_column,
                end_line: d.end_line,
                end_column: d.end_column,
                message: d.message,
                severity: d.severity,
                code: d.code,
                source: d.source,
            })
            .collect();
        let _ = app.emit(
            "diagnostics-changed",
            DiagnosticsChangedEvent {
                path: request.path.clone(),
                version_id: diag_response.version_id,
                diagnostics,
            },
        );
    }

    Ok(ApplyEditsJsonResponse {
        success: response.success,
        new_version_id: response.new_version_id,
    })
}

#[tauri::command]
fn get_buffer_snapshot(state: State<MonacoHostState>, payload: Vec<u8>) -> Result<Vec<u8>, String> {
    let request = decode_message::<host::GetBufferSnapshotRequest>(payload)?;
    let response = host_handlers::get_buffer_snapshot(&state, request)?;
    encode_message(response)
}

#[tauri::command]
fn undo(state: State<MonacoHostState>, payload: Vec<u8>) -> Result<Vec<u8>, String> {
    let request = decode_message::<host::UndoRequest>(payload)?;
    let response = host_handlers::undo(&state, request)?;
    encode_message(response)
}

#[tauri::command]
fn undo_json(
    app: tauri::AppHandle,
    state: State<MonacoHostState>,
    request: UndoRedoJsonRequest,
) -> Result<UndoRedoJsonResponse, String> {
    let response = host_handlers::undo(&state, host::UndoRequest {
        resource: Some(file_uri_from_path(&request.path)),
    })?;
    let changes: Vec<EditChangeJson> = response
        .changes
        .iter()
        .map(|c| EditChangeJson {
            start_line: c.start_line,
            start_column: c.start_column,
            end_line: c.end_line,
            end_column: c.end_column,
            range_offset: c.range_offset,
            range_length: c.range_length,
            text: String::from_utf8_lossy(&c.text_utf8).to_string(),
        })
        .collect();
    let buffer_changes: Vec<BufferChangeEvent> = response
        .changes
        .iter()
        .map(|c| BufferChangeEvent {
            start_line: c.start_line,
            start_column: c.start_column,
            end_line: c.end_line,
            end_column: c.end_column,
            range_offset: c.range_offset,
            range_length: c.range_length,
            text: String::from_utf8_lossy(&c.text_utf8).to_string(),
        })
        .collect();
    EventBroadcaster::emit_buffer_content_changed(
        &app,
        BufferContentChangedEvent {
            path: request.path.clone(),
            version_id: response.version_id,
            changes: buffer_changes,
            is_undoing: true,
            is_redoing: false,
        },
    );
    Ok(UndoRedoJsonResponse {
        success: response.success,
        version_id: response.version_id,
        changes,
    })
}

#[tauri::command]
fn redo(state: State<MonacoHostState>, payload: Vec<u8>) -> Result<Vec<u8>, String> {
    let request = decode_message::<host::RedoRequest>(payload)?;
    let response = host_handlers::redo(&state, request)?;
    encode_message(response)
}

#[tauri::command]
fn redo_json(
    app: tauri::AppHandle,
    state: State<MonacoHostState>,
    request: UndoRedoJsonRequest,
) -> Result<UndoRedoJsonResponse, String> {
    let response = host_handlers::redo(&state, host::RedoRequest {
        resource: Some(file_uri_from_path(&request.path)),
    })?;
    let changes: Vec<EditChangeJson> = response
        .changes
        .iter()
        .map(|c| EditChangeJson {
            start_line: c.start_line,
            start_column: c.start_column,
            end_line: c.end_line,
            end_column: c.end_column,
            range_offset: c.range_offset,
            range_length: c.range_length,
            text: String::from_utf8_lossy(&c.text_utf8).to_string(),
        })
        .collect();
    let buffer_changes: Vec<BufferChangeEvent> = response
        .changes
        .iter()
        .map(|c| BufferChangeEvent {
            start_line: c.start_line,
            start_column: c.start_column,
            end_line: c.end_line,
            end_column: c.end_column,
            range_offset: c.range_offset,
            range_length: c.range_length,
            text: String::from_utf8_lossy(&c.text_utf8).to_string(),
        })
        .collect();
    EventBroadcaster::emit_buffer_content_changed(
        &app,
        BufferContentChangedEvent {
            path: request.path.clone(),
            version_id: response.version_id,
            changes: buffer_changes,
            is_undoing: false,
            is_redoing: true,
        },
    );
    Ok(UndoRedoJsonResponse {
        success: response.success,
        version_id: response.version_id,
        changes,
    })
}

#[tauri::command]
fn tokenize_document_json(
    state: State<MonacoHostState>,
    request: TokenizeDocumentJsonRequest,
) -> Result<TokenizeDocumentJsonResponse, String> {
    let registry = state.buffer_registry().read().map_err(|e| e.to_string())?;
    let response = monaco_tauri::syntax_handlers::tokenize_document(
        &registry,
        monaco_tauri::proto::code::ipc::editor::language::TokenizationRequest {
            resource: Some(file_uri_from_path(&request.path)),
            version_id: 0,
        },
    )?;
    let tokens = response
        .tokens
        .into_iter()
        .map(|t| SyntaxTokenJson {
            token_type: t.token_type,
            start_line: t.start_line,
            start_column: t.start_column,
            end_line: t.end_line,
            end_column: t.end_column,
            text: t.text,
        })
        .collect();
    Ok(TokenizeDocumentJsonResponse {
        version_id: response.version_id,
        tokens,
        has_errors: response.has_errors,
    })
}

#[tauri::command]
fn tokenize_document_range_json(
    state: State<MonacoHostState>,
    request: TokenizeRangeJsonRequest,
) -> Result<TokenizeDocumentJsonResponse, String> {
    let registry = state.buffer_registry().read().map_err(|e| e.to_string())?;
    let response = monaco_tauri::syntax_handlers::tokenize_document_range(
        &registry,
        monaco_tauri::proto::code::ipc::editor::language::TokenizationRequest {
            resource: Some(file_uri_from_path(&request.path)),
            version_id: 0,
        },
        request.start_line,
        request.end_line,
    )?;
    let tokens = response
        .tokens
        .into_iter()
        .map(|t| SyntaxTokenJson {
            token_type: t.token_type,
            start_line: t.start_line,
            start_column: t.start_column,
            end_line: t.end_line,
            end_column: t.end_column,
            text: t.text,
        })
        .collect();
    Ok(TokenizeDocumentJsonResponse {
        version_id: response.version_id,
        tokens,
        has_errors: response.has_errors,
    })
}

#[tauri::command]
fn fold_document_json(
    state: State<MonacoHostState>,
    request: FoldingRangeJsonRequest,
) -> Result<FoldingRangeJsonResponse, String> {
    let registry = state.buffer_registry().read().map_err(|e| e.to_string())?;
    let response = monaco_tauri::syntax_handlers::fold_document(
        &registry,
        monaco_tauri::proto::code::ipc::editor::language::FoldingRangeRequest {
            resource: Some(file_uri_from_path(&request.path)),
            version_id: 0,
        },
    )?;
    let ranges = response
        .ranges
        .into_iter()
        .map(|r| FoldingRangeJson {
            start_line: r.start_line,
            end_line: r.end_line,
            kind: r.kind,
        })
        .collect();
    Ok(FoldingRangeJsonResponse {
        version_id: response.version_id,
        ranges,
    })
}

#[tauri::command]
fn semantic_tokens_document_json(
    state: State<MonacoHostState>,
    request: SemanticTokensJsonRequest,
) -> Result<SemanticTokensJsonResponse, String> {
    let registry = state.buffer_registry().read().map_err(|e| e.to_string())?;
    let response = monaco_tauri::syntax_handlers::semantic_tokens_document(
        &registry,
        monaco_tauri::proto::code::ipc::editor::language::SemanticTokensRequest {
            resource: Some(file_uri_from_path(&request.path)),
            version_id: 0,
        },
    )?;
    Ok(SemanticTokensJsonResponse {
        version_id: response.version_id,
        data: response.data,
    })
}

#[tauri::command]
fn semantic_tokens_delta_document_json(
    state: State<MonacoHostState>,
    request: SemanticTokensDeltaJsonRequest,
) -> Result<SemanticTokensDeltaJsonResponse, String> {
    let registry = state.buffer_registry().read().map_err(|e| e.to_string())?;
    let response = monaco_tauri::syntax_handlers::semantic_tokens_delta_document(
        &registry,
        monaco_tauri::proto::code::ipc::editor::language::SemanticTokensDeltaRequest {
            resource: Some(file_uri_from_path(&request.path)),
            version_id: 0,
            previous_result_id: request.previous_result_id,
        },
    )?;
    Ok(SemanticTokensDeltaJsonResponse {
        version_id: response.version_id,
        result_id: response.result_id,
        data: response.data,
        removed: response.removed,
    })
}

#[tauri::command]
fn hover_document_json(
    state: State<MonacoHostState>,
    request: HoverJsonRequest,
) -> Result<HoverJsonResponse, String> {
    let registry = state.buffer_registry().read().map_err(|e| e.to_string())?;
    let response = monaco_tauri::syntax_handlers::hover_document(
        &registry,
        monaco_tauri::proto::code::ipc::editor::language::HoverRequest {
            resource: Some(file_uri_from_path(&request.path)),
            line: request.line,
            column: request.column,
            version_id: 0,
        },
    )?;
    Ok(HoverJsonResponse {
        contents: String::from_utf8_lossy(&response.contents_utf8).to_string(),
        start_line: response.start_line,
        start_column: response.start_column,
        end_line: response.end_line,
        end_column: response.end_column,
    })
}

#[tauri::command]
fn diagnostics_document_json(
    state: State<MonacoHostState>,
    request: DiagnosticJsonRequest,
) -> Result<DiagnosticJsonResponse, String> {
    let registry = state.buffer_registry().read().map_err(|e| e.to_string())?;
    let response = monaco_tauri::syntax_handlers::diagnostics_document(
        &registry,
        monaco_tauri::proto::code::ipc::editor::language::DiagnosticRequest {
            resource: Some(file_uri_from_path(&request.path)),
            version_id: 0,
        },
    )?;
    let diagnostics = response
        .diagnostics
        .into_iter()
        .map(|d| DiagnosticJson {
            start_line: d.start_line,
            start_column: d.start_column,
            end_line: d.end_line,
            end_column: d.end_column,
            message: d.message,
            severity: d.severity,
            code: d.code,
            source: d.source,
        })
        .collect();
    Ok(DiagnosticJsonResponse {
        version_id: response.version_id,
        diagnostics,
    })
}

#[tauri::command]
fn document_symbols_document_json(
    state: State<MonacoHostState>,
    request: DocumentSymbolJsonRequest,
) -> Result<DocumentSymbolJsonResponse, String> {
    let registry = state.buffer_registry().read().map_err(|e| e.to_string())?;
    let response = monaco_tauri::syntax_handlers::document_symbols_document(
        &registry,
        monaco_tauri::proto::code::ipc::editor::language::DocumentSymbolRequest {
            resource: Some(file_uri_from_path(&request.path)),
            version_id: 0,
        },
    )?;

    fn convert(s: monaco_tauri::proto::code::ipc::editor::language::DocumentSymbol) -> DocumentSymbolJson {
        DocumentSymbolJson {
            name: s.name,
            detail: s.detail,
            kind: s.kind,
            start_line: s.start_line,
            start_column: s.start_column,
            end_line: s.end_line,
            end_column: s.end_column,
            children: s.children.into_iter().map(convert).collect(),
        }
    }

    let symbols = response.symbols.into_iter().map(convert).collect();
    Ok(DocumentSymbolJsonResponse {
        version_id: response.version_id,
        symbols,
    })
}

// --- Security management commands ---

#[derive(Serialize)]
struct AuditLogEntryJson {
    timestamp_ms: u64,
    principal_id: String,
    action: String,
    resource: String,
    allowed: bool,
    reason: String,
}

#[derive(Serialize)]
struct AuditLogJsonResponse {
    entries: Vec<AuditLogEntryJson>,
}

#[tauri::command]
fn get_audit_log(state: State<MonacoHostState>) -> AuditLogJsonResponse {
    let entries = state
        .capabilities()
        .audit_log()
        .entries()
        .into_iter()
        .map(|e| AuditLogEntryJson {
            timestamp_ms: e.timestamp_ms,
            principal_id: e.principal_id,
            action: e.action,
            resource: e.resource,
            allowed: e.allowed,
            reason: e.reason,
        })
        .collect();
    AuditLogJsonResponse { entries }
}

#[derive(Serialize)]
struct SandboxSummaryJsonResponse {
    principals: Vec<SandboxPrincipalJson>,
    total_violations: u64,
}

#[derive(Serialize)]
struct SandboxPrincipalJson {
    id: String,
    open_buffers: usize,
    memory_bytes: usize,
    violations: u64,
}

#[tauri::command]
fn get_sandbox_summary(state: State<MonacoHostState>) -> SandboxSummaryJsonResponse {
    let summary = state.sandbox().usage_summary();
    let principals: Vec<SandboxPrincipalJson> = summary
        .into_iter()
        .map(|(id, (open_buffers, memory_bytes, violations))| SandboxPrincipalJson {
            id,
            open_buffers,
            memory_bytes,
            violations,
        })
        .collect();
    SandboxSummaryJsonResponse {
        principals,
        total_violations: state.sandbox().total_violations(),
    }
}

#[derive(Deserialize)]
struct GrantPermissionJsonRequest {
    principal_id: String,
    permission: String,
}

#[tauri::command]
fn grant_permission(
    state: State<MonacoHostState>,
    request: GrantPermissionJsonRequest,
) -> Result<(), String> {
    let perm = match request.permission.as_str() {
        "read_file" => monaco_tauri::security::Permission::ReadFile,
        "write_file" => monaco_tauri::security::Permission::WriteFile,
        "create_file" => monaco_tauri::security::Permission::CreateFile,
        "delete_file" => monaco_tauri::security::Permission::DeleteFile,
        "execute_command" => monaco_tauri::security::Permission::ExecuteCommand,
        "network_access" => monaco_tauri::security::Permission::NetworkAccess,
        "list_directory" => monaco_tauri::security::Permission::ListDirectory,
        "full_access" => monaco_tauri::security::Permission::FullAccess,
        _ => return Err(format!("unknown permission: {}", request.permission)),
    };
    state.capabilities().grant(&request.principal_id, perm);
    Ok(())
}

#[derive(Serialize)]
struct PermissionsJsonResponse {
    permissions: Vec<String>,
}

#[tauri::command]
fn get_permissions(
    state: State<MonacoHostState>,
    principal_id: String,
) -> PermissionsJsonResponse {
    let perms = state
        .capabilities()
        .permissions_for(&principal_id)
        .into_iter()
        .map(|p| p.name().to_string())
        .collect();
    PermissionsJsonResponse { permissions: perms }
}

// --- MCP Agent commands ---

#[derive(Deserialize)]
struct ExecuteMcpToolRequest {
    tool_name: String,
    arguments: String, // JSON string
}

#[derive(Serialize)]
struct ExecuteMcpToolResponse {
    success: bool,
    content: String,
    version_id: u64,
    error: Option<String>,
}

#[tauri::command]
fn execute_mcp_tool(
    state: State<MonacoHostState>,
    request: ExecuteMcpToolRequest,
) -> Result<ExecuteMcpToolResponse, String> {
    let registry = state.buffer_registry().read().map_err(|e| e.to_string())?;
    let tool_registry = monaco_tauri::mcp::McpToolRegistry::with_defaults();

    let tool = tool_registry
        .get(&request.tool_name)
        .ok_or_else(|| format!("Unknown MCP tool: {}", request.tool_name))?;

    let result = tool.execute(&registry, &request.arguments);
    Ok(ExecuteMcpToolResponse {
        success: result.success,
        content: result.content,
        version_id: result.version_id,
        error: result.error,
    })
}

#[derive(Serialize)]
struct McpToolInfoJson {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Serialize)]
struct ListMcpToolsResponse {
    tools: Vec<McpToolInfoJson>,
}

#[tauri::command]
fn list_mcp_tools() -> ListMcpToolsResponse {
    let tool_registry = monaco_tauri::mcp::McpToolRegistry::with_defaults();
    let tools = tool_registry
        .list_tools()
        .into_iter()
        .map(|t| McpToolInfoJson {
            name: t.name().to_string(),
            description: t.description().to_string(),
            input_schema: t.input_schema(),
        })
        .collect();
    ListMcpToolsResponse { tools }
}

#[tauri::command]
fn completion_document_json(
    state: State<MonacoHostState>,
    request: CompletionJsonRequest,
) -> Result<CompletionJsonResponse, String> {
    let registry = state.buffer_registry().read().map_err(|e| e.to_string())?;
    let response = monaco_tauri::syntax_handlers::completion_document(
        &registry,
        monaco_tauri::proto::code::ipc::editor::language::CompletionRequest {
            resource: Some(file_uri_from_path(&request.path)),
            line: request.line,
            column: request.column,
            version_id: 0,
            trigger_character: request.trigger_character,
        },
    )?;
    let items = response
        .items
        .into_iter()
        .map(|i| CompletionItemJson {
            label: i.label,
            kind: i.kind,
            detail: i.detail,
            documentation: String::from_utf8_lossy(&i.documentation_utf8).to_string(),
            insert_text: i.insert_text,
            sort_text: i.sort_text,
            filter_text: i.filter_text,
        })
        .collect();
    Ok(CompletionJsonResponse {
        items,
        is_incomplete: response.is_incomplete,
        version_id: response.version_id,
    })
}

fn main() {
    tauri::Builder::default()
        .manage(MonacoHostState::new())
        .invoke_handler(tauri::generate_handler![
            workspace_roots_json,
            list_directory_json,
            open_document_json,
            save_document_json,
            get_workspace_roots,
            set_primary_workspace_root,
            open_document,
            save_document,
            save_document_as,
            list_directory,
            close_document,
            close_document_json,
            apply_edits,
            apply_edits_json,
            get_buffer_snapshot,
            undo,
            undo_json,
            redo,
            redo_json,
            tokenize_document_json,
            tokenize_document_range_json,
            fold_document_json,
            semantic_tokens_document_json,
            semantic_tokens_delta_document_json,
            hover_document_json,
            diagnostics_document_json,
            document_symbols_document_json,
            completion_document_json,
            get_audit_log,
            get_sandbox_summary,
            grant_permission,
            get_permissions,
            execute_mcp_tool,
            list_mcp_tools
        ])
        .run(tauri::generate_context!())
        .expect("error while running Monaco Tauri prototype");
}
