use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

use crate::buffer::{BufferRegistry, ContentChange, Position};
use crate::syntax::{extract_document_symbols, SyntaxParser};

/// The result of executing an MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpToolResult {
    pub success: bool,
    pub content: String,
    pub version_id: u64,
    pub error: Option<String>,
    pub certainty: Option<McpCertainty>,
    pub provenance: Option<McpResultProvenance>,
    pub evidence: Option<serde_json::Value>,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum McpCertainty {
    Observed,
    Derived,
    Heuristic,
    Speculative,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpResultProvenance {
    pub source_kind: String,
    pub path: Option<String>,
    pub version_id: Option<u64>,
    pub producer: String,
}

impl McpToolResult {
    pub fn ok(content: impl Into<String>, version_id: u64) -> Self {
        Self {
            success: true,
            content: content.into(),
            version_id,
            error: None,
            certainty: None,
            provenance: None,
            evidence: None,
            data: None,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            content: String::new(),
            version_id: 0,
            error: Some(message.into()),
            certainty: Some(McpCertainty::Unknown),
            provenance: None,
            evidence: None,
            data: None,
        }
    }

    pub fn with_truth(
        mut self,
        certainty: McpCertainty,
        provenance: McpResultProvenance,
        evidence: serde_json::Value,
        data: Option<serde_json::Value>,
    ) -> Self {
        self.certainty = Some(certainty);
        self.provenance = Some(provenance);
        self.evidence = Some(evidence);
        self.data = data;
        self
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
                write!(
                    f,
                    "Version conflict: expected {} but found {}",
                    expected, actual
                )
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
        registry.register(Box::new(GetBufferMetadataTool));
        registry.register(Box::new(GetBufferSnapshotProofTool));
        registry.register(Box::new(GetSymbolIndexTool));
        registry.register(Box::new(GetSymbolAtPositionTool));
        registry.register(Box::new(GetBufferVersionLineageTool));
        registry
    }
}

fn make_provenance(
    tool_name: &str,
    path: Option<&str>,
    version_id: Option<u64>,
) -> McpResultProvenance {
    McpResultProvenance {
        source_kind: "buffer_registry".to_string(),
        path: path.map(|value| value.to_string()),
        version_id,
        producer: format!("monaco_rust.mcp.{}", tool_name),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push_str(&format!("{:02x}", byte));
    }
    output
}

fn parser_for_path(path: &str) -> Result<SyntaxParser, String> {
    let ext = path.rfind('.').map(|i| &path[i..]);
    match ext {
        Some(".js") | Some(".mjs") | Some(".cjs") => SyntaxParser::for_javascript(),
        Some(".ts") | Some(".mts") | Some(".cts") | Some(".tsx") => SyntaxParser::for_typescript(),
        _ => SyntaxParser::for_rust(),
    }
}

fn symbol_to_json(symbol: &crate::syntax::DocumentSymbol) -> serde_json::Value {
    serde_json::json!({
        "name": symbol.name,
        "detail": symbol.detail,
        "kind": symbol.kind,
        "range": {
            "start_line": symbol.start_line,
            "start_column": symbol.start_column,
            "end_line": symbol.end_line,
            "end_column": symbol.end_column,
        },
        "children": symbol.children.iter().map(symbol_to_json).collect::<Vec<_>>(),
    })
}

fn position_in_symbol(symbol: &crate::syntax::DocumentSymbol, line: u32, column: u32) -> bool {
    let starts_before =
        line > symbol.start_line || (line == symbol.start_line && column >= symbol.start_column);
    let ends_after =
        line < symbol.end_line || (line == symbol.end_line && column <= symbol.end_column);
    starts_before && ends_after
}

fn find_symbol_at_position(
    symbols: &[crate::syntax::DocumentSymbol],
    line: u32,
    column: u32,
) -> Option<&crate::syntax::DocumentSymbol> {
    for symbol in symbols {
        if position_in_symbol(symbol, line, column) {
            if let Some(child) = find_symbol_at_position(&symbol.children, line, column) {
                return Some(child);
            }
            return Some(symbol);
        }
    }
    None
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
            Some(text) => {
                let byte_len = text.len();
                let line_count = text.lines().count();
                McpToolResult::ok(text.clone(), version_id).with_truth(
                    McpCertainty::Observed,
                    make_provenance("read_file", Some(&args.path), Some(version_id)),
                    serde_json::json!({
                        "path": args.path,
                        "version_id": version_id,
                        "line_range": {
                            "start_line": args.start_line,
                            "end_line": args.end_line,
                        },
                        "byte_len": byte_len,
                        "line_count": line_count,
                    }),
                    Some(serde_json::json!({
                        "path": args.path,
                        "content": text,
                    })),
                )
            }
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
            None => {
                return McpToolResult::err(format!(
                    "old_text not found in file. Expected: {:?}",
                    args.old_text
                ))
            }
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
            Some(event) => McpToolResult::ok("Replaced text successfully", event.version_id)
                .with_truth(
                    McpCertainty::Observed,
                    make_provenance("edit_file", Some(&args.path), Some(event.version_id)),
                    serde_json::json!({
                        "path": args.path,
                        "version_id": event.version_id,
                        "change_count": event.changes.len(),
                        "expected_version": args.expected_version,
                        "operation": "replace_exact_text",
                    }),
                    Some(serde_json::json!({
                        "path": args.path,
                        "version_id": event.version_id,
                    })),
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
                if let Some(rest) = trimmed.strip_prefix("fn ") {
                    let name = rest.split(['(', '<', ' ']).next()?;
                    Some(format!("fn {} (line {})", name, i + 1))
                } else if let Some(rest) = trimmed.strip_prefix("struct ") {
                    let name = rest.split(['<', ' ']).next()?;
                    Some(format!("struct {} (line {})", name, i + 1))
                } else if let Some(rest) = trimmed.strip_prefix("enum ") {
                    let name = rest.split(['<', ' ']).next()?;
                    Some(format!("enum {} (line {})", name, i + 1))
                } else if let Some(rest) = trimmed.strip_prefix("trait ") {
                    let name = rest.split(['<', ' ']).next()?;
                    Some(format!("trait {} (line {})", name, i + 1))
                } else if let Some(rest) = trimmed.strip_prefix("impl ") {
                    if let Some(for_pos) = rest.find(" for ") {
                        let trait_name = rest[..for_pos].trim();
                        let type_name = rest[for_pos + 5..].split(' ').next()?;
                        Some(format!(
                            "impl {} for {} (line {})",
                            trait_name,
                            type_name,
                            i + 1
                        ))
                    } else {
                        let type_name = rest.split(' ').next()?;
                        Some(format!("impl {} (line {})", type_name, i + 1))
                    }
                } else if let Some(rest) = trimmed.strip_prefix("mod ") {
                    let name = rest.split(' ').next()?;
                    Some(format!("mod {} (line {})", name, i + 1))
                } else if let Some(rest) = trimmed.strip_prefix("const ") {
                    let name = rest.split([':', ' ']).next()?;
                    Some(format!("const {} (line {})", name, i + 1))
                } else if let Some(rest) = trimmed.strip_prefix("static ") {
                    let name = rest.split([':', ' ']).next()?;
                    Some(format!("static {} (line {})", name, i + 1))
                } else if let Some(rest) = trimmed.strip_prefix("type ") {
                    let name = rest.split(['=', ' ']).next()?;
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

        McpToolResult::ok(output, version_id).with_truth(
            McpCertainty::Heuristic,
            make_provenance("list_symbols", Some(&args.path), Some(version_id)),
            serde_json::json!({
                "path": args.path,
                "version_id": version_id,
                "symbol_count": symbols.len(),
                "extractor": "line_heuristics",
            }),
            Some(serde_json::json!({
                "path": args.path,
                "symbols": symbols,
            })),
        )
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
        .with_truth(
            McpCertainty::Observed,
            make_provenance("apply_edits", Some(&args.path), Some(last_version)),
            serde_json::json!({
                "path": args.path,
                "version_id": last_version,
                "applied_edits": applied,
                "requested_edits": total_edits,
                "expected_version": args.expected_version,
            }),
            Some(serde_json::json!({
                "path": args.path,
                "version_id": last_version,
            })),
        )
    }
}

// ---------------------------------------------------------------------------
// Tool: get_buffer_metadata
// ---------------------------------------------------------------------------

struct GetBufferMetadataTool;

impl McpTool for GetBufferMetadataTool {
    fn name(&self) -> &str {
        "get_buffer_metadata"
    }

    fn description(&self) -> &str {
        "Return exact runtime metadata for an open buffer, including version, dirty state, undo/redo availability, and content length."
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

        let version_id = match registry.get_buffer_version(&args.path) {
            Some(value) => value,
            None => return McpToolResult::err(format!("Buffer not found: {}", args.path)),
        };

        let bytes = match registry.get_buffer_content_bytes(&args.path) {
            Some(value) => value,
            None => return McpToolResult::err(format!("Buffer not found: {}", args.path)),
        };

        let content = match registry.get_buffer_content(&args.path) {
            Some(value) => value,
            None => return McpToolResult::err(format!("Buffer not found: {}", args.path)),
        };

        let dirty = registry.is_buffer_dirty(&args.path).unwrap_or(false);
        let can_undo = registry.can_undo(&args.path).unwrap_or(false);
        let can_redo = registry.can_redo(&args.path).unwrap_or(false);
        let is_open = registry
            .open_resources()
            .iter()
            .any(|resource| resource == &args.path);
        let line_count = content.lines().count();

        let data = serde_json::json!({
            "path": args.path,
            "version_id": version_id,
            "is_dirty": dirty,
            "can_undo": can_undo,
            "can_redo": can_redo,
            "is_open": is_open,
            "byte_len": bytes.len(),
            "line_count": line_count,
            "content_sha256": sha256_hex(&bytes),
        });

        McpToolResult::ok("Retrieved buffer metadata", version_id).with_truth(
            McpCertainty::Observed,
            make_provenance("get_buffer_metadata", Some(&args.path), Some(version_id)),
            data.clone(),
            Some(data),
        )
    }
}

// ---------------------------------------------------------------------------
// Tool: get_buffer_snapshot_proof
// ---------------------------------------------------------------------------

struct GetBufferSnapshotProofTool;

impl McpTool for GetBufferSnapshotProofTool {
    fn name(&self) -> &str {
        "get_buffer_snapshot_proof"
    }

    fn description(&self) -> &str {
        "Return an exact snapshot proof for an open buffer, including content hash, EOL mode, dirty state, and optional exact content."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute file path" },
                "include_content": { "type": "boolean", "description": "Whether to include the exact snapshot content in the response data" }
            },
            "required": ["path"]
        })
    }

    fn execute(&self, registry: &BufferRegistry, args: &str) -> McpToolResult {
        #[derive(Deserialize)]
        struct Args {
            path: String,
            include_content: Option<bool>,
        }

        let args: Args = match serde_json::from_str(args) {
            Ok(a) => a,
            Err(e) => return McpToolResult::err(format!("Invalid arguments: {}", e)),
        };

        let snapshot = match registry.get_buffer_snapshot(&args.path) {
            Some(value) => value,
            None => return McpToolResult::err(format!("Buffer not found: {}", args.path)),
        };

        let content = match String::from_utf8(snapshot.content_utf8.clone()) {
            Ok(value) => value,
            Err(e) => {
                return McpToolResult::err(format!("Buffer content is not valid UTF-8: {}", e))
            }
        };

        let include_content = args.include_content.unwrap_or(true);
        let line_count = content.lines().count();
        let content_sha256 = sha256_hex(&snapshot.content_utf8);

        let evidence = serde_json::json!({
            "path": snapshot.resource,
            "version_id": snapshot.version_id,
            "is_dirty": snapshot.is_dirty,
            "eol": snapshot.eol,
            "byte_len": snapshot.content_utf8.len(),
            "line_count": line_count,
            "content_sha256": content_sha256,
            "include_content": include_content,
        });

        let data = if include_content {
            Some(serde_json::json!({
                "path": args.path,
                "version_id": snapshot.version_id,
                "content": content,
            }))
        } else {
            None
        };

        McpToolResult::ok("Retrieved buffer snapshot proof", snapshot.version_id).with_truth(
            McpCertainty::Observed,
            make_provenance(
                "get_buffer_snapshot_proof",
                Some(&args.path),
                Some(snapshot.version_id),
            ),
            evidence,
            data,
        )
    }
}

// ---------------------------------------------------------------------------
// Tool: get_symbol_index
// ---------------------------------------------------------------------------

struct GetSymbolIndexTool;

impl McpTool for GetSymbolIndexTool {
    fn name(&self) -> &str {
        "get_symbol_index"
    }

    fn description(&self) -> &str {
        "Return exact structured document symbols for an open buffer using Tree-sitter parsing."
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

        let content = match registry.get_buffer_content(&args.path) {
            Some(value) => value,
            None => return McpToolResult::err(format!("Buffer not found: {}", args.path)),
        };

        let version_id = registry.get_buffer_version(&args.path).unwrap_or(0);
        let mut parser = match parser_for_path(&args.path) {
            Ok(value) => value,
            Err(e) => return McpToolResult::err(format!("Parser selection failed: {}", e)),
        };
        let parsed = match parser.parse(&content) {
            Ok(value) => value,
            Err(e) => return McpToolResult::err(format!("Parse failed: {}", e)),
        };

        let symbols = extract_document_symbols(&parsed.tree, &content);
        let symbol_data = symbols.iter().map(symbol_to_json).collect::<Vec<_>>();

        McpToolResult::ok("Retrieved structured symbol index", version_id).with_truth(
            McpCertainty::Observed,
            make_provenance("get_symbol_index", Some(&args.path), Some(version_id)),
            serde_json::json!({
                "path": args.path,
                "version_id": version_id,
                "symbol_count": symbol_data.len(),
                "parser_has_errors": parsed.tree.root_node().has_error(),
                "extractor": "tree_sitter_document_symbols",
            }),
            Some(serde_json::json!({
                "path": args.path,
                "version_id": version_id,
                "symbols": symbol_data,
            })),
        )
    }
}

// ---------------------------------------------------------------------------
// Tool: get_symbol_at_position
// ---------------------------------------------------------------------------

struct GetSymbolAtPositionTool;

impl McpTool for GetSymbolAtPositionTool {
    fn name(&self) -> &str {
        "get_symbol_at_position"
    }

    fn description(&self) -> &str {
        "Return the exact structured symbol that contains a given 1-indexed line and column."
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute file path" },
                "line": { "type": "integer", "description": "1-indexed line number" },
                "column": { "type": "integer", "description": "1-indexed column number" }
            },
            "required": ["path", "line", "column"]
        })
    }

    fn execute(&self, registry: &BufferRegistry, args: &str) -> McpToolResult {
        #[derive(Deserialize)]
        struct Args {
            path: String,
            line: u32,
            column: u32,
        }

        let args: Args = match serde_json::from_str(args) {
            Ok(a) => a,
            Err(e) => return McpToolResult::err(format!("Invalid arguments: {}", e)),
        };

        let content = match registry.get_buffer_content(&args.path) {
            Some(value) => value,
            None => return McpToolResult::err(format!("Buffer not found: {}", args.path)),
        };

        let version_id = registry.get_buffer_version(&args.path).unwrap_or(0);
        let mut parser = match parser_for_path(&args.path) {
            Ok(value) => value,
            Err(e) => return McpToolResult::err(format!("Parser selection failed: {}", e)),
        };
        let parsed = match parser.parse(&content) {
            Ok(value) => value,
            Err(e) => return McpToolResult::err(format!("Parse failed: {}", e)),
        };

        let symbols = extract_document_symbols(&parsed.tree, &content);
        let symbol = match find_symbol_at_position(&symbols, args.line, args.column) {
            Some(value) => value,
            None => {
                return McpToolResult::err(format!(
                    "No symbol found at {}:{} in {}",
                    args.line, args.column, args.path
                ))
            }
        };

        let symbol_json = symbol_to_json(symbol);

        McpToolResult::ok("Retrieved symbol at position", version_id).with_truth(
            McpCertainty::Observed,
            make_provenance("get_symbol_at_position", Some(&args.path), Some(version_id)),
            serde_json::json!({
                "path": args.path,
                "version_id": version_id,
                "line": args.line,
                "column": args.column,
                "parser_has_errors": parsed.tree.root_node().has_error(),
            }),
            Some(serde_json::json!({
                "path": args.path,
                "version_id": version_id,
                "symbol": symbol_json,
            })),
        )
    }
}

// ---------------------------------------------------------------------------
// Tool: get_buffer_version_lineage
// ---------------------------------------------------------------------------

struct GetBufferVersionLineageTool;

impl McpTool for GetBufferVersionLineageTool {
    fn name(&self) -> &str {
        "get_buffer_version_lineage"
    }

    fn description(&self) -> &str {
        "Return the exact current version-lineage posture for an open buffer, including what the runtime can and cannot currently reconstruct."
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

        let version_id = match registry.get_buffer_version(&args.path) {
            Some(value) => value,
            None => return McpToolResult::err(format!("Buffer not found: {}", args.path)),
        };
        let bytes = match registry.get_buffer_content_bytes(&args.path) {
            Some(value) => value,
            None => return McpToolResult::err(format!("Buffer not found: {}", args.path)),
        };

        let dirty = registry.is_buffer_dirty(&args.path).unwrap_or(false);
        let can_undo = registry.can_undo(&args.path).unwrap_or(false);
        let can_redo = registry.can_redo(&args.path).unwrap_or(false);

        let data = serde_json::json!({
            "path": args.path,
            "current_version_id": version_id,
            "content_sha256": sha256_hex(&bytes),
            "is_dirty": dirty,
            "can_undo": can_undo,
            "can_redo": can_redo,
            "lineage_model": "monotonic_version_counter",
            "lineage_capabilities": {
                "historical_versions_stored": false,
                "exact_prior_diff_reconstructable": false,
                "snapshot_proof_available_for_current_version": true,
                "undo_redo_affordance_available": true,
            },
        });

        McpToolResult::ok("Retrieved current buffer lineage posture", version_id).with_truth(
            McpCertainty::Observed,
            make_provenance(
                "get_buffer_version_lineage",
                Some(&args.path),
                Some(version_id),
            ),
            data.clone(),
            Some(data),
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

/// Execute the `get_buffer_metadata` MCP tool.
pub fn get_buffer_metadata_tool(registry: &BufferRegistry, args: &str) -> McpToolResult {
    GetBufferMetadataTool.execute(registry, args)
}

/// Execute the `get_buffer_snapshot_proof` MCP tool.
pub fn get_buffer_snapshot_proof_tool(registry: &BufferRegistry, args: &str) -> McpToolResult {
    GetBufferSnapshotProofTool.execute(registry, args)
}

/// Execute the `get_symbol_index` MCP tool.
pub fn get_symbol_index_tool(registry: &BufferRegistry, args: &str) -> McpToolResult {
    GetSymbolIndexTool.execute(registry, args)
}

/// Execute the `get_symbol_at_position` MCP tool.
pub fn get_symbol_at_position_tool(registry: &BufferRegistry, args: &str) -> McpToolResult {
    GetSymbolAtPositionTool.execute(registry, args)
}

/// Execute the `get_buffer_version_lineage` MCP tool.
pub fn get_buffer_version_lineage_tool(registry: &BufferRegistry, args: &str) -> McpToolResult {
    GetBufferVersionLineageTool.execute(registry, args)
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
        assert_eq!(result.certainty, Some(McpCertainty::Observed));
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
        assert_eq!(
            registry.get_buffer_content("test://a.rs"),
            Some("hello universe".to_string())
        );
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
            "fn main() {}\nstruct Point { x: i32 }\nenum Color { Red }\n",
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
        assert_eq!(
            registry.get_buffer_content("test://a.rs"),
            Some("xxx bbb yyy".to_string())
        );
    }

    #[test]
    fn mcp_get_buffer_metadata_tool_returns_exact_runtime_facts() {
        let registry = make_registry_with_file("test://a.rs", "fn main() {}\n");
        let result = get_buffer_metadata_tool(&registry, r#"{"path":"test://a.rs"}"#);
        assert!(result.success);
        assert_eq!(result.certainty, Some(McpCertainty::Observed));
        let data = result.data.expect("expected metadata data");
        assert_eq!(data["path"], "test://a.rs");
        assert_eq!(data["version_id"], 1);
        assert_eq!(data["is_dirty"], false);
        assert_eq!(data["can_undo"], false);
        assert_eq!(data["can_redo"], false);
        assert_eq!(data["is_open"], true);
    }

    #[test]
    fn mcp_get_buffer_snapshot_proof_tool_returns_hash_and_content() {
        let registry = make_registry_with_file("test://a.rs", "hello\nworld\n");
        let result = get_buffer_snapshot_proof_tool(&registry, r#"{"path":"test://a.rs"}"#);
        assert!(result.success);
        assert_eq!(result.certainty, Some(McpCertainty::Observed));
        let evidence = result.evidence.expect("expected snapshot evidence");
        assert_eq!(evidence["path"], "test://a.rs");
        assert_eq!(evidence["version_id"], 1);
        assert_eq!(
            evidence["content_sha256"],
            "4a1e67f2fe1d1cc7b31d0ca2ec441da4778203a036a77da10344c85e24ff0f92"
        );
        let data = result.data.expect("expected snapshot content");
        assert_eq!(data["content"], "hello\nworld\n");
    }

    #[test]
    fn mcp_get_buffer_snapshot_proof_tool_can_omit_content() {
        let registry = make_registry_with_file("test://a.rs", "hello\nworld\n");
        let result = get_buffer_snapshot_proof_tool(
            &registry,
            r#"{"path":"test://a.rs","include_content":false}"#,
        );
        assert!(result.success);
        let evidence = result.evidence.expect("expected snapshot evidence");
        assert_eq!(evidence["include_content"], false);
        assert!(result.data.is_none());
    }

    #[test]
    fn mcp_get_buffer_metadata_tool_reports_dirty_and_undo_state() {
        let registry = make_registry_with_file("test://a.rs", "hello world");
        let edit = edit_file_tool(
            &registry,
            r#"{"path":"test://a.rs","old_text":"world","new_text":"rust"}"#,
        );
        assert!(edit.success);

        let result = get_buffer_metadata_tool(&registry, r#"{"path":"test://a.rs"}"#);
        assert!(result.success);
        let data = result.data.expect("expected metadata data");
        assert_eq!(data["is_dirty"], true);
        assert_eq!(data["can_undo"], true);
        assert_eq!(data["can_redo"], false);
    }

    #[test]
    fn mcp_get_symbol_index_tool_returns_structured_symbols() {
        let registry =
            make_registry_with_file("test://a.rs", "struct Point { x: i32 }\nfn main() {}\n");
        let result = get_symbol_index_tool(&registry, r#"{"path":"test://a.rs"}"#);
        assert!(result.success);
        assert_eq!(result.certainty, Some(McpCertainty::Observed));
        let data = result.data.expect("expected symbol data");
        let symbols = data["symbols"].as_array().expect("symbols array");
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0]["name"], "Point");
        assert_eq!(symbols[0]["kind"], "struct");
        assert_eq!(symbols[1]["name"], "main");
        assert_eq!(symbols[1]["kind"], "function");
    }

    #[test]
    fn mcp_get_symbol_index_tool_uses_typescript_parser_for_ts_paths() {
        let registry = make_registry_with_file(
            "test://a.ts",
            "interface Point { x: number; }\nfunction main() { return 1; }\n",
        );
        let result = get_symbol_index_tool(&registry, r#"{"path":"test://a.ts"}"#);
        assert!(result.success);
        let evidence = result.evidence.expect("expected symbol evidence");
        assert_eq!(evidence["parser_has_errors"], false);
        let data = result.data.expect("expected symbol data");
        let symbols = data["symbols"].as_array().expect("symbols array");
        assert!(symbols.iter().any(|s| s["name"] == "Point"));
        assert!(symbols.iter().any(|s| s["name"] == "main"));
    }

    #[test]
    fn mcp_get_symbol_at_position_tool_returns_deepest_symbol() {
        let registry =
            make_registry_with_file("test://a.rs", "struct Point { x: i32 }\nfn main() {}\n");
        let result = get_symbol_at_position_tool(
            &registry,
            r#"{"path":"test://a.rs","line":1,"column":16}"#,
        );
        assert!(result.success);
        assert_eq!(result.certainty, Some(McpCertainty::Observed));
        let data = result.data.expect("expected symbol data");
        assert_eq!(data["symbol"]["name"], "x");
        assert_eq!(data["symbol"]["kind"], "property");
    }

    #[test]
    fn mcp_get_symbol_at_position_tool_reports_missing_symbol() {
        let registry = make_registry_with_file("test://a.rs", "fn main() {}\n");
        let result =
            get_symbol_at_position_tool(&registry, r#"{"path":"test://a.rs","line":3,"column":1}"#);
        assert!(!result.success);
        assert!(result
            .error
            .expect("missing symbol error")
            .contains("No symbol found"));
    }

    #[test]
    fn mcp_get_buffer_version_lineage_tool_reports_current_boundary() {
        let registry = make_registry_with_file("test://a.rs", "fn main() {}\n");
        let result = get_buffer_version_lineage_tool(&registry, r#"{"path":"test://a.rs"}"#);
        assert!(result.success);
        assert_eq!(result.certainty, Some(McpCertainty::Observed));
        let data = result.data.expect("expected lineage data");
        assert_eq!(data["current_version_id"], 1);
        assert_eq!(data["lineage_model"], "monotonic_version_counter");
        assert_eq!(
            data["lineage_capabilities"]["historical_versions_stored"],
            false
        );
        assert_eq!(
            data["lineage_capabilities"]["snapshot_proof_available_for_current_version"],
            true
        );
    }

    #[test]
    fn mcp_tool_registry_default_tools() {
        let registry = McpToolRegistry::with_defaults();
        let names = registry.names();
        assert!(names.contains(&"read_file".to_string()));
        assert!(names.contains(&"edit_file".to_string()));
        assert!(names.contains(&"list_symbols".to_string()));
        assert!(names.contains(&"apply_edits".to_string()));
        assert!(names.contains(&"get_buffer_metadata".to_string()));
        assert!(names.contains(&"get_buffer_snapshot_proof".to_string()));
        assert!(names.contains(&"get_symbol_index".to_string()));
        assert!(names.contains(&"get_symbol_at_position".to_string()));
        assert!(names.contains(&"get_buffer_version_lineage".to_string()));
    }

    #[test]
    fn mcp_edit_file_tool_not_found() {
        let registry = BufferRegistry::new();
        let result = edit_file_tool(
            &registry,
            r#"{"path":"test://missing.rs","old_text":"x","new_text":"y"}"#,
        );
        assert!(!result.success);
        assert!(result.error.unwrap().contains("not found"));
    }
}
