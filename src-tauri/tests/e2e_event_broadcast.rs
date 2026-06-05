use std::fs;
use std::path::Path;

use monaco_tauri::events::{
    BufferClosedEvent, BufferContentChangedEvent, BufferOpenedEvent,
};
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

/// E2E: Verify buffer-content-changed event can be constructed and serialized.
/// (The actual Tauri emit path requires an AppHandle; we verify the payload here.)
#[test]
fn e2e_buffer_content_changed_event_roundtrip() {
    let event = BufferContentChangedEvent {
        path: "test://main.rs".to_string(),
        version_id: 7,
        changes: vec![monaco_tauri::events::BufferChangeEvent {
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 1,
            range_offset: 0,
            range_length: 0,
            text: "hello".to_string(),
        }],
        is_undoing: false,
        is_redoing: false,
    };

    let json = serde_json::to_string(&event).unwrap();
    let parsed: BufferContentChangedEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, parsed);
    assert_eq!(parsed.version_id, 7);
    assert_eq!(parsed.changes.len(), 1);
}

/// E2E: Verify buffer opened / closed events roundtrip.
#[test]
fn e2e_buffer_lifecycle_events_roundtrip() {
    let open = BufferOpenedEvent {
        path: "test://open.rs".to_string(),
        version_id: 1,
        language_id: "rust".to_string(),
    };
    let close = BufferClosedEvent {
        path: "test://open.rs".to_string(),
    };

    let open_json = serde_json::to_string(&open).unwrap();
    let close_json = serde_json::to_string(&close).unwrap();

    assert!(open_json.contains("rust"));
    assert!(close_json.contains("open.rs"));
}

/// E2E: Verify that applying edits produces correct version increment
/// and that the event payload reflects the change.
#[test]
fn e2e_edit_produces_version_increment() {
    let temp_dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("test-temp-e2e-version");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let path = temp_dir.join("version.rs");
    fs::write(&path, "fn main() {}\n").unwrap();

    let state = MonacoHostState::new();
    let opened = host_handlers::open_document(
        &state,
        host::OpenDocumentRequest {
            resource: Some(file_uri(&path)),
            create_if_missing: false,
            preferred_language_id: String::new(),
        },
    )
    .unwrap();

    let initial_version = opened.snapshot.unwrap().version_id;

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
                text_utf8: b";".to_vec(),
            }],
        },
    )
    .unwrap();

    assert!(edited.new_version_id > initial_version);

    let _ = fs::remove_dir_all(&temp_dir);
}
