#![no_main]

use libfuzzer_sys::fuzz_target;
use monaco_tauri::buffer::{ContentChange, Position, TextBuffer};

fuzz_target!(|data: &[u8]| {
    if data.len() < 17 {
        return;
    }

    let start_line = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let start_column = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let end_line = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
    let end_column = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
    let range_offset = u64::from_le_bytes([
        data[16], data[17 % data.len()], data[18 % data.len()], data[19 % data.len()],
        data[20 % data.len()], data[21 % data.len()], data[22 % data.len()], data[23 % data.len()],
    ]);
    let range_length = if data.len() > 24 {
        u64::from_le_bytes([
            data[24], data[25 % data.len()], data[26 % data.len()], data[27 % data.len()],
            data[28 % data.len()], data[29 % data.len()], data[30 % data.len()], data[31 % data.len()],
        ])
    } else {
        0
    };

    let text = if data.len() > 32 {
        String::from_utf8_lossy(&data[32..]).to_string()
    } else {
        String::new()
    };

    let mut buffer = TextBuffer::new("fuzz://test.rs".to_string(), "fn main() {}\n");

    let change = ContentChange::new(
        Position::new(start_line.max(1), start_column.max(1)),
        Position::new(end_line.max(1), end_column.max(1)),
        text,
        range_offset,
        range_length,
    );

    let _ = buffer.apply_change(&change);
});
