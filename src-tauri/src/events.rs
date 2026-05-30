use serde::{Deserialize, Serialize};
use tauri::Emitter;

/// A single buffer change for event broadcasting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BufferChangeEvent {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub range_offset: u64,
    pub range_length: u64,
    pub text: String,
}

/// Event emitted when a buffer's content changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BufferContentChangedEvent {
    pub path: String,
    pub version_id: u64,
    pub changes: Vec<BufferChangeEvent>,
    pub is_undoing: bool,
    pub is_redoing: bool,
}

/// Event emitted when a buffer is opened.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BufferOpenedEvent {
    pub path: String,
    pub version_id: u64,
    pub language_id: String,
}

/// Event emitted when a buffer is closed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BufferClosedEvent {
    pub path: String,
}

/// Event emitted when a buffer is saved.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BufferSavedEvent {
    pub path: String,
    pub version_id: u64,
}

/// Event emitted when the workspace roots change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceRootsChangedEvent {
    pub roots: Vec<WorkspaceRootEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspaceRootEvent {
    pub path: String,
    pub name: String,
    pub is_primary: bool,
}

/// Broadcaster for buffer registry events via Tauri's event system.
///
/// Events flow from the Rust backend to the TypeScript frontend,
/// enabling multi-view synchronization and agent-driven editing.
pub struct EventBroadcaster;

impl EventBroadcaster {
    /// Publishes a buffer content changed event.
    pub fn emit_buffer_content_changed<R: tauri::Runtime>(
        app: &impl Emitter<R>,
        event: BufferContentChangedEvent,
    ) {
        let _ = app.emit("buffer-content-changed", event);
    }

    /// Publishes a buffer opened event.
    pub fn emit_buffer_opened<R: tauri::Runtime>(app: &impl Emitter<R>, event: BufferOpenedEvent) {
        let _ = app.emit("buffer-opened", event);
    }

    /// Publishes a buffer closed event.
    pub fn emit_buffer_closed<R: tauri::Runtime>(app: &impl Emitter<R>, event: BufferClosedEvent) {
        let _ = app.emit("buffer-closed", event);
    }

    /// Publishes a buffer saved event.
    pub fn emit_buffer_saved<R: tauri::Runtime>(app: &impl Emitter<R>, event: BufferSavedEvent) {
        let _ = app.emit("buffer-saved", event);
    }

    /// Publishes a workspace roots changed event.
    pub fn emit_workspace_roots_changed<R: tauri::Runtime>(
        app: &impl Emitter<R>,
        event: WorkspaceRootsChangedEvent,
    ) {
        let _ = app.emit("workspace-roots-changed", event);
    }
}

/// A lightweight subscription tracker for buffer events.
///
/// Tracks which resources have active listeners so the backend can
/// avoid emitting events for unobserved buffers.
#[derive(Debug, Default)]
pub struct EventSubscriptionTracker {
    /// Set of resource paths that have active subscribers.
    subscribed_paths: std::collections::HashSet<String>,
    /// Monotonically increasing event sequence number for ordering.
    sequence: u64,
}

impl EventSubscriptionTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a subscription for a buffer path.
    pub fn subscribe(&mut self, path: &str) {
        self.subscribed_paths.insert(path.to_string());
    }

    /// Removes a subscription for a buffer path.
    pub fn unsubscribe(&mut self, path: &str) {
        self.subscribed_paths.remove(path);
    }

    /// Returns true if the given path has active subscribers.
    pub fn is_subscribed(&self, path: &str) -> bool {
        self.subscribed_paths.contains(path)
    }

    /// Returns the next sequence number and increments the counter.
    pub fn next_sequence(&mut self) -> u64 {
        self.sequence += 1;
        self.sequence
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_tracker_tracks_subscriptions() {
        let mut tracker = EventSubscriptionTracker::new();
        assert!(!tracker.is_subscribed("test://a.rs"));

        tracker.subscribe("test://a.rs");
        assert!(tracker.is_subscribed("test://a.rs"));

        tracker.unsubscribe("test://a.rs");
        assert!(!tracker.is_subscribed("test://a.rs"));
    }

    #[test]
    fn subscription_tracker_sequence_increments() {
        let mut tracker = EventSubscriptionTracker::new();
        assert_eq!(tracker.next_sequence(), 1);
        assert_eq!(tracker.next_sequence(), 2);
        assert_eq!(tracker.next_sequence(), 3);
    }

    #[test]
    fn buffer_content_changed_event_roundtrip() {
        let event = BufferContentChangedEvent {
            path: "test://main.rs".to_string(),
            version_id: 5,
            changes: vec![BufferChangeEvent {
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
    }
}
