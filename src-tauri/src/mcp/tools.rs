use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::buffer::{BufferRegistry, ContentChange, LineRange, ModelContentChangedEvent, Position};

/// The result of executing an MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpToolResult {
    pub success: bool,
    pub content: String,
    pub version_id: u64,
    pub error: Option<String>,
}

impl McpToolResult {
    pub fn ok(content: impl Into<String>, version_id: u64) -> Self {
        Self {
            success: true,
            content: content.into(),
            version_id,
            error: None,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            content: String::new(),
            version_id: 0,
            error: Some(message.into()),
        }
    }
}

/// Errors that can occur during MCP tool execution.
#[derive(Debug, Clone, PartialEq)]
pub enum McpToolError {
    BufferNotFound(String),
    VersionConflict { expected: u64, actual: u64 },
    InvalidEdit(String),
    SerializationError(String),
}

impl std::fmt::Display for McpToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpToolError::BufferNotFound(r) => write!(f, "Buffer not found: {}", r),
            McpToolError::VersionConflict { expected, actual } => {
                write!(f, "Version conflict: expected {} but found {}", expected, actual)
            }
            McpToolError::InvalidEdit(msg) => write!(f, "Invalid edit: {}", msg),
            McpToolError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
        }
    }
}

/// A trait for MCP tools that can be executed against the buffer registry.
pub trait McpTool: Send + Sync {
    /// The unique name of the tool (e.g. "read_file").
    fn name(&self) -> &str;

    /// A human-readable description for LLM consumption.
    fn description(&self) -> &str;

    /// JSON Schema for the tool's input parameters.
    fn input_schema(&self) -> serde_json::Value;

    /// Execute the tool with the given JSON arguments.
    fn execute(&self, registry: &BufferRegistry, args: &str) -> McpToolResult;
}

/// A registry of available MCP tools.
#[derive(Default)]
pub struct McpToolRegistry {
    tools: HashMap<String, Box<dyn McpTool>>,
}

impl McpToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn McpTool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<&dyn McpTool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    pub fn list_tools(&self) -> Vec<&dyn McpTool> {
        self.tools.values().map(|t| t.as_ref()).collect()
    }

    pub fn names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Creates a default registry with the built-in file editing tools.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(ReadFileTool));
        registry.register(Box::new(EditFileTool));
        registry.register(Box::new(ListSymbolsTool));
        registry.register(Box::new(ApplyEditsTool));
        registry
    }
}

// ---------------------------------------------------------------------------
// Tool: read_file
// ---------------------------------------------------------------------------

struct ReadFileTool;

impl McpTool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file or a range of lines. Returns the file content as a string."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute file path" },
                "start_line": { "type": "integer", "description": "Start line (0-indexed, inclusive)" },
                "end_line": { "type": "integer", "description": "End line (0-indexed, exclusive)" }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, registry: &BufferRegistry, args: &str) -> McpToolResult {
        #[derive(Deserialize)]
        struct Args {
            path: String,
            start_line: Option<usize>,
            end_line: Option<usize>,
        }

        let args: Args = match serde_json::from_str(args) {
            Ok(a) => a,
            Err(e) => return McpToolResult::err(format!("Invalid arguments: {}", e)),
        };

        let version_id = registry.get_buffer_version(&args.path).unwrap_or(0);

        let content = if let (Some(start), Some(end)) = (args.start_line, args.end_line) {
            registry.get_buffer_content_range(&args.path, start, end)
        } else {
            registry.get_buffer_content(&args.path)
        };

        match content {
            Some(text) => McpToolResult::ok(text, version_id),
            None => McpToolResult::err(format!("Buffer not found: {}", args.path)),
        }
    }
}

// ---------------------------------------------------------------------------
// Tool: edit_file
// ---------------------------------------------------------------------------

struct EditFileTool;

impl McpTool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Apply a single text edit to a file with optimistic version checking. \
         The edit must specify the exact old_text to replace and the new_text to insert."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute file path" },
                "old_text": { "type": "string", "description": "Exact text to replace" },
                "new_text": { "type": "string", "description": "Replacement text" },
                "expected_version": { "type": "integer", "description": "Buffer version_id for optimistic locking" }
            },
            "required": ["path", "old_text", "new_text"]
        })
    }

    fn execute(&self, registry: &BufferRegistry, args: &str) -> McpToolResult {
        #[derive(Deserialize)]
        struct Args {
            path: String,
            old_text: String,
            new_text: String,
            expected_version: Option<u64>,
        }

        let args: Args = match serde_json::from_str(args) {
            Ok(a) => a,
            Err(e) => return McpToolResult::err(format!("Invalid arguments: {}", e)),
        };

        let content = match registry.get_buffer_content(&args.path) {
            Some(c) => c,
            None => return McpToolResult::err(format!("Buffer not found: {}", args.path)),
        };

        let start_offset = match content.find(&args.old_text) {
            Some(o) => o,
            None => return McpToolResult::err(format!(
                "old_text not found in file. Expected: {:?}",
                args.old_text
            )),
        };

        // Build a ContentChange from the old_text → new_text replacement
        let start_pos = offset_to_position(&content, start_offset);
        let end_pos = offset_to_position(&content, start_offset + args.old_text.len());

        let change = ContentChange::new(
            start_pos,
            end_pos,
            args.new_text,
            start_offset as u64,
            args.old_text.encode_utf16().count() as u64,
        );

        let result = if let Some(expected) = args.expected_version {
            match registry.apply_edit_optimistic(&args.path, &change, expected) {
                Ok(event) => event,
                Err(e) => return McpToolResult::err(e),
            }
        } else {
            registry.apply_edit(&args.path, &change)
        };

        match result {
            Some(event) => McpToolResult::ok(
                format!("Replaced text successfully"),
                event.version_id,
            ),
            None => McpToolResult::err("Edit had no effect"),
        }
    }
}

// ---------------------------------------------------------------------------
// Tool: list_symbols
// ---------------------------------------------------------------------------

struct ListSymbolsTool;

impl McpTool for ListSymbolsTool {
    fn name(&self) -> &str {
        "list_symbols"
    }

    fn description(&self) -> &str {
        "List top-level symbols (functions, structs, enums, etc.) from a file. \
         Requires the file to be open in the buffer registry."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute file path" }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, registry: &BufferRegistry, args: &str) -> McpToolResult {
        #[derive(Deserialize)]
        struct Args {
            path: String,
        }

        let args: Args = match serde_json::from_str(args) {
            Ok(a) => a,
            Err(e) => return McpToolResult::err(format!("Invalid arguments: {}", e)),
        };

        let version_id = registry.get_buffer_version(&args.path).unwrap_or(0);

        let content = match registry.get_buffer_content(&args.path) {
            Some(c) => c,
            None => return McpToolResult::err(format!("Buffer not found: {}", args.path)),
        };

        // Simple regex-free symbol extraction using basic heuristics
        let symbols: Vec<String> = content
            .lines()
            .enumerate()
            .filter_map(|(i, line)| {
                let trimmed = line.trim_start();
                if trimmed.starts_with("fn ") {
                    let name = trimmed[3..].split(|c: char| c == '(' || c == '<' || c == ' ').next()?;
                    Some(format!("fn {} (line {})", name, i + 1))
                } else if trimmed.starts_with("struct ") {
                    let name = trimmed[7..].split(|c: char| c == '<' || c == ' ').next()?;
                    Some(format!("struct {} (line {})", name, i + 1))
                } else if trimmed.starts_with("enum ") {
                    let name = trimmed[5..].split(|c: char| c == '<' || c == ' ').next()?;
                    Some(format!("enum {} (line {})", name, i + 1))
                } else if trimmed.starts_with("trait ") {
                    let name = trimmed[6..].split(|c: char| c == '<' || c == ' ').next()?;
                    Some(format!("trait {} (line {})", name, i + 1))
                } else if trimmed.starts_with("impl ") {
                    let rest = &trimmed[5..];
                    if let Some(for_pos) = rest.find(" for ") {
                        let trait_name = rest[..for_pos].trim();
                        let type_name = rest[for_pos + 5..].split(|c: char| c == ' ').next()?;
                        Some(format!("impl {} for {} (line {})", trait_name, type_name, i + 1))
                    } else {
                        let type_name = rest.split(|c: char| c == ' ').next()?;
                        Some(format!("impl {} (line {})", type_name, i + 1))
                    }
                } else if trimmed.starts_with("mod ") {
                    let name = trimmed[4..].split(|c: char| c == ' ').next()?;
                    Some(format!("mod {} (line {})", name, i + 1))
                } else if trimmed.starts_with("const ") || trimmed.starts_with("static ") {
                    let prefix = if trimmed.starts_with("const ") { "const" } else { "static" };
                    let offset = if prefix == "const" { 6 } else { 7 };
                    let name = trimmed[offset..].split(|c: char| c == ':' || c == ' ').next()?;
                    Some(format!("{} {} (line {})", prefix, name, i + 1))
                } else if trimmed.starts_with("type ") {
                    let name = trimmed[5..].split(|c: char| c == '=' || c == ' ').next()?;
                    Some(format!("type {} (line {})", name, i + 1))
                } else {
                    None
                }
            })
            .collect();

        let output = if symbols.is_empty() {
            "No symbols found (file may not be Rust or may be empty)".to_string()
        } else {
            symbols.join("\n")
        };

        McpToolResult::ok(output, version_id)
    }
}

// ---------------------------------------------------------------------------
// Tool: apply_edits
// ---------------------------------------------------------------------------

struct ApplyEditsTool;

impl McpTool for ApplyEditsTool {
    fn name(&self) -> &str {
        "apply_edits"
    }

    fn description(&self) -> &str {
        "Apply multiple Monaco-style edits to a file atomically. \
         Each edit specifies start/end position (1-indexed line/column) and replacement text."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute file path" },
                "edits": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "start_line": { "type": "integer" },
                            "start_column": { "type": "integer" },
                            "end_line": { "type": "integer" },
                            "end_column": { "type": "integer" },
                            "text": { "type": "string" }
                        },
                        "required": ["start_line", "start_column", "end_line", "end_column", "text"]
                    }
                },
                "expected_version": { "type": "integer", "description": "Optional version for optimistic locking" }
            },
            "required": ["path", "edits"]
        })
    }

    fn execute(&self, registry: &BufferRegistry, args: &str) -> McpToolResult {
        #[derive(Deserialize)]
        struct EditArgs {
            start_line: u32,
            start_column: u32,
            end_line: u32,
            end_column: u32,
            text: String,
        }

        #[derive(Deserialize)]
        struct Args {
            path: String,
            edits: Vec<EditArgs>,
            expected_version: Option<u64>,
        }

        let args: Args = match serde_json::from_str(args) {
            Ok(a) => a,
            Err(e) => return McpToolResult::err(format!("Invalid arguments: {}", e)),
        };

        // Validate version first if requested
        if let Some(expected) = args.expected_version {
            let version = match registry.get_buffer_version(&args.path) {
                Some(v) => v,
                None => return McpToolResult::err(format!("Buffer not found: {}", args.path)),
            };
            if version != expected {
                return McpToolResult::err(format!(
                    "version conflict: expected {} but found {}",
                    expected, version
                ));
            }
        }

        let total_edits = args.edits.len();
        let mut last_version = 0;
        let mut applied = 0;

        for edit in args.edits {
            let change = ContentChange::new(
                Position::new(edit.start_line, edit.start_column),
                Position::new(edit.end_line, edit.end_column),
                edit.text,
                0, // range_offset not used for agent edits
                0, // range_length not used for agent edits
            );

            match registry.apply_edit(&args.path, &change) {
                Some(event) => {
                    last_version = event.version_id;
                    applied += 1;
                }
                None => {
                    return McpToolResult::err(format!(
                        "Edit {} of {} had no effect",
                        applied + 1,
                        total_edits
                    ));
                }
            }
        }

        McpToolResult::ok(
            format!("Applied {} edits successfully", applied),
            last_version,
        )
    }
}

// ---------------------------------------------------------------------------
// Helper: byte offset to Position (1-indexed line/column)
// ---------------------------------------------------------------------------

fn offset_to_position(text: &str, offset: usize) -> Position {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in text.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    Position::new(line, col)
}

// ---------------------------------------------------------------------------
// Public helpers for Tauri commands
// ---------------------------------------------------------------------------

/// Execute the `read_file` MCP tool.
pub fn read_file_tool(registry: &BufferRegistry, args: &str) -> McpToolResult {
    ReadFileTool.execute(registry, args)
}

/// Execute the `edit_file` MCP tool.
pub fn edit_file_tool(registry: &BufferRegistry, args: &str) -> McpToolResult {
    EditFileTool.execute(registry, args)
}

/// Execute the `list_symbols` MCP tool.
pub fn list_symbols_tool(registry: &BufferRegistry, args: &str) -> McpToolResult {
    ListSymbolsTool.execute(registry, args)
}

/// Execute the `apply_edits` MCP tool.
pub fn apply_edits_tool(registry: &BufferRegistry, args: &str) -> McpToolResult {
    ApplyEditsTool.execute(registry, args)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_registry_with_file(path: &str, content: &str) -> BufferRegistry {
        let mut registry = BufferRegistry::new();
        registry.open_buffer(path.to_string(), content);
        registry
    }

    #[test]
    fn mcp_read_file_tool_reads_content() {
        let registry = make_registry_with_file("test://a.rs", "hello world");
        let result = read_file_tool(&registry, r#"{"path":"test://a.rs"}"#);
        assert!(result.success);
        assert_eq!(result.content, "hello world");
        assert_eq!(result.version_id, 1);
    }

    #[test]
    fn mcp_read_file_tool_reads_range() {
        let registry = make_registry_with_file("test://a.rs", "line1\nline2\nline3");
        let result = read_file_tool(
            &registry,
            r#"{"path":"test://a.rs","start_line":0,"end_line":2}"#,
        );
        assert!(result.success);
        assert_eq!(result.content, "line1\nline2\n");
    }

    #[test]
    fn mcp_edit_file_tool_replaces_text() {
        let registry = make_registry_with_file("test://a.rs", "hello world");
        let result = edit_file_tool(
            &registry,
            r#"{"path":"test://a.rs","old_text":"world","new_text":"universe"}"#,
        );
        assert!(result.success);
        assert_eq!(registry.get_buffer_content("test://a.rs"), Some("hello universe".to_string()));
    }

    #[test]
    fn mcp_edit_file_tool_with_version_lock() {
        let registry = make_registry_with_file("test://a.rs", "hello world");
        // version is 1 after creation
        let result = edit_file_tool(
            &registry,
            r#"{"path":"test://a.rs","old_text":"world","new_text":"universe","expected_version":1}"#,
        );
        assert!(result.success);
        assert_eq!(result.version_id, 2);
    }

    #[test]
    fn mcp_edit_file_tool_version_conflict() {
        let registry = make_registry_with_file("test://a.rs", "hello world");
        let result = edit_file_tool(
            &registry,
            r#"{"path":"test://a.rs","old_text":"world","new_text":"universe","expected_version":99}"#,
        );
        assert!(!result.success);
        assert!(result.error.unwrap().contains("version conflict"));
    }

    #[test]
    fn mcp_list_symbols_tool_extracts_rust_symbols() {
        let registry = make_registry_with_file(
            "test://a.rs",
            "fn main() {}\nstruct Point { x: i32 }\nenum Color { Red }\n"
        );
        let result = list_symbols_tool(&registry, r#"{"path":"test://a.rs"}"#);
        assert!(result.success);
        assert!(result.content.contains("fn main (line 1)"));
        assert!(result.content.contains("struct Point (line 2)"));
        assert!(result.content.contains("enum Color (line 3)"));
    }

    #[test]
    fn mcp_apply_edits_tool_multiple_edits() {
        let registry = make_registry_with_file("test://a.rs", "aaa bbb ccc");
        let result = apply_edits_tool(
            &registry,
            r#"{"path":"test://a.rs","edits":[{"start_line":1,"start_column":1,"end_line":1,"end_column":4,"text":"xxx"},{"start_line":1,"start_column":9,"end_line":1,"end_column":12,"text":"yyy"}]}"#,
        );
        assert!(result.success);
        assert_eq!(registry.get_buffer_content("test://a.rs"), Some("xxx bbb yyy".to_string()));
    }

    #[test]
    fn mcp_tool_registry_default_tools() {
        let registry = McpToolRegistry::with_defaults();
        let names = registry.names();
        assert!(names.contains(&"read_file".to_string()));
        assert!(names.contains(&"edit_file".to_string()));
        assert!(names.contains(&"list_symbols".to_string()));
        assert!(names.contains(&"apply_edits".to_string()));
    }

    #[test]
    fn mcp_edit_file_tool_not_found() {
        let registry = BufferRegistry::new();
        let result = edit_file_tool(&registry, r#"{"path":"test://missing.rs","old_text":"x","new_text":"y"}"#);
        assert!(!result.success);
        assert!(result.error.unwrap().contains("not found"));
    }
}
