//! Text buffer management for Monaco Editor.
//!
//! This module provides a rope-based text buffer implementation that mirrors
//! Monaco's ITextModel public interface. It supports:
//! - UTF-8 content storage with efficient line indexing
//! - Incremental edits with delta tracking
//! - Undo/redo with transaction support
//! - Version tracking for conflict resolution
//!
//! The buffer is designed to run entirely in the Rust backend, communicating
//! with the frontend via protobuf messages.

mod content_change;
mod line_index;
mod position;
mod registry;
mod text_buffer;
mod undo;

pub use content_change::ContentChange;
pub use line_index::LineIndex;
pub use position::Position;
pub use registry::BufferRegistry;
pub use text_buffer::{BufferSnapshot as TextBufferSnapshot, LineRange, ModelContentChangedEvent, TextBuffer};
pub use undo::{UndoEntry, UndoStack, UndoTransaction};
