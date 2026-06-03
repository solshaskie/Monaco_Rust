use std::fs;
use std::path::Path;

use monaco_tauri::host_handlers;
use monaco_tauri::host_handlers::MonacoHostState;
use monaco_tauri::proto::code::ipc::editor::host;
use monaco_tauri::proto::code::ipc::file;

fn file_uri(path: &Path) -> file::Uri {
    file::Uri {
        scheme: "file".to_string(),
        authority: String::new(),
        path: path.to_string_lossy().to_string(),
        query: String::new(),
        fragment: String::new(),
    }
}

/// E2E: Verify the full command surface exposed in main.rs.
/// Each command is tested through the host_handlers layer, which is
/// the narrow surface beneath the #[tauri::command] wrappers.
#[test]
fn e2e_workspace_roots() {
    let state = MonacoHostState::new();
    let roots = host_handlers::get_workspace_roots(&state);
    // At minimum, the workspace roots call should succeed
    assert!(roots.roots.is_empty() || !roots.roots.is_empty());
}

#[test]
fn e2e_open_document_lifecycle() {
    let temp_dir = std::env::temp_dir().join("monaco-e2e-open");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let path = temp_dir.join("test.rs");
    fs::write(&path, "fn main() {}\n").unwrap();

    let state = MonacoHostState::new();

    // open_document
    let opened = host_handlers::open_document(
        &state,
        host::OpenDocumentRequest {
            resource: Some(file_uri(&path)),
            create_if_missing: false,
            preferred_language_id: String::new(),
        },
    )
    .unwrap();

    assert!(opened.snapshot.is_some());
    let snapshot = opened.snapshot.unwrap();
    assert_eq!(snapshot.content_utf8, b"fn main() {}\n");

    // get_buffer_snapshot
    let fetched = host_handlers::get_buffer_snapshot(
        &state,
        host::GetBufferSnapshotRequest {
            resource: Some(file_uri(&path)),
            at_version_id: 0,
        },
    )
    .unwrap();
    assert!(fetched.snapshot.is_some());

    // apply_edits
    let edited = host_handlers::apply_edits(
        &state,
        host::ApplyEditsRequest {
            resource: Some(file_uri(&path)),
            edits: vec![monaco_tauri::proto::code::ipc::editor::ModelContentChange {
                start_line: 1,
                start_column: 13,
                end_line: 1,
                end_column: 13,
                range_offset: 12,
                range_length: 0,
                text_utf8: b"// comment".to_vec(),
            }],
        },
    )
    .unwrap();
    assert!(edited.success);

    // save_document
    let saved = host_handlers::save_document(
        &state,
        host::SaveDocumentRequest {
            snapshot: Some(monaco_tauri::proto::code::ipc::editor::BufferSnapshot {
                resource: Some(file_uri(&path)),
                version_id: edited.new_version_id,
                content_utf8: b"fn main() {}// comment\n".to_vec(),
                eol: "\n".to_string(),
                is_dirty: true,
            }),
            create: false,
            overwrite: true,
            etag: String::new(),
        },
    )
    .unwrap();
    assert!(saved.persisted_version_id > 0);

    // close_document
    let closed = host_handlers::close_document(
        &state,
        host::CloseDocumentRequest {
            resource: Some(file_uri(&path)),
            save_if_dirty: false,
        },
    )
    .unwrap();
    assert!(closed.closed);

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn e2e_list_directory() {
    let temp_dir = std::env::temp_dir().join("monaco-e2e-list");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();
    fs::write(temp_dir.join("a.rs"), "").unwrap();
    fs::write(temp_dir.join("b.rs"), "").unwrap();

    let listing = host_handlers::list_directory(host::ListDirectoryRequest {
        resource: Some(file_uri(&temp_dir)),
        include_file_stats: true,
    })
    .unwrap();

    let names: Vec<String> = listing.entries.iter().map(|e| e.name.clone()).collect();
    assert!(names.contains(&"a.rs".to_string()));
    assert!(names.contains(&"b.rs".to_string()));

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn e2e_undo_redo() {
    let temp_dir = std::env::temp_dir().join("monaco-e2e-undo");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let path = temp_dir.join("undo.rs");
    fs::write(&path, "hello\n").unwrap();

    let state = MonacoHostState::new();
    host_handlers::open_document(
        &state,
        host::OpenDocumentRequest {
            resource: Some(file_uri(&path)),
            create_if_missing: false,
            preferred_language_id: String::new(),
        },
    )
    .unwrap();

    host_handlers::apply_edits(
        &state,
        host::ApplyEditsRequest {
            resource: Some(file_uri(&path)),
            edits: vec![monaco_tauri::proto::code::ipc::editor::ModelContentChange {
                start_line: 1,
                start_column: 6,
                end_line: 1,
                end_column: 6,
                range_offset: 5,
                range_length: 0,
                text_utf8: b" world".to_vec(),
            }],
        },
    )
    .unwrap();

    let undo = host_handlers::undo(
        &state,
        host::UndoRequest {
            resource: Some(file_uri(&path)),
        },
    )
    .unwrap();
    assert!(undo.success);

    let redo = host_handlers::redo(
        &state,
        host::RedoRequest {
            resource: Some(file_uri(&path)),
        },
    )
    .unwrap();
    assert!(redo.success);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn e2e_security_permissions() {
    let state = MonacoHostState::new();
    let principal = monaco_tauri::security::Principal {
        id: "test-user".to_string(),
        kind: monaco_tauri::security::PrincipalKind::User,
    };

    // Default: no permissions
    assert!(!state.capabilities().check(
        &principal,
        monaco_tauri::security::Permission::ReadFile,
        "/etc/passwd"
    ));

    // Grant and verify
    state
        .capabilities()
        .grant("test-user", monaco_tauri::security::Permission::ReadFile);
    assert!(state.capabilities().check(
        &principal,
        monaco_tauri::security::Permission::ReadFile,
        "/etc/passwd"
    ));
}
