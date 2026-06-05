#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use monaco_tauri::events::{
    BufferChangeEvent, BufferClosedEvent, BufferContentChangedEvent, BufferOpenedEvent,
    BufferSavedEvent, EventBroadcaster,
};
use monaco_tauri::host_handlers;
use monaco_tauri::host_handlers::{build_lsp_changes_from_proto, MonacoHostState};
use monaco_tauri::proto::code::ipc::editor::host;
use monaco_tauri::proto::code::ipc::file;
use monaco_tauri::wasm_sync;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{Emitter, Manager, State};

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
struct SaveDocumentAsJsonRequest {
    source_path: String,
    target_path: String,
    content: String,
    version_id: u64,
    overwrite: bool,
}

#[derive(Serialize)]
struct SaveDocumentAsJsonResponse {
    path: String,
    version_id: u64,
    content: String,
    language_id: String,
    eol: String,
    is_dirty: bool,
    source_closed: bool,
}

#[derive(Deserialize)]
struct GetBufferSnapshotJsonRequest {
    path: String,
    at_version_id: u64,
}

#[derive(Serialize)]
struct GetBufferSnapshotJsonResponse {
    path: String,
    version_id: u64,
    content: String,
    eol: String,
    is_dirty: bool,
}

#[derive(Deserialize)]
struct SetPrimaryWorkspaceRootJsonRequest {
    path: String,
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

#[derive(Deserialize)]
struct CodeActionJsonRequest {
    path: String,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
    kind_filter: String,
}

#[derive(Serialize)]
struct CodeActionJson {
    title: String,
    kind: String,
    is_preferred: bool,
}

#[derive(Serialize)]
struct CodeActionJsonResponse {
    version_id: u64,
    actions: Vec<CodeActionJson>,
}

#[derive(Deserialize)]
struct WasmSyncStateJsonRequest {
    path: String,
    previous_content: Option<String>,
    previous_version_id: Option<u64>,
}

#[derive(Serialize)]
struct WasmSyncSnapshotJson {
    path: String,
    content: String,
    version_id: u64,
    line_count: usize,
}

#[derive(Serialize)]
struct WasmSyncDeltaJson {
    start_line: usize,
    end_line: usize,
    text: String,
    version_id: u64,
}

#[derive(Serialize)]
struct WasmSyncStateJsonResponse {
    mode: String,
    snapshot: Option<WasmSyncSnapshotJson>,
    delta: Option<WasmSyncDeltaJson>,
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
fn workspace_roots(state: State<MonacoHostState>) -> Result<Vec<WorkspaceRootJson>, String> {
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
fn list_directory(
    state: State<MonacoHostState>,
    path: String,
) -> Result<Vec<DirectoryEntryJson>, String> {
    // Security: check list directory permission
    let principal = monaco_tauri::security::Principal {
        id: "user".to_string(),
        kind: monaco_tauri::security::PrincipalKind::User,
    };
    if !state.capabilities().check(
        &principal,
        monaco_tauri::security::Permission::ListDirectory,
        &path,
    ) {
        return Err("permission denied: list_directory".to_string());
    }

    let response = host_handlers::list_directory(
        &state,
        host::ListDirectoryRequest {
            resource: Some(file_uri_from_path(&path)),
            include_file_stats: true,
        },
    )?;

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
                is_directory: stat
                    .as_ref()
                    .map(|value| value.is_directory)
                    .unwrap_or(false),
                size: stat.as_ref().map(|value| value.size).unwrap_or_default(),
            }
        })
        .collect())
}

#[tauri::command]
fn open_document(
    app: tauri::AppHandle,
    state: State<MonacoHostState>,
    path: String,
) -> Result<OpenDocumentJson, String> {
    let response = host_handlers::open_document(
        &state,
        host::OpenDocumentRequest {
            resource: Some(file_uri_from_path(&path)),
            create_if_missing: false,
            preferred_language_id: String::new(),
        },
    )?;
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
        content: String::from_utf8((&*snapshot.content_utf8).to_vec()).map_err(|err| err.to_string())?,
        version_id: snapshot.version_id,
    })
}

#[tauri::command]
fn save_document(
    app: tauri::AppHandle,
    state: State<MonacoHostState>,
    request: SaveDocumentJsonRequest,
) -> Result<u64, String> {
    let response = host_handlers::save_document(
        &state,
        host::SaveDocumentRequest {
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
        },
    )?;
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
fn set_primary_workspace_root(
    state: State<MonacoHostState>,
    request: SetPrimaryWorkspaceRootJsonRequest,
) -> Result<WorkspaceRootJson, String> {
    let response = host_handlers::set_primary_workspace_root(
        &state,
        host::SetPrimaryWorkspaceRootRequest {
            resource: Some(file_uri_from_path(&request.path)),
        },
    )?;
    let root = response.root.unwrap_or_default();
    Ok(WorkspaceRootJson {
        path: root
            .resource
            .as_ref()
            .map(|r| r.path.clone())
            .unwrap_or_default(),
        name: root.name,
        is_primary: root.is_primary,
    })
}

#[tauri::command]
fn save_document_as(
    app: tauri::AppHandle,
    state: State<MonacoHostState>,
    request: SaveDocumentAsJsonRequest,
) -> Result<SaveDocumentAsJsonResponse, String> {
    let response = host_handlers::save_document_as(
        &state,
        host::SaveDocumentAsRequest {
            snapshot: Some(monaco_tauri::proto::code::ipc::editor::BufferSnapshot {
                resource: Some(file_uri_from_path(&request.source_path)),
                version_id: request.version_id,
                content_utf8: request.content.into_bytes(),
                eol: "\n".to_string(),
                is_dirty: true,
            }),
            target: Some(file_uri_from_path(&request.target_path)),
            overwrite: request.overwrite,
        },
    )?;
    let snapshot = response
        .snapshot
        .ok_or_else(|| "missing snapshot in save-as response".to_string())?;
    EventBroadcaster::emit_buffer_saved(
        &app,
        BufferSavedEvent {
            path: request.target_path.clone(),
            version_id: snapshot.version_id,
        },
    );
    Ok(SaveDocumentAsJsonResponse {
        path: snapshot
            .resource
            .as_ref()
            .map(|r| r.path.clone())
            .unwrap_or_default(),
        version_id: snapshot.version_id,
        content: String::from_utf8((&*snapshot.content_utf8).to_vec())
            .map_err(|err| err.to_string())?,
        language_id: String::new(),
        eol: snapshot.eol,
        is_dirty: snapshot.is_dirty,
        source_closed: response.source_closed,
    })
}

#[tauri::command]
fn close_document(
    app: tauri::AppHandle,
    state: State<MonacoHostState>,
    request: CloseDocumentJsonRequest,
) -> Result<CloseDocumentJsonResponse, String> {
    let response = host_handlers::close_document(
        &state,
        host::CloseDocumentRequest {
            resource: Some(file_uri_from_path(&request.path)),
            save_if_dirty: request.save_if_dirty,
        },
    )?;
    EventBroadcaster::emit_buffer_closed(
        &app,
        BufferClosedEvent {
            path: request.path.clone(),
        },
    );
    state.buffer_registry().read().push_event(
        monaco_tauri::buffer::BufferEvent {
            event_type: "closed".to_string(),
            resource: request.path.clone(),
            version_id: None,
            data: None,
        },
    );
    Ok(CloseDocumentJsonResponse {
        closed: response.closed,
        was_dirty: response.was_dirty,
    })
}

#[derive(Deserialize)]
struct CloseBuffersBatchJsonRequest {
    paths: Vec<String>,
}

#[derive(Serialize)]
struct CloseBuffersBatchJsonResponse {
    results: Vec<CloseDocumentJsonResponse>,
}

#[tauri::command]
fn close_buffers_batch(
    app: tauri::AppHandle,
    state: State<MonacoHostState>,
    request: CloseBuffersBatchJsonRequest,
) -> Result<CloseBuffersBatchJsonResponse, String> {
    let mut registry = state.buffer_registry().write();
    let mut results = Vec::new();
    for path in &request.paths {
        let was_dirty = registry.is_buffer_dirty(path).unwrap_or(false);
        let closed = registry.close_buffer(path);
        if closed {
            EventBroadcaster::emit_buffer_closed(
                &app,
                BufferClosedEvent { path: path.clone() },
            );
            registry.push_event(
                monaco_tauri::buffer::BufferEvent {
                    event_type: "closed".to_string(),
                    resource: path.clone(),
                    version_id: None,
                    data: None,
                },
            );
        }
        results.push(CloseDocumentJsonResponse { closed, was_dirty });
    }
    Ok(CloseBuffersBatchJsonResponse { results })
}

#[tauri::command]
fn apply_edits(
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
        .map(
            |edit| monaco_tauri::proto::code::ipc::editor::ModelContentChange {
                start_line: edit.start_line,
                start_column: edit.start_column,
                end_line: edit.end_line,
                end_column: edit.end_column,
                range_offset: edit.range_offset,
                range_length: edit.range_length,
                text_utf8: edit.text.into_bytes(),
            },
        )
        .collect();
    let response = host_handlers::apply_edits(
        &state,
        host::ApplyEditsRequest {
            resource: Some(file_uri_from_path(&request.path)),
            edits,
        },
    )?;

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

    // Debounce diagnostics: cancel in-flight task for this buffer, spawn new one
    let app_clone = app.clone();
    let path_clone = request.path.clone();
    let handle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(150)).await;
        let state = app_clone.state::<MonacoHostState>();
        let mut lsp = state.lsp_registry().write();
        let registry = state.buffer_registry().read();
        if let Ok(diag_response) = monaco_tauri::syntax_handlers::diagnostics_document(
            &registry,
            Some(&mut lsp),
            monaco_tauri::proto::code::ipc::editor::language::DiagnosticRequest {
                resource: Some(file_uri_from_path(&path_clone)),
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
            let _ = app_clone.emit(
                "diagnostics-changed",
                DiagnosticsChangedEvent {
                    path: path_clone.clone(),
                    version_id: diag_response.version_id,
                    diagnostics,
                },
            );
        }
    });
    state.replace_diagnostic_task(&request.path, handle);

    // Batch LSP didChange: accumulate with any rapid subsequent edits
    if response.new_version_id > 0 {
        let lsp_changes = response
            .applied_event
            .map(|e| build_lsp_changes_from_proto(&e.changes))
            .unwrap_or_default();
        state.queue_did_change(
            &request.path,
            response.new_version_id as i32,
            lsp_changes,
        );
    }

    Ok(ApplyEditsJsonResponse {
        success: response.success,
        new_version_id: response.new_version_id,
    })
}

#[tauri::command]
fn get_buffer_snapshot(
    state: State<MonacoHostState>,
    request: GetBufferSnapshotJsonRequest,
) -> Result<GetBufferSnapshotJsonResponse, String> {
    let response = host_handlers::get_buffer_snapshot(
        &state,
        host::GetBufferSnapshotRequest {
            resource: Some(file_uri_from_path(&request.path)),
            at_version_id: request.at_version_id,
        },
    )?;
    let snapshot = response
        .snapshot
        .ok_or_else(|| "missing snapshot".to_string())?;
    Ok(GetBufferSnapshotJsonResponse {
        path: snapshot
            .resource
            .as_ref()
            .map(|r| r.path.clone())
            .unwrap_or_default(),
        version_id: snapshot.version_id,
        content: String::from_utf8((&*snapshot.content_utf8).to_vec())
            .map_err(|err| err.to_string())?,
        eol: snapshot.eol,
        is_dirty: snapshot.is_dirty,
    })
}

#[tauri::command]
fn undo(
    app: tauri::AppHandle,
    state: State<MonacoHostState>,
    request: UndoRedoJsonRequest,
) -> Result<UndoRedoJsonResponse, String> {
    let response = host_handlers::undo(
        &state,
        host::UndoRequest {
            resource: Some(file_uri_from_path(&request.path)),
        },
    )?;
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

    // Batch LSP didChange with any rapid subsequent edits
    if response.success {
        let lsp_changes = build_lsp_changes_from_proto(&response.changes);
        state.queue_did_change(&request.path, response.version_id as i32, lsp_changes);
    }

    Ok(UndoRedoJsonResponse {
        success: response.success,
        version_id: response.version_id,
        changes,
    })
}

#[tauri::command]
fn redo(
    app: tauri::AppHandle,
    state: State<MonacoHostState>,
    request: UndoRedoJsonRequest,
) -> Result<UndoRedoJsonResponse, String> {
    let response = host_handlers::redo(
        &state,
        host::RedoRequest {
            resource: Some(file_uri_from_path(&request.path)),
        },
    )?;
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

    // Batch LSP didChange with any rapid subsequent edits
    if response.success {
        let lsp_changes = build_lsp_changes_from_proto(&response.changes);
        state.queue_did_change(&request.path, response.version_id as i32, lsp_changes);
    }

    Ok(UndoRedoJsonResponse {
        success: response.success,
        version_id: response.version_id,
        changes,
    })
}

#[tauri::command]
fn tokenize_document(
    state: State<MonacoHostState>,
    request: TokenizeDocumentJsonRequest,
) -> Result<TokenizeDocumentJsonResponse, String> {
    let registry = state.buffer_registry().read();
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
fn tokenize_document_range(
    state: State<MonacoHostState>,
    request: TokenizeRangeJsonRequest,
) -> Result<TokenizeDocumentJsonResponse, String> {
    let registry = state.buffer_registry().read();
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
fn fold_document(
    state: State<MonacoHostState>,
    request: FoldingRangeJsonRequest,
) -> Result<FoldingRangeJsonResponse, String> {
    let registry = state.buffer_registry().read();
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
fn semantic_tokens_document(
    state: State<MonacoHostState>,
    request: SemanticTokensJsonRequest,
) -> Result<SemanticTokensJsonResponse, String> {
    let registry = state.buffer_registry().read();
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
fn semantic_tokens_delta_document(
    state: State<MonacoHostState>,
    request: SemanticTokensDeltaJsonRequest,
) -> Result<SemanticTokensDeltaJsonResponse, String> {
    let registry = state.buffer_registry().read();
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
fn hover_document(
    state: State<MonacoHostState>,
    request: HoverJsonRequest,
) -> Result<HoverJsonResponse, String> {
    let registry = state.buffer_registry().read();
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
fn diagnostics_document(
    state: State<MonacoHostState>,
    request: DiagnosticJsonRequest,
) -> Result<DiagnosticJsonResponse, String> {
    let registry = state.buffer_registry().read();
    let mut lsp = state.lsp_registry().write();
    let response = monaco_tauri::syntax_handlers::diagnostics_document(
        &registry,
        Some(&mut lsp),
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
fn document_symbols_document(
    state: State<MonacoHostState>,
    request: DocumentSymbolJsonRequest,
) -> Result<DocumentSymbolJsonResponse, String> {
    let registry = state.buffer_registry().read();
    let response = monaco_tauri::syntax_handlers::document_symbols_document(
        &registry,
        monaco_tauri::proto::code::ipc::editor::language::DocumentSymbolRequest {
            resource: Some(file_uri_from_path(&request.path)),
            version_id: 0,
        },
    )?;

    fn convert(
        s: monaco_tauri::proto::code::ipc::editor::language::DocumentSymbol,
    ) -> DocumentSymbolJson {
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
        .map(
            |(id, (open_buffers, memory_bytes, violations))| SandboxPrincipalJson {
                id,
                open_buffers,
                memory_bytes,
                violations,
            },
        )
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
fn get_permissions(state: State<MonacoHostState>, principal_id: String) -> PermissionsJsonResponse {
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
    status: monaco_tauri::mcp::McpStatus,
    content: String,
    version_id: u64,
    error: Option<monaco_tauri::mcp::McpError>,
    certainty: Option<monaco_tauri::mcp::McpCertainty>,
    provenance: Option<monaco_tauri::mcp::McpResultProvenance>,
    evidence: Option<serde_json::Value>,
    data: Option<serde_json::Value>,
}

#[tauri::command]
fn execute_mcp_tool(
    state: State<MonacoHostState>,
    request: ExecuteMcpToolRequest,
) -> Result<ExecuteMcpToolResponse, String> {
    // Validate path containment for tools that accept a path argument
    if let Ok(args) = serde_json::from_str::<serde_json::Value>(&request.arguments) {
        if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
            host_handlers::validate_mcp_path(&state, path)?;
        }
    }

    let registry = state.buffer_registry().read();
    let tool_registry = state.mcp_tool_registry();

    let tool = tool_registry
        .get(&request.tool_name)
        .ok_or_else(|| format!("Unknown MCP tool: {}", request.tool_name))?;

    let result = tool.execute(&registry, &request.arguments);
    Ok(ExecuteMcpToolResponse {
        success: result.success,
        status: result.status,
        content: result.content,
        version_id: result.version_id,
        error: result.error,
        certainty: result.certainty,
        provenance: result.provenance,
        evidence: result.evidence,
        data: result.data,
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
fn list_mcp_tools(state: State<MonacoHostState>) -> ListMcpToolsResponse {
    let tool_registry = state.mcp_tool_registry();
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
fn completion_document(
    state: State<MonacoHostState>,
    request: CompletionJsonRequest,
) -> Result<CompletionJsonResponse, String> {
    let registry = state.buffer_registry().read();
    let mut lsp = state.lsp_registry().write();
    let response = monaco_tauri::syntax_handlers::completion_document(
        &registry,
        Some(&mut lsp),
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

#[tauri::command]
fn code_actions_document(
    state: State<MonacoHostState>,
    request: CodeActionJsonRequest,
) -> Result<CodeActionJsonResponse, String> {
    let registry = state.buffer_registry().read();
    let mut lsp = state.lsp_registry().write();
    let response = monaco_tauri::syntax_handlers::code_actions_document(
        &registry,
        Some(&mut lsp),
        monaco_tauri::proto::code::ipc::editor::language::CodeActionRequest {
            resource: Some(file_uri_from_path(&request.path)),
            version_id: 0,
            start_line: request.start_line,
            start_column: request.start_column,
            end_line: request.end_line,
            end_column: request.end_column,
            kind_filter: request.kind_filter,
        },
    )?;
    let actions = response
        .actions
        .into_iter()
        .map(|a| CodeActionJson {
            title: a.title,
            kind: a.kind,
            is_preferred: a.is_preferred,
        })
        .collect();
    Ok(CodeActionJsonResponse {
        version_id: response.version_id,
        actions,
    })
}

#[tauri::command]
fn wasm_sync_state(
    state: State<MonacoHostState>,
    request: WasmSyncStateJsonRequest,
) -> Result<WasmSyncStateJsonResponse, String> {
    let registry = state.buffer_registry().read();

    match (&request.previous_content, request.previous_version_id) {
        (Some(previous_content), Some(previous_version_id)) => {
            let delta = wasm_sync::compute_line_delta(
                &registry,
                &request.path,
                previous_content,
                previous_version_id,
            )?;

            if let Some(delta) = delta {
                return Ok(WasmSyncStateJsonResponse {
                    mode: "delta".to_string(),
                    snapshot: None,
                    delta: Some(WasmSyncDeltaJson {
                        start_line: delta.start_line,
                        end_line: delta.end_line,
                        text: delta.text,
                        version_id: delta.version_id,
                    }),
                });
            }

            Ok(WasmSyncStateJsonResponse {
                mode: "noop".to_string(),
                snapshot: None,
                delta: None,
            })
        }
        _ => {
            let snapshot = wasm_sync::build_snapshot(&registry, &request.path)?
                .ok_or_else(|| format!("Buffer not found: {}", request.path))?;

            Ok(WasmSyncStateJsonResponse {
                mode: "snapshot".to_string(),
                snapshot: Some(WasmSyncSnapshotJson {
                    path: snapshot.resource,
                    content: snapshot.content,
                    version_id: snapshot.version_id,
                    line_count: snapshot.line_count,
                }),
                delta: None,
            })
        }
    }
}

#[tauri::command]
fn wasm_sync_state_binary(
    state: State<MonacoHostState>,
    request: WasmSyncStateJsonRequest,
) -> Result<Vec<u8>, String> {
    let registry = state.buffer_registry().read();

    match (&request.previous_content, request.previous_version_id) {
        (Some(previous_content), Some(previous_version_id)) => {
            let delta = wasm_sync::compute_line_delta(
                &registry,
                &request.path,
                previous_content,
                previous_version_id,
            )?;
            if let Some(delta) = delta {
                Ok(delta.to_binary())
            } else {
                Ok(wasm_sync::noop_binary())
            }
        }
        _ => {
            let snapshot = wasm_sync::build_snapshot(&registry, &request.path)?
                .ok_or_else(|| format!("Buffer not found: {}", request.path))?;
            Ok(snapshot.to_binary())
        }
    }
}

fn main() {
    tauri::Builder::default()
        .manage(MonacoHostState::new())
        .setup(|app| {
            app.state::<MonacoHostState>()
                .set_app_handle(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            workspace_roots,
            list_directory,
            open_document,
            save_document,
            set_primary_workspace_root,
            save_document_as,
            close_document,
            close_buffers_batch,
            apply_edits,
            get_buffer_snapshot,
            undo,
            redo,
            tokenize_document,
            tokenize_document_range,
            fold_document,
            semantic_tokens_document,
            semantic_tokens_delta_document,
            hover_document,
            diagnostics_document,
            document_symbols_document,
            completion_document,
            code_actions_document,
            wasm_sync_state,
            wasm_sync_state_binary,
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
