use std::fs;
use std::path::PathBuf;

use monaco_tauri::host_handlers;
use monaco_tauri::host_handlers::MonacoHostState;
use monaco_tauri::proto::code::ipc::editor;
use monaco_tauri::proto::code::ipc::editor::host;
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

#[test]
fn save_as_and_list_directory_round_trip() {
    let temp_dir = std::env::temp_dir().join("monaco-tauri-m4-integration");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let source = temp_dir.join("source.js");
    let target = temp_dir.join("renamed.js");
    fs::write(&source, "const answer = 42;\n").unwrap();

    let state = MonacoHostState::new();

    let opened = host_handlers::open_document(&state, host::OpenDocumentRequest {
        resource: Some(file_uri(&source)),
        create_if_missing: false,
        preferred_language_id: String::new(),
    })
    .unwrap();

    let snapshot = opened.snapshot.unwrap();
    let saved = host_handlers::save_document_as(&state, host::SaveDocumentAsRequest {
        snapshot: Some(editor::BufferSnapshot {
            resource: Some(file_uri(&source)),
            version_id: snapshot.version_id + 1,
            content_utf8: b"const renamed = true;\n".to_vec(),
            eol: "\n".to_string(),
            is_dirty: true,
        }),
        target: Some(file_uri(&target)),
        overwrite: true,
    })
    .unwrap();

    assert!(saved.source_closed);
    assert_eq!(
        saved.snapshot.unwrap().content_utf8,
        b"const renamed = true;\n".to_vec()
    );

    let listing = host_handlers::list_directory(host::ListDirectoryRequest {
        resource: Some(file_uri(&temp_dir)),
        include_file_stats: true,
    })
    .unwrap();

    let names: Vec<String> = listing.entries.into_iter().map(|entry| entry.name).collect();
    assert_eq!(names, vec!["renamed.js".to_string(), "source.js".to_string()]);
}

#[test]
fn tauri_dist_contains_shell_and_worker_assets() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dist_root = manifest_dir.parent().unwrap().join("tauri-dist");
    let index_html = dist_root.join("index.html");
    let asset_dir = dist_root.join("vendor/monaco-editor/min/vs/assets");

    let html = fs::read_to_string(&index_html).expect("tauri-dist index.html should exist");
    assert!(html.contains("Monaco Tauri Prototype"));
    assert!(html.contains("Workers"));

    let asset_names: Vec<String> = fs::read_dir(&asset_dir)
        .expect("worker asset directory should exist")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();

    assert!(
        asset_names.iter().any(|name| name.starts_with("ts.worker-") && name.ends_with(".js")),
        "expected bundled TS worker asset"
    );
    assert!(
        asset_names
            .iter()
            .any(|name| name.starts_with("json.worker-") && name.ends_with(".js")),
        "expected bundled JSON worker asset"
    );
}

// ---------------------------------------------------------------------------
// Phase 7: End-to-end editing workflow tests
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_edit_undo_redo_save_workflow() {
    let temp_dir = std::env::temp_dir().join("monaco-tauri-e2e-workflow");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let path = temp_dir.join("workflow.rs");
    std::fs::write(&path, "fn main() {}\n").unwrap();

    let state = MonacoHostState::new();

    // 1. Open document
    let opened = host_handlers::open_document(&state, host::OpenDocumentRequest {
        resource: Some(file_uri(&path)),
        create_if_missing: false,
        preferred_language_id: String::new(),
    }).unwrap();
    let snapshot = opened.snapshot.unwrap();
    assert_eq!(snapshot.content_utf8, b"fn main() {}\n");

    // 2. Apply edits
    let edit_response = host_handlers::apply_edits(&state, host::ApplyEditsRequest {
        resource: Some(file_uri(&path)),
        edits: vec![
            editor::ModelContentChange {
                start_line: 1,
                start_column: 12,
                end_line: 1,
                end_column: 12,
                range_offset: 11,
                range_length: 0,
                text_utf8: b" { println!(\"hello\") }".to_vec(),
            },
        ],
    }).unwrap();
    assert!(edit_response.success);
    assert_eq!(edit_response.new_version_id, 2);

    // 3. Undo
    let undo_response = host_handlers::undo(&state, host::UndoRequest {
        resource: Some(file_uri(&path)),
    }).unwrap();
    assert!(undo_response.success);
    assert_eq!(undo_response.version_id, 3);
    assert!(!undo_response.changes.is_empty());

    // 4. Redo
    let redo_response = host_handlers::redo(&state, host::RedoRequest {
        resource: Some(file_uri(&path)),
    }).unwrap();
    assert!(redo_response.success);
    assert_eq!(redo_response.version_id, 4);
    assert!(!redo_response.changes.is_empty());

    // 5. Get snapshot and save
    let snap = host_handlers::get_buffer_snapshot(&state, host::GetBufferSnapshotRequest {
        resource: Some(file_uri(&path)),
        at_version_id: 0,
    }).unwrap();
    let snap_clone = snap.snapshot.clone().unwrap();
    assert_eq!(snap_clone.version_id, 4);

    let _ = host_handlers::save_document(&state, host::SaveDocumentRequest {
        snapshot: Some(snap.snapshot.unwrap()),
        create: false,
        overwrite: true,
        etag: String::new(),
    }).unwrap();

    // 6. Verify file on disk
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("println!(\"hello\")"));

    // 7. Close
    let closed = host_handlers::close_document(&state, host::CloseDocumentRequest {
        resource: Some(file_uri(&path)),
        save_if_dirty: false,
    }).unwrap();
    assert!(closed.closed);
}

#[test]
fn multi_view_buffer_sync_via_registry() {
    let temp_dir = std::env::temp_dir().join("monaco-tauri-multi-view");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let path = temp_dir.join("shared.rs");
    std::fs::write(&path, "let x = 1;\n").unwrap();

    let state = MonacoHostState::new();

    // View 1 opens the file
    let v1 = host_handlers::open_document(&state, host::OpenDocumentRequest {
        resource: Some(file_uri(&path)),
        create_if_missing: false,
        preferred_language_id: String::new(),
    }).unwrap();
    assert_eq!(v1.snapshot.unwrap().content_utf8, b"let x = 1;\n");

    // View 2 opens the same file (should get the same buffer)
    let v2 = host_handlers::open_document(&state, host::OpenDocumentRequest {
        resource: Some(file_uri(&path)),
        create_if_missing: false,
        preferred_language_id: String::new(),
    }).unwrap();
    assert_eq!(v2.snapshot.unwrap().content_utf8, b"let x = 1;\n");

    // View 1 applies an edit (replace the entire line)
    host_handlers::apply_edits(&state, host::ApplyEditsRequest {
        resource: Some(file_uri(&path)),
        edits: vec![editor::ModelContentChange {
            start_line: 1,
            start_column: 1,
            end_line: 2,
            end_column: 1,
            range_offset: 0,
            range_length: 11,
            text_utf8: b"let x = 1 + 2;\n".to_vec(),
        }],
    }).unwrap();

    // View 2 should see the updated content
    let v2_snapshot = host_handlers::get_buffer_snapshot(&state, host::GetBufferSnapshotRequest {
        resource: Some(file_uri(&path)),
        at_version_id: 0,
    }).unwrap();
    let content = String::from_utf8(v2_snapshot.snapshot.unwrap().content_utf8).unwrap();
    assert_eq!(content, "let x = 1 + 2;\n");

    // Close from either view should release the buffer
    let closed = host_handlers::close_document(&state, host::CloseDocumentRequest {
        resource: Some(file_uri(&path)),
        save_if_dirty: false,
    }).unwrap();
    assert!(closed.closed);
}

#[test]
fn security_permission_denial_blocks_operations() {
    let temp_dir = std::env::temp_dir().join("monaco-tauri-security");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let path = temp_dir.join("secret.txt");
    std::fs::write(&path, "secret").unwrap();

    let state = MonacoHostState::new();

    // Revoke read permission for the default user principal
    state.capabilities().revoke("user", monaco_tauri::security::Permission::ReadFile);

    // Open should now fail
    let result = host_handlers::open_document(&state, host::OpenDocumentRequest {
        resource: Some(file_uri(&path)),
        create_if_missing: false,
        preferred_language_id: String::new(),
    });
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("permission denied"));

    // Re-grant for cleanup / other tests
    state.capabilities().grant("user", monaco_tauri::security::Permission::ReadFile);
}

#[test]
fn security_sandbox_blocks_oversized_file() {
    let temp_dir = std::env::temp_dir().join("monaco-tauri-sandbox");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let path = temp_dir.join("huge.txt");
    let big_content = vec![b'x'; 200 * 1024 * 1024 + 1]; // 200MB + 1 byte
    std::fs::write(&path, &big_content).unwrap();

    let state = MonacoHostState::new();

    let result = host_handlers::open_document(&state, host::OpenDocumentRequest {
        resource: Some(file_uri(&path)),
        create_if_missing: false,
        preferred_language_id: String::new(),
    });
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.contains("exceeded") || err.contains("limit"),
        "Expected sandbox limit error, got: {}", err
    );

    let _ = std::fs::remove_file(&path);
}
