use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::buffer::{BufferRegistry, ContentChange, Position};
use crate::lsp::LspRegistry;
use crate::proto::code::ipc::editor;
use crate::proto::code::ipc::editor::host;
use crate::proto::code::ipc::file;
use crate::security::{
    CapabilityRegistry, Permission, Principal, PrincipalKind, SandboxLimits, SecuritySandbox,
};

/// Default principal for built-in editor operations (the user).
const DEFAULT_PRINCIPAL: &str = "user";

pub struct MonacoHostState {
    workspace_roots: RwLock<Vec<host::WorkspaceRoot>>,
    buffer_registry: RwLock<BufferRegistry>,
    sandbox: Arc<SecuritySandbox>,
    capabilities: Arc<CapabilityRegistry>,
    lsp_registry: RwLock<LspRegistry>,
}

impl Default for MonacoHostState {
    fn default() -> Self {
        Self::new()
    }
}

impl MonacoHostState {
    pub fn new() -> Self {
        let current_dir = std::env::current_dir().ok().map(|dir| {
            if dir.file_name().and_then(|value| value.to_str()) == Some("src-tauri") {
                dir.parent().map(PathBuf::from).unwrap_or(dir)
            } else {
                dir
            }
        });
        let roots = current_dir
            .map(|dir| {
                vec![host::WorkspaceRoot {
                    resource: Some(path_to_uri(&dir)),
                    name: dir
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("workspace")
                        .to_string(),
                    is_primary: true,
                }]
            })
            .unwrap_or_default();

        let sandbox = Arc::new(SecuritySandbox::new(SandboxLimits::default()));
        let capabilities = Arc::new(CapabilityRegistry::default());
        // Grant default user principal full file access for built-in editor operations
        capabilities.grant_many(
            DEFAULT_PRINCIPAL,
            &[
                Permission::ReadFile,
                Permission::WriteFile,
                Permission::CreateFile,
                Permission::DeleteFile,
                Permission::ListDirectory,
                Permission::ReadWorkspaceConfig,
                Permission::WriteWorkspaceConfig,
            ],
        );

        let workspace_root = roots
            .iter()
            .find(|r| r.is_primary)
            .and_then(|r| r.resource.as_ref())
            .map(|r| r.path.clone())
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .map(|d| d.to_string_lossy().to_string())
                    .unwrap_or_default()
            });

        Self {
            workspace_roots: RwLock::new(roots),
            buffer_registry: RwLock::new(BufferRegistry::new()),
            sandbox,
            capabilities,
            lsp_registry: RwLock::new(LspRegistry::new(workspace_root)),
        }
    }

    pub fn sandbox(&self) -> &Arc<SecuritySandbox> {
        &self.sandbox
    }

    pub fn capabilities(&self) -> &Arc<CapabilityRegistry> {
        &self.capabilities
    }

    pub fn workspace_roots(&self) -> Vec<host::WorkspaceRoot> {
        self.workspace_roots
            .read()
            .expect("workspace roots poisoned")
            .clone()
    }

    pub fn set_primary_root(&self, resource: &file::Uri) -> Result<host::WorkspaceRoot, String> {
        let mut roots = self
            .workspace_roots
            .write()
            .map_err(|_| "workspace roots poisoned".to_string())?;

        let mut selected: Option<host::WorkspaceRoot> = None;
        for root in roots.iter_mut() {
            let is_match = root.resource.as_ref() == Some(resource);
            root.is_primary = is_match;
            if is_match {
                selected = Some(root.clone());
            }
        }

        if let Some(root) = selected {
            return Ok(root);
        }

        let path = uri_to_path(resource)?;
        let new_root = host::WorkspaceRoot {
            resource: Some(resource.clone()),
            name: path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("workspace")
                .to_string(),
            is_primary: true,
        };
        for root in roots.iter_mut() {
            root.is_primary = false;
        }
        roots.push(new_root.clone());
        Ok(new_root)
    }

    pub fn buffer_registry(&self) -> &RwLock<BufferRegistry> {
        &self.buffer_registry
    }

    pub fn lsp_registry(&self) -> &RwLock<LspRegistry> {
        &self.lsp_registry
    }
}

pub fn get_workspace_roots(state: &MonacoHostState) -> host::GetWorkspaceRootsResponse {
    host::GetWorkspaceRootsResponse {
        roots: state.workspace_roots(),
    }
}

pub fn set_primary_workspace_root(
    state: &MonacoHostState,
    request: host::SetPrimaryWorkspaceRootRequest,
) -> Result<host::SetPrimaryWorkspaceRootResponse, String> {
    let resource = request
        .resource
        .ok_or_else(|| "missing workspace root resource".to_string())?;
    let root = state.set_primary_root(&resource)?;
    Ok(host::SetPrimaryWorkspaceRootResponse { root: Some(root) })
}

/// Helper to build the default principal.
fn default_principal() -> Principal {
    Principal {
        id: DEFAULT_PRINCIPAL.to_string(),
        kind: PrincipalKind::User,
    }
}

pub fn open_document(
    state: &MonacoHostState,
    request: host::OpenDocumentRequest,
) -> Result<host::OpenDocumentResponse, String> {
    let resource = request
        .resource
        .ok_or_else(|| "missing document resource".to_string())?;
    let path = uri_to_path(&resource)?;

    // Security: check read permission
    let principal = default_principal();
    if !state
        .capabilities
        .check(&principal, Permission::ReadFile, &resource.path)
    {
        return Err("permission denied: read_file".to_string());
    }

    // Check if buffer already exists in registry
    {
        let registry = state.buffer_registry.read().map_err(|e| e.to_string())?;
        if registry.has_buffer(&resource.path) {
            // Return existing buffer snapshot
            if let Some(snapshot) = registry.get_buffer_snapshot(&resource.path) {
                return Ok(host::OpenDocumentResponse {
                    snapshot: Some(editor::BufferSnapshot {
                        resource: Some(resource.clone()),
                        version_id: snapshot.version_id,
                        content_utf8: snapshot.content_utf8,
                        eol: snapshot.eol,
                        is_dirty: snapshot.is_dirty,
                    }),
                    stat: Some(build_file_stat(&path)?),
                    language_id: if request.preferred_language_id.is_empty() {
                        detect_language_id(&path)
                    } else {
                        request.preferred_language_id
                    },
                    created: false,
                });
            }
        }
    }

    // Read file from disk
    let (content, created) = match fs::read(&path) {
        Ok(bytes) => (bytes, false),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound && request.create_if_missing => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|err| err.to_string())?;
            }
            fs::write(&path, []).map_err(|err| err.to_string())?;
            (Vec::new(), true)
        }
        Err(err) => return Err(err.to_string()),
    };

    // Security: enforce sandbox buffer size limits
    state
        .sandbox
        .check_buffer_open(DEFAULT_PRINCIPAL, content.len())?;

    // Open buffer in registry
    {
        let mut registry = state.buffer_registry.write().map_err(|e| e.to_string())?;
        registry.open_buffer_from_bytes(resource.path.clone(), &content)?;
    }
    state
        .sandbox
        .record_buffer_opened(DEFAULT_PRINCIPAL, content.len());

    let stat = build_file_stat(&path)?;
    let language_id = if request.preferred_language_id.is_empty() {
        detect_language_id(&path)
    } else {
        request.preferred_language_id.clone()
    };

    // Notify LSP server that document was opened
    if let Ok(mut lsp) = state.lsp_registry.write() {
        let text = String::from_utf8_lossy(&content).to_string();
        lsp.did_open(&resource.path, &language_id, 1, &text);
    }

    Ok(host::OpenDocumentResponse {
        snapshot: Some(editor::BufferSnapshot {
            resource: Some(resource),
            version_id: 1,
            content_utf8: content,
            eol: detect_eol(&path),
            is_dirty: false,
        }),
        stat: Some(stat),
        language_id,
        created,
    })
}

pub fn save_document(
    state: &MonacoHostState,
    request: host::SaveDocumentRequest,
) -> Result<host::SaveDocumentResponse, String> {
    let snapshot = request
        .snapshot
        .ok_or_else(|| "missing document snapshot".to_string())?;
    let resource = snapshot
        .resource
        .clone()
        .ok_or_else(|| "missing document resource".to_string())?;
    let path = uri_to_path(&resource)?;

    // Security: check write/create permission
    let principal = default_principal();
    let perm = if path.exists() {
        Permission::WriteFile
    } else {
        Permission::CreateFile
    };
    if !state.capabilities.check(&principal, perm, &resource.path) {
        return Err("permission denied: write/create file".to_string());
    }

    if path.exists() && !request.overwrite {
        return Err(format!(
            "refusing to overwrite existing file: {}",
            path.display()
        ));
    }
    if !path.exists() && !request.create {
        return Err(format!(
            "refusing to create missing file: {}",
            path.display()
        ));
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(&path, &snapshot.content_utf8).map_err(|err| err.to_string())?;

    // Update buffer in registry if it exists
    {
        let registry = state.buffer_registry.read().map_err(|e| e.to_string())?;
        if registry.has_buffer(&resource.path) {
            // Update the buffer content to match the saved content
            let content = String::from_utf8(snapshot.content_utf8.clone())
                .map_err(|e| format!("Invalid UTF-8 content: {}", e))?;
            registry.set_buffer_content(&resource.path, &content);
            registry.mark_buffer_saved(&resource.path);
        }
    }

    Ok(host::SaveDocumentResponse {
        stat: Some(build_file_stat(&path)?),
        persisted_version_id: snapshot.version_id,
    })
}

pub fn save_document_as(
    state: &MonacoHostState,
    request: host::SaveDocumentAsRequest,
) -> Result<host::SaveDocumentAsResponse, String> {
    let snapshot = request
        .snapshot
        .ok_or_else(|| "missing document snapshot".to_string())?;
    let target = request
        .target
        .ok_or_else(|| "missing target resource".to_string())?;
    let target_path = uri_to_path(&target)?;

    if target_path.exists() && !request.overwrite {
        return Err(format!(
            "refusing to overwrite existing target: {}",
            target_path.display()
        ));
    }

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(&target_path, &snapshot.content_utf8).map_err(|err| err.to_string())?;

    // Update registry: close old buffer, open new one
    {
        let mut registry = state.buffer_registry.write().map_err(|e| e.to_string())?;

        // Get content before closing
        let content = snapshot.content_utf8.clone();

        // Close old buffer if exists
        if let Some(source_resource) = &snapshot.resource {
            registry.close_buffer(&source_resource.path);
        }

        // Open new buffer
        let content_str =
            String::from_utf8(content).map_err(|e| format!("Invalid UTF-8 content: {}", e))?;
        registry.open_buffer(target.path.clone(), &content_str);
    }

    Ok(host::SaveDocumentAsResponse {
        snapshot: Some(editor::BufferSnapshot {
            resource: Some(target.clone()),
            version_id: snapshot.version_id,
            content_utf8: snapshot.content_utf8,
            eol: snapshot.eol,
            is_dirty: false,
        }),
        stat: Some(build_file_stat(&target_path)?),
        source_closed: true,
    })
}

pub fn close_document(
    state: &MonacoHostState,
    request: host::CloseDocumentRequest,
) -> Result<host::CloseDocumentResponse, String> {
    let resource = request
        .resource
        .ok_or_else(|| "missing document resource".to_string())?;

    let registry = state.buffer_registry.read().map_err(|e| e.to_string())?;
    let is_dirty = registry.is_buffer_dirty(&resource.path).unwrap_or(false);
    drop(registry);

    // If save_if_dirty and buffer is dirty, save first
    if request.save_if_dirty && is_dirty {
        if let Some(snapshot) = state
            .buffer_registry
            .read()
            .map_err(|e| e.to_string())?
            .get_buffer_snapshot(&resource.path)
        {
            let path = uri_to_path(&resource)?;
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|err| err.to_string())?;
            }
            fs::write(&path, &snapshot.content_utf8).map_err(|err| err.to_string())?;
        }
    }

    // Notify LSP server that document was closed
    let language_id = detect_language_id_from_path(&resource.path);
    if let Ok(mut lsp) = state.lsp_registry.write() {
        lsp.did_close(&resource.path, &language_id);
    }

    // Close the buffer
    let mut registry = state.buffer_registry.write().map_err(|e| e.to_string())?;
    let size = registry
        .get_buffer_content(&resource.path)
        .map(|c| c.len())
        .unwrap_or(0);
    let closed = registry.close_buffer(&resource.path);
    state.sandbox.record_buffer_closed(DEFAULT_PRINCIPAL, size);

    Ok(host::CloseDocumentResponse {
        closed,
        was_dirty: is_dirty,
    })
}

pub fn apply_edits(
    state: &MonacoHostState,
    request: host::ApplyEditsRequest,
) -> Result<host::ApplyEditsResponse, String> {
    let resource = request
        .resource
        .ok_or_else(|| "missing document resource".to_string())?;

    // Security: check write permission
    let principal = default_principal();
    if !state
        .capabilities
        .check(&principal, Permission::WriteFile, &resource.path)
    {
        return Err("permission denied: write_file".to_string());
    }

    let registry = state.buffer_registry.read().map_err(|e| e.to_string())?;
    if !registry.has_buffer(&resource.path) {
        return Err(format!("No open buffer for resource: {}", resource.path));
    }
    drop(registry);

    // Convert protobuf edits to internal ContentChange format
    let changes: Result<Vec<ContentChange>, String> = request
        .edits
        .iter()
        .map(|edit| {
            let text = String::from_utf8(edit.text_utf8.clone())
                .map_err(|e| format!("Invalid UTF-8: {}", e))?;
            Ok(ContentChange::new(
                Position::new(edit.start_line, edit.start_column),
                Position::new(edit.end_line, edit.end_column),
                text,
                edit.range_offset,
                edit.range_length,
            ))
        })
        .collect();
    let changes = changes?;

    // Apply edits to buffer
    let registry = state.buffer_registry.read().map_err(|e| e.to_string())?;
    let mut applied_changes = Vec::new();
    let mut new_version_id = 0;

    for change in &changes {
        if let Some(event) = registry.apply_edit(&resource.path, change) {
            new_version_id = event.version_id;
            applied_changes.push(event.changes.clone());
        }
    }
    drop(registry);

    // Notify LSP server of document change
    if new_version_id > 0 {
        if let Ok(registry) = state.buffer_registry.read() {
            if let Some(content) = registry.get_buffer_content(&resource.path) {
                let language_id = detect_language_id_from_path(&resource.path);
                if let Ok(mut lsp) = state.lsp_registry.write() {
                    let change_json = serde_json::json!({
                        "range": null,
                        "rangeLength": null,
                        "text": content,
                    });
                    lsp.did_change(
                        &resource.path,
                        &language_id,
                        new_version_id as i32,
                        vec![change_json],
                    );
                }
            }
        }
    }

    Ok(host::ApplyEditsResponse {
        success: !applied_changes.is_empty(),
        new_version_id,
        applied_event: if applied_changes.is_empty() {
            None
        } else {
            Some(editor::ModelContentChangedEvent {
                resource: Some(resource),
                version_id: new_version_id,
                is_flush: false,
                is_undoing: false,
                is_redoing: false,
                changes: applied_changes
                    .into_iter()
                    .flatten()
                    .map(|c| editor::ModelContentChange {
                        start_line: c.start_position.line,
                        start_column: c.start_position.column,
                        end_line: c.end_position.line,
                        end_column: c.end_position.column,
                        range_offset: c.range_offset,
                        range_length: c.range_length,
                        text_utf8: c.text.into_bytes(),
                    })
                    .collect(),
            })
        },
    })
}

pub fn get_buffer_snapshot(
    state: &MonacoHostState,
    request: host::GetBufferSnapshotRequest,
) -> Result<host::GetBufferSnapshotResponse, String> {
    let resource = request
        .resource
        .ok_or_else(|| "missing document resource".to_string())?;

    let registry = state.buffer_registry.read().map_err(|e| e.to_string())?;
    let snapshot = registry
        .get_buffer_snapshot(&resource.path)
        .ok_or_else(|| format!("No buffer found for resource: {}", resource.path))?;

    Ok(host::GetBufferSnapshotResponse {
        snapshot: Some(editor::BufferSnapshot {
            resource: Some(resource),
            version_id: snapshot.version_id,
            content_utf8: snapshot.content_utf8,
            eol: snapshot.eol,
            is_dirty: snapshot.is_dirty,
        }),
    })
}

pub fn undo(
    state: &MonacoHostState,
    request: host::UndoRequest,
) -> Result<host::UndoResponse, String> {
    let resource = request
        .resource
        .ok_or_else(|| "missing document resource".to_string())?;

    let registry = state.buffer_registry.read().map_err(|e| e.to_string())?;
    if !registry.has_buffer(&resource.path) {
        return Err(format!("No open buffer for resource: {}", resource.path));
    }
    drop(registry);

    let event = state
        .buffer_registry
        .read()
        .map_err(|e| e.to_string())?
        .undo(&resource.path);

    if let Some(ref e) = event {
        if let Ok(registry) = state.buffer_registry.read() {
            if let Some(content) = registry.get_buffer_content(&resource.path) {
                let language_id = detect_language_id_from_path(&resource.path);
                if let Ok(mut lsp) = state.lsp_registry.write() {
                    let change_json = serde_json::json!({
                        "range": null,
                        "rangeLength": null,
                        "text": content,
                    });
                    lsp.did_change(
                        &resource.path,
                        &language_id,
                        e.version_id as i32,
                        vec![change_json],
                    );
                }
            }
        }
    }

    Ok(host::UndoResponse {
        success: event.is_some(),
        version_id: event.as_ref().map(|e| e.version_id).unwrap_or(0),
        changes: event
            .map(|e| {
                e.changes
                    .into_iter()
                    .map(|c| editor::ModelContentChange {
                        start_line: c.start_position.line,
                        start_column: c.start_position.column,
                        end_line: c.end_position.line,
                        end_column: c.end_position.column,
                        range_offset: c.range_offset,
                        range_length: c.range_length,
                        text_utf8: c.text.into_bytes(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

pub fn redo(
    state: &MonacoHostState,
    request: host::RedoRequest,
) -> Result<host::RedoResponse, String> {
    let resource = request
        .resource
        .ok_or_else(|| "missing document resource".to_string())?;

    let registry = state.buffer_registry.read().map_err(|e| e.to_string())?;
    if !registry.has_buffer(&resource.path) {
        return Err(format!("No open buffer for resource: {}", resource.path));
    }
    drop(registry);

    let event = state
        .buffer_registry
        .read()
        .map_err(|e| e.to_string())?
        .redo(&resource.path);

    if let Some(ref e) = event {
        if let Ok(registry) = state.buffer_registry.read() {
            if let Some(content) = registry.get_buffer_content(&resource.path) {
                let language_id = detect_language_id_from_path(&resource.path);
                if let Ok(mut lsp) = state.lsp_registry.write() {
                    let change_json = serde_json::json!({
                        "range": null,
                        "rangeLength": null,
                        "text": content,
                    });
                    lsp.did_change(
                        &resource.path,
                        &language_id,
                        e.version_id as i32,
                        vec![change_json],
                    );
                }
            }
        }
    }

    Ok(host::RedoResponse {
        success: event.is_some(),
        version_id: event.as_ref().map(|e| e.version_id).unwrap_or(0),
        changes: event
            .map(|e| {
                e.changes
                    .into_iter()
                    .map(|c| editor::ModelContentChange {
                        start_line: c.start_position.line,
                        start_column: c.start_position.column,
                        end_line: c.end_position.line,
                        end_column: c.end_position.column,
                        range_offset: c.range_offset,
                        range_length: c.range_length,
                        text_utf8: c.text.into_bytes(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

pub fn list_directory(
    request: host::ListDirectoryRequest,
) -> Result<host::ListDirectoryResponse, String> {
    let resource = request
        .resource
        .ok_or_else(|| "missing directory resource".to_string())?;
    let path = uri_to_path(&resource)?;

    let mut entries = Vec::new();
    for entry in fs::read_dir(&path).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let entry_path = entry.path();
        let stat = build_file_stat(&entry_path)?;
        entries.push(host::DirectoryEntry {
            name: entry.file_name().to_string_lossy().to_string(),
            stat: if request.include_file_stats {
                Some(stat)
            } else {
                None
            },
        });
    }

    entries.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(host::ListDirectoryResponse {
        resource: Some(resource),
        entries,
    })
}

fn build_file_stat(path: &Path) -> Result<file::FileStat, String> {
    let metadata = fs::symlink_metadata(path).map_err(|err| err.to_string())?;
    let is_directory = metadata.is_dir();
    let is_symbolic_link = metadata.file_type().is_symlink();
    let file_type = if is_symbolic_link {
        file::FileType::SymbolicLink as i32
    } else if is_directory {
        file::FileType::Directory as i32
    } else {
        file::FileType::File as i32
    };

    Ok(file::FileStat {
        resource: Some(path_to_uri(path)),
        is_directory,
        is_symbolic_link,
        size: metadata.len() as i64,
        mtime: modified_millis(&metadata).unwrap_or_default(),
        ctime: created_millis(&metadata).unwrap_or_default(),
        etag: format!(
            "{}:{}",
            metadata.len(),
            modified_millis(&metadata).unwrap_or_default()
        ),
        children: Vec::new(),
        r#type: file_type,
    })
}

fn modified_millis(metadata: &fs::Metadata) -> Option<i64> {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
}

fn created_millis(metadata: &fs::Metadata) -> Option<i64> {
    metadata
        .created()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
}

fn uri_to_path(resource: &file::Uri) -> Result<PathBuf, String> {
    if !resource.scheme.is_empty() && resource.scheme != "file" {
        return Err(format!("unsupported URI scheme: {}", resource.scheme));
    }
    Ok(PathBuf::from(&resource.path))
}

fn path_to_uri(path: &Path) -> file::Uri {
    file::Uri {
        scheme: "file".to_string(),
        authority: String::new(),
        path: path.to_string_lossy().to_string(),
        query: String::new(),
        fragment: String::new(),
    }
}

fn detect_language_id(path: &Path) -> String {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
    {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "mjs" | "cjs" => "javascript",
        "json" => "json",
        "md" => "markdown",
        "html" => "html",
        "css" => "css",
        "proto" => "protobuf",
        "yml" | "yaml" => "yaml",
        _ => "plaintext",
    }
    .to_string()
}

fn detect_language_id_from_path(path: &str) -> String {
    detect_language_id(Path::new(path))
}

fn detect_eol(path: &Path) -> String {
    fs::read(path)
        .ok()
        .and_then(|bytes| {
            if bytes.windows(2).any(|pair| pair == b"\r\n") {
                Some("\r\n")
            } else if bytes.contains(&b'\n') {
                Some("\n")
            } else {
                None
            }
        })
        .unwrap_or("\n")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_open_document_round_trip() {
        let temp_dir = std::env::temp_dir().join("monaco-tauri-host-handlers");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        let target = temp_dir.join("round-trip.ts");

        let state = MonacoHostState::new();

        let response = save_document(
            &state,
            host::SaveDocumentRequest {
                snapshot: Some(editor::BufferSnapshot {
                    resource: Some(path_to_uri(&target)),
                    version_id: 7,
                    content_utf8: b"console.log('hello');\n".to_vec(),
                    eol: "\n".to_string(),
                    is_dirty: true,
                }),
                create: true,
                overwrite: true,
                etag: String::new(),
            },
        )
        .unwrap();

        assert_eq!(response.persisted_version_id, 7);

        let opened = open_document(
            &state,
            host::OpenDocumentRequest {
                resource: Some(path_to_uri(&target)),
                create_if_missing: false,
                preferred_language_id: String::new(),
            },
        )
        .unwrap();

        let snapshot = opened.snapshot.unwrap();
        assert_eq!(snapshot.content_utf8, b"console.log('hello');\n".to_vec());
        assert_eq!(opened.language_id, "typescript");
    }

    #[test]
    fn buffer_registry_integration() {
        let temp_dir = std::env::temp_dir().join("monaco-tauri-buffer-registry");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        let target = temp_dir.join("test.txt");
        fs::write(&target, "initial content").unwrap();

        let state = MonacoHostState::new();

        // Open document - should create buffer in registry
        let opened = open_document(
            &state,
            host::OpenDocumentRequest {
                resource: Some(path_to_uri(&target)),
                create_if_missing: false,
                preferred_language_id: String::new(),
            },
        )
        .unwrap();

        assert_eq!(
            opened.snapshot.as_ref().unwrap().content_utf8,
            b"initial content"
        );

        // Apply an edit
        let edits_response = apply_edits(
            &state,
            host::ApplyEditsRequest {
                resource: Some(path_to_uri(&target)),
                edits: vec![editor::ModelContentChange {
                    start_line: 1,
                    start_column: 8,
                    end_line: 1,
                    end_column: 8,
                    range_offset: 7,
                    range_length: 0,
                    text_utf8: b" new".to_vec(),
                }],
            },
        )
        .unwrap();

        assert!(edits_response.success);

        // Verify content changed
        let snapshot = get_buffer_snapshot(
            &state,
            host::GetBufferSnapshotRequest {
                resource: Some(path_to_uri(&target)),
                at_version_id: 0,
            },
        )
        .unwrap();

        assert_eq!(
            snapshot.snapshot.as_ref().unwrap().content_utf8,
            b"initial new content"
        );
    }
}
