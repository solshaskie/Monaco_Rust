use monaco_tauri::host_handlers::MonacoHostState;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_file_path(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{}_{}", name, stamp))
}

#[test]
fn e2e_sparse_large_file_session_reads_viewport_without_buffer_hydration() {
    let path = temp_file_path("monaco_sparse_large_file.txt");
    let content: String = (0..20_000)
        .map(|i| format!("entry-{} = value-{}\n", i, i))
        .collect();
    fs::write(&path, content).unwrap();

    let state = MonacoHostState::new();
    assert_eq!(state.buffer_registry().read().buffer_count(), 0);

    let session = state.sparse_sessions().open_session(&path).unwrap();
    assert_eq!(state.buffer_registry().read().buffer_count(), 0);
    assert_eq!(session.line_count, 20_000);
    assert!(session.byte_length > 0);

    let slice = state
        .sparse_sessions()
        .read_viewport(&session.session_id, 10_240, 3)
        .unwrap();
    assert_eq!(slice.start_line, 10_240);
    assert_eq!(slice.end_line, 10_243);
    assert_eq!(
        slice.content,
        "entry-10240 = value-10240\nentry-10241 = value-10241\nentry-10242 = value-10242\n"
    );
    assert_eq!(state.buffer_registry().read().buffer_count(), 0);

    assert!(state.sparse_sessions().close_session(&session.session_id));

    fs::remove_file(path).ok();
}
