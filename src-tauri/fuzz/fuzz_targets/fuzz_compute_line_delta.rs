#![no_main]

use libfuzzer_sys::fuzz_target;
use monaco_tauri::buffer::BufferRegistry;
use monaco_tauri::wasm_sync::compute_line_delta;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let mid = data.len() / 2;
    let current_content = String::from_utf8_lossy(&data[..mid]);
    let old_content = String::from_utf8_lossy(&data[mid..]);

    let mut registry = BufferRegistry::new();
    registry.open_buffer("fuzz://test.rs".to_string(), &current_content);

    let _ = compute_line_delta(&registry, "fuzz://test.rs", &old_content, 1);
});
