use crate::buffer::ContentChange;

/// An undo/redo operation entry.
#[derive(Debug, Clone)]
pub enum UndoEntry {
    /// An insert operation (stores the text that was inserted and its position).
    Insert { position: ContentChange },
    /// A delete operation (stores the text that was deleted and its position).
    Delete {
        position: ContentChange,
        deleted_text: String,
    },
    /// A replace operation (stores both the old and new text).
    Replace {
        position: ContentChange,
        old_text: String,
    },
}

impl UndoEntry {
    /// Creates an insert undo entry.
    pub fn insert(change: ContentChange) -> Self {
        Self::Insert { position: change }
    }

    /// Creates a delete undo entry.
    pub fn delete(change: ContentChange, deleted_text: String) -> Self {
        Self::Delete {
            position: change,
            deleted_text,
        }
    }

    /// Creates a replace undo entry.
    pub fn replace(change: ContentChange, old_text: String) -> Self {
        Self::Replace {
            position: change,
            old_text,
        }
    }
}

/// A transaction groups multiple content changes together for atomic undo/redo.
#[derive(Debug, Clone)]
pub struct UndoTransaction {
    /// The entries in this transaction (in order of application).
    pub entries: Vec<UndoEntry>,
    /// A label for this transaction (optional, for UI display).
    pub label: Option<String>,
}

impl UndoTransaction {
    /// Creates a new empty transaction.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            label: None,
        }
    }

    /// Creates a transaction with a label.
    pub fn with_label(label: String) -> Self {
        Self {
            entries: Vec::new(),
            label: Some(label),
        }
    }

    /// Adds an entry to the transaction.
    pub fn push(&mut self, entry: UndoEntry) {
        self.entries.push(entry);
    }

    /// Returns true if the transaction has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the number of entries in the transaction.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl Default for UndoTransaction {
    fn default() -> Self {
        Self::new()
    }
}

/// Undo/redo stack for text buffer operations.
#[derive(Debug)]
pub struct UndoStack {
    /// Stack of undo transactions (most recent at the end).
    undo_stack: Vec<UndoTransaction>,
    /// Stack of redo transactions (most recent at the end).
    redo_stack: Vec<UndoTransaction>,
    /// Maximum number of undo entries to keep (0 = unlimited).
    max_size: usize,
}

impl UndoStack {
    /// Creates a new undo stack with unlimited size.
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_size: 0,
        }
    }

    /// Creates a new undo stack with a maximum size.
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_size,
        }
    }

    /// Pushes a transaction onto the undo stack.
    pub fn push(&mut self, transaction: UndoTransaction) {
        if !transaction.is_empty() {
            self.undo_stack.push(transaction);
            self.redo_stack.clear(); // Clear redo stack on new action

            // Enforce max size
            if self.max_size > 0 && self.undo_stack.len() > self.max_size {
                let excess = self.undo_stack.len() - self.max_size;
                self.undo_stack.drain(0..excess);
            }
        }
    }

    /// Returns true if there are undo operations available.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns true if there are redo operations available.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Pops a transaction from the undo stack and returns it.
    /// The transaction is moved to the redo stack.
    pub fn undo(&mut self) -> Option<UndoTransaction> {
        if let Some(transaction) = self.undo_stack.pop() {
            self.redo_stack.push(transaction.clone());
            Some(transaction)
        } else {
            None
        }
    }

    /// Pops a transaction from the redo stack and returns it.
    /// The transaction is moved to the undo stack.
    pub fn redo(&mut self) -> Option<UndoTransaction> {
        if let Some(transaction) = self.redo_stack.pop() {
            self.undo_stack.push(transaction.clone());
            Some(transaction)
        } else {
            None
        }
    }

    /// Clears both undo and redo stacks.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Returns the number of undo operations available.
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Returns the number of redo operations available.
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Position;

    fn make_insert_change() -> ContentChange {
        ContentChange::insert(Position::new(1, 1), "hello".to_string(), 0)
    }

    fn make_delete_change() -> ContentChange {
        ContentChange::delete(Position::new(1, 1), Position::new(1, 6), 0, 5)
    }

    #[test]
    fn undo_stack_new() {
        let stack = UndoStack::new();
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.undo_count(), 0);
        assert_eq!(stack.redo_count(), 0);
    }

    #[test]
    fn undo_stack_push_and_undo() {
        let mut stack = UndoStack::new();
        let mut transaction = UndoTransaction::new();
        transaction.push(UndoEntry::insert(make_insert_change()));
        stack.push(transaction);

        assert!(stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.undo_count(), 1);
        assert_eq!(stack.redo_count(), 0);

        let undone = stack.undo();
        assert!(undone.is_some());
        assert_eq!(undone.unwrap().len(), 1);

        assert!(!stack.can_undo());
        assert!(stack.can_redo());
        assert_eq!(stack.undo_count(), 0);
        assert_eq!(stack.redo_count(), 1);
    }

    #[test]
    fn undo_stack_redo() {
        let mut stack = UndoStack::new();
        let mut transaction = UndoTransaction::new();
        transaction.push(UndoEntry::insert(make_insert_change()));
        stack.push(transaction);

        stack.undo();
        assert!(stack.can_redo());

        let redone = stack.redo();
        assert!(redone.is_some());

        assert!(stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.undo_count(), 1);
        assert_eq!(stack.redo_count(), 0);
    }

    #[test]
    fn undo_stack_new_action_clears_redo() {
        let mut stack = UndoStack::new();
        let mut transaction1 = UndoTransaction::new();
        transaction1.push(UndoEntry::insert(make_insert_change()));
        stack.push(transaction1);

        stack.undo();
        assert!(stack.can_redo());

        let mut transaction2 = UndoTransaction::new();
        transaction2.push(UndoEntry::delete(make_delete_change(), "hello".to_string()));
        stack.push(transaction2);

        // Redo stack should be cleared after new action
        assert!(!stack.can_redo());
        assert_eq!(stack.redo_count(), 0);
    }

    #[test]
    fn undo_stack_max_size() {
        let mut stack = UndoStack::with_max_size(2);

        for i in 0..3 {
            let mut transaction = UndoTransaction::new();
            let change = ContentChange::insert(Position::new(1, 1), format!("text{}", i), 0);
            transaction.push(UndoEntry::insert(change));
            stack.push(transaction);
        }

        // Should only keep the last 2 transactions
        assert_eq!(stack.undo_count(), 2);
    }

    #[test]
    fn undo_stack_clear() {
        let mut stack = UndoStack::new();
        let mut transaction = UndoTransaction::new();
        transaction.push(UndoEntry::insert(make_insert_change()));
        stack.push(transaction);

        stack.clear();

        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.undo_count(), 0);
        assert_eq!(stack.redo_count(), 0);
    }

    #[test]
    fn undo_transaction_with_label() {
        let transaction = UndoTransaction::with_label("Test transaction".to_string());
        assert_eq!(transaction.label, Some("Test transaction".to_string()));
        assert!(transaction.is_empty());
        assert_eq!(transaction.len(), 0);
    }
}
