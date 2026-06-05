use std::time::Instant;

use monaco_tauri::buffer::{BufferRegistry, ContentChange, Position, TextBuffer};

/// Regression benchmark: small file insert should complete in <1ms.
#[test]
fn bench_small_file_insert() {
    // Pre-construct buffers and changes outside the timing loop
    let mut buffers: Vec<TextBuffer> = (0..1000)
        .map(|_| TextBuffer::new("bench://small.rs".to_string(), "fn main() {}\n"))
        .collect();
    let change =
        ContentChange::insert(Position::new(1, 12), " println!(\"hello\")".to_string(), 11);

    let start = Instant::now();
    for buffer in &mut buffers {
        let _ = buffer.apply_change(&change);
    }
    let elapsed = start.elapsed();
    let per_op = elapsed.as_micros() as f64 / 1000.0;
    println!("Small file insert: {:.2} µs/op", per_op);
    assert!(
        per_op < 1000.0,
        "Small file insert too slow: {:.2} µs/op",
        per_op
    );
}

/// Regression benchmark: undo/redo should complete in <1ms.
#[test]
fn bench_undo_redo_cycle() {
    let mut buffer = TextBuffer::new("bench://undo.rs".to_string(), "hello world\n");
    let change = ContentChange::insert(Position::new(1, 7), "beautiful ".to_string(), 6);
    buffer.apply_change(&change);

    let start = Instant::now();
    for _ in 0..1000 {
        let _ = buffer.undo();
        let _ = buffer.redo();
    }
    let elapsed = start.elapsed();
    let per_cycle = elapsed.as_micros() as f64 / 1000.0;
    println!("Undo/redo cycle: {:.2} µs/cycle", per_cycle);
    assert!(
        per_cycle < 1000.0,
        "Undo/redo too slow: {:.2} µs/cycle",
        per_cycle
    );
}

/// Regression benchmark: get_line for a 1k-line file should be fast.
#[test]
fn bench_line_access_large_file() {
    let content: String = (0..1000)
        .map(|i| format!("// Line {} comment that is long enough to matter\n", i))
        .collect();
    let buffer = TextBuffer::new("bench://large.rs".to_string(), &content);

    let start = Instant::now();
    for _ in 0..10_000 {
        let _ = buffer.get_line(500);
        let _ = buffer.get_line(0);
        let _ = buffer.get_line(999);
    }
    let elapsed = start.elapsed();
    let per_access = elapsed.as_nanos() as f64 / 30_000.0; // 3 accesses per iteration
    println!("Line access (1k lines): {:.1} ns/op", per_access);
    assert!(
        per_access < 15_000.0,
        "Line access too slow: {:.1} ns/op",
        per_access
    );
}

/// Regression benchmark: Registry open/close should be fast.
#[test]
fn bench_registry_open_close() {
    let mut registry = BufferRegistry::new();

    let start = Instant::now();
    for i in 0..100 {
        let path = format!("bench://file{}.rs", i);
        registry.open_buffer(path.clone(), &format!("fn main_{}() {{}}\n", i));
        registry.close_buffer(&path);
    }
    let elapsed = start.elapsed();
    let per_cycle = elapsed.as_micros() as f64 / 100.0;
    println!("Registry open/close: {:.2} µs/cycle", per_cycle);
    assert!(
        per_cycle < 1000.0,
        "Registry open/close too slow: {:.2} µs/cycle",
        per_cycle
    );
}

/// Regression benchmark: Content range fetch for a 10k-line file.
#[test]
fn bench_content_range_large_file() {
    let content: String = (0..10_000)
        .map(|i| {
            format!(
                "// Line {} with some padding to make it realistic in size\n",
                i
            )
        })
        .collect();
    let buffer = TextBuffer::new("bench://huge.rs".to_string(), &content);

    let start = Instant::now();
    for _ in 0..1000 {
        let _ = buffer.get_value_in_line_range(100, 200);
        let _ = buffer.get_value_in_line_range(5000, 5100);
        let _ = buffer.get_value_in_line_range(9900, 10_000);
    }
    let elapsed = start.elapsed();
    let per_call = elapsed.as_nanos() as f64 / 3000.0;
    println!("Content range (10k lines): {:.1} ns/call", per_call);
    assert!(
        per_call < 100_000.0,
        "Content range fetch too slow: {:.1} ns/call",
        per_call
    );
}

/// Regression benchmark: Dirty line tracking overhead.
#[test]
fn bench_dirty_line_tracking_overhead() {
    let mut buffer = TextBuffer::new("bench://dirty.rs".to_string(), "line1\nline2\nline3\n");
    let change = ContentChange::insert(Position::new(2, 6), " appended".to_string(), 6);

    let start = Instant::now();
    for _ in 0..10_000 {
        let _ = buffer.apply_change(&change);
        // Check and clear dirty lines
        assert_eq!(buffer.dirty_line_ranges().len(), 1);
        buffer.clear_dirty_lines();
        // Reset buffer
        buffer = TextBuffer::new("bench://dirty.rs".to_string(), "line1\nline2\nline3\n");
    }
    let elapsed = start.elapsed();
    let per_op = elapsed.as_micros() as f64 / 10_000.0;
    println!("Dirty line tracking: {:.2} µs/op", per_op);
    assert!(
        per_op < 100.0,
        "Dirty line tracking too slow: {:.2} µs/op",
        per_op
    );
}

/// Regression benchmark: Tokenize a large Rust file.
#[test]
fn bench_tokenize_large_rust_file() {
    let content: String = (0..1000)
        .map(|i| {
            format!(
                "fn function_{}() -> u32 {{ let x = {}; return x + {}; }}\n",
                i,
                i,
                i * 2
            )
        })
        .collect();

    let mut parser = monaco_tauri::syntax::SyntaxParser::for_rust().unwrap();

    let start = Instant::now();
    let parsed = parser.parse(&content).unwrap();
    let tokens = monaco_tauri::syntax::tokenize_tree(&parsed.tree, &content);
    let elapsed = start.elapsed();

    println!(
        "Tokenize large file ({} lines, {} tokens): {:?}",
        content.lines().count(),
        tokens.len(),
        elapsed
    );
    assert!(
        elapsed.as_millis() < 500,
        "Tokenization too slow: {:?}",
        elapsed
    );
}
