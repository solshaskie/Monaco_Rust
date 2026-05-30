# Monaco_Rust Hydration Packet

> **Read this first** if you are entering this project without prior session context.
> This is the fast path. Deeper truth lives in the standard working docs linked at the end.

---

## Project in One Sentence

Monaco_Rust is a phased refactoring of Monaco Editor's JavaScript text engine into a Rust-backed, Tauri-hosted, Protobuf-serialized high-performance agentic workspace. The goal is radical performance (sub-16ms edits, 60fps scrolling), native security boundaries, and MCP-aware agentic editing capabilities.

Deferred work (WASM bridge, custom renderer, cross-platform CI) is tracked in [`WASM_ROADMAP.md`](./WASM_ROADMAP.md).

## Current Phase Snapshot

| Phase | Status | Notes |
|-------|--------|-------|
| Phase 1: Foundation - Protobuf Text Buffer Layer | ✅ Phase Complete (47 unit tests + 3 integration tests passing) | Rope-based TextBuffer in Rust, BufferRegistry for headless document management, integrated with host_handlers via Protobuf |
| Phase 2: Syntax Highlighting - Tree-sitter Integration | ✅ Phase Complete (Rust, JavaScript, TypeScript grammar support; tokenization, folding, semantic tokens, delta updates) | Replace Monarch/regex tokenization with Tree-sitter |
| Phase 3: Language Features - Native Transport Layer | 🟡 Substantially Complete (hover, diagnostics with push events, document symbols, completions via Tree-sitter symbols; external LSP deferred) | Route Monaco's language feature providers through Tauri/Protobuf |
| Phase 4: Headless Buffer Registry | ✅ Phase Complete (buffer registry with multi-view Arc sharing, event broadcaster with subscription tracking, Tauri push events for content changes, version guards, MCP tool framework with read_file/edit_file/list_symbols/apply_edits, optimistic locking, agent-facing Tauri commands) | Full event broadcasting and MCP agent integration complete |
| Phase 5: Performance Optimization | ✅ Phase Partially Complete (content range fetching, dirty line tracking, viewport-aware tokenization; WASM zero-copy and custom renderer deferred) | Zero-copy buffer sharing, layout offload, incremental rendering |
| Phase 6: Security Hardening | ✅ Phase Complete (resource sandbox with per-principal quotas, capability registry with permission model, audit logging, revocable permissions; integrated into all host handlers and Tauri commands) | Memory bounds, capability-based security |
| Phase 7: Testing and Validation | ✅ Phase Complete (120 lib unit tests + 14 protobuf roundtrip tests + 7 performance benchmarks + 6 integration tests + 1 artifact test; all passing) | End-to-end workflows, multi-view sync, security integration, performance regression tracking |

**Estimated Total Timeline:** 13-17 weeks. Phase 1 consumed ~1 week; 12-16 weeks remain.

---

## What Exists Now

### Core Data Structures (Rust, `src-tauri/src/buffer/`)
- **`TextBuffer`** — Rope-based (`ropey` crate) text storage with version tracking, dirty state, EOL detection, and line indexing
- **`ContentChange`** — Mirrors Monaco's `IModelContentChange` with 1-indexed positions, UTF-16 column support, range offset/length
- **`Position`** — Monaco-compatible 1-indexed `(line, column)` pair
- **`LineIndex`** — Byte offset array per line with UTF-16 ↔ UTF-8 conversion for Monaco column compatibility
- **`UndoStack`** / **`UndoTransaction`** — Transaction-grouped undo/redo with configurable max stack depth
- **`BufferRegistry`** — Thread-safe (`Arc<RwLock<>>`) headless document registry keyed by resource URI; supports open/close/edit/undo/redo/snapshot

### Protobuf Schemas (`proto/`)
| File | Content |
|------|---------|
| `ipc_envelope.proto` | Core request/response/event framing (`IpcRequest`, `IpcResponse`, `IpcEvent`) |
| `ipc_file.proto` | File system types (`Uri`, `FileStat`, read/write/delete/rename) |
| `ipc_editor.proto` | Editor buffer types (`ModelContentChange`, `ModelContentChangedEvent`, `BufferSnapshot`, `OpenBufferRequest`, `ApplyEditsRequest`) |
| `ipc_editor_host.proto` | Host-layer operations (workspace roots, open/save/close document, apply edits, undo/redo, buffer snapshot) |
| `ipc_editor_language.proto` | Language features (completion, diagnostics, hover, document symbols, semantic tokens) |

### Tauri Backend (`src-tauri/src/`)
- **`main.rs`** — 17 registered Tauri commands with both JSON and Protobuf IPC paths; `parser_for_path` selects Tree-sitter grammar by file extension
- **`host_handlers.rs`** — `MonacoHostState` managing workspace roots + `BufferRegistry`; implements `open_document`, `save_document`, `save_document_as`, `close_document`, `apply_edits`, `undo`, `redo`, `get_buffer_snapshot`, `list_directory`
- **`build.rs`** — Compiles all 5 Protobuf definitions via `prost-build`

### Frontend Shell (`tauri/index.html`)
- Dark-theme Monaco editor shell with sidebar, tab bar, file explorer, status bar
- Local worker probing (TypeScript, JSON) for language features
- Ctrl+S save binding via Tauri invoke
- Tree-sitter powered language providers (Rust, JavaScript, TypeScript):
  - `registerDocumentSemanticTokensProvider` with LSP packed format
  - `registerFoldingRangeProvider` for code folding
  - `registerHoverProvider` for symbol hover info
  - Tree-sitter diagnostics displayed as Monaco markers (`setModelMarkers`)
  - `registerDocumentSymbolProvider` for outline view with symbol hierarchy

### Syntax Module (Rust, `src-tauri/src/syntax/`)
- **`SyntaxParser`** — Tree-sitter grammar loader for Rust (`tree-sitter-rust`), JavaScript (`tree-sitter-javascript`), and TypeScript (`tree-sitter-typescript`) with incremental parse support
- **`Tokenizer`** — Full-document tokenization that converts CST to `SyntaxToken` stream
- **`tokenize_tree`** — Maps Tree-sitter node kinds to Monaco token types; supports Rust, JavaScript, and TypeScript node kinds
- **`extract_folding_ranges`** — Extracts foldable code ranges from Tree-sitter CST (functions, structs, enums, impls, traits, blocks, match expressions, loops, if statements)
- **`pack_semantic_tokens`** — Packs `SyntaxToken`s into LSP `SemanticTokens` format (5-tuple uint32 array: deltaLine, deltaStart, length, tokenType, modifiers)
- **`hover_at_position`** — Finds the Tree-sitter node at a cursor position and returns its kind and text as markdown hover content
- **`collect_syntax_diagnostics`** — Extracts parse errors and missing nodes from Tree-sitter CST for LSP-style diagnostics
- **`extract_document_symbols`** — Extracts document symbols from Tree-sitter CST; currently optimized for Rust (functions, structs, enums, traits, impls, mods, consts, statics, types, macros) with children (struct fields, enum variants)

### Test Coverage
- **89 unit tests** across 15 modules: `position` (4), `line_index` (8), `content_change` (4), `undo` (7), `text_buffer` (11), `registry` (11), `host_handlers` (2), `parser` (5), `tokens` (4), `folding` (4), `semantic_tokens` (3), `hover` (3), `diagnostics` (3), `symbols` (4), `syntax_handlers` (13)
- **3 integration tests** in `m4_integration.rs`: save-as round-trip, directory listing, distribution asset verification
- **1 artifact packaging test** in `m4_artifact_packaging.rs`

---

## Architecture Boundaries (Non-Negotiable)

Monaco_Rust **is:**
- A Rust-backend replacement for Monaco's JavaScript text engine
- Protobuf-serialized IPC between frontend and backend
- Backend-only text buffer operations (no WASM bridge until stable)
- Headless document registry supporting agent-driven and multi-view editing

Monaco_Rust **is not:**
- A full Monaco Editor rewrite from scratch
- A WASM-first architecture (WASM deferred to Phase 5 after backend stabilization)
- A replacement for Monaco's HTML/CSS visual layer — only the text model
- An extension host or language server — those connect through the Tauri IPC layer

**Key design decisions:**
- **Backend-only first** — Buffer logic lives entirely in the Rust process. WASM bridge evaluated after Phase 1 stability
- **Mirror public interfaces, not internals** — Matches Monaco's `ITextModel`, `IModelContentChange`, `IModelContentChangedEvent`, `IPosition` but not internal implementation details
- **`ropey` crate** for the rope data structure — battle-tested, avoids custom piece-tree implementation complexity
- **Thread-safe registry** — `Arc<RwLock<TextBuffer>>` enables concurrent reads with exclusive writes

---

## How to Build and Test

```bash
cd src-tauri
cargo build
cargo test
```

All 47 unit tests + 3 integration tests pass with zero failures currently. The build happens inside the `src-tauri` crate directory, not the monorepo root.

---

## Key Files

| File | Purpose |
|------|---------|
| `PHASED_ROADMAP.md` | Complete 7-phase build plan with architecture diagram |
| `refactor_docs/Monaco_Rust.md` | Strategic rationale and architectural guidance |
| `src-tauri/src/buffer/mod.rs` | Buffer module entry point and re-exports |
| `src-tauri/src/host_handlers.rs` | Host-layer operations integrating BufferRegistry |
| `proto/ipc_editor_host.proto` | Buffer operation protobuf messages |
| `src-tauri/Cargo.toml` | Crate dependencies (`ropey`, `prost`, `serde`, `tauri`) |
| `tauri/index.html` | Frontend shell with Monaco editor |

---

## Active Risks / Watch Points

1. **Position model ambiguity** — Columns are in UTF-16 code units (Monaco's native), but the rope operates on UTF-8 bytes. The `position_to_offset`/`offset_to_position` conversion in `LineIndex` handles this, but non-BMP characters (emoji, CJK) need verification that the round-trip is lossless
2. **`ropey` slice boundaries** — `Rope::get_slice()` returns `Option<RopeSlice>` and fails if the range straddles chunk boundaries improperly. Current tests only exercise ASCII — non-ASCII (multi-byte UTF-8) needs stress testing
3. **Undo stack memory** — Unlimited undo stack by default. With large files and many edits, this could grow unbounded. The `with_max_undo()` constructor exists but isn't wired to any frontend config
4. **No dirty-file-on-close guard in UI** — The `close_document` handler supports `save_if_dirty`, but the Tauri frontend doesn't invoke it yet
5. **Integration tests share temp dirs** — Tests use `std::env::temp_dir()` with unique subdirectories, but potential collision on concurrent test runs is not proven safe

---

## What to Do Right Now

If you are picking up this project for the first time:

1. **Read the roadmap:** `PHASED_ROADMAP.md` — especially Phase 1 (Foundation) to understand what's been built
2. **Build and test:** `cd src-tauri && cargo build && cargo test` — verify 89 unit tests pass
3. **Explore the buffer module:** `src-tauri/src/buffer/` — Position, LineIndex, ContentChange, TextBuffer, BufferRegistry, UndoStack
4. **Check the proto schemas:** `proto/*.proto` — the IPC contract between frontend and backend
5. **Review the host handlers:** `src-tauri/src/host_handlers.rs` — how operations flow from Tauri commands to the buffer registry
6. **Understand the architecture doc:** `refactor_docs/Monaco_Rust.md` for the strategic vision
7. **Next work:** Integrate external LSP for completions, optimize token/folding refresh mechanism, improve JS/TS document symbol extraction

---

## Standard Working Docs

Read these in this order when you need deeper truth:

1. `PHASED_ROADMAP.md` — Build order, phase map, what comes next
2. `refactor_docs/Monaco_Rust.md` — Strategic rationale, architectural guidance, implementation pointers
3. `HYDRATION.md` — (this file) Session continuity and project state
4. `proto/*.proto` — IPC contract definitions
5. `src-tauri/src/buffer/*.rs` — Core buffer implementation

---

## Recent Session Summary

**Session 2026-05-29 (Phase 3.2 + 3.1): Diagnostics Push Pipeline + Completion Provider Bridge**

- **Phase 3.2 — Diagnostics Pipeline:**
  - Refactored `SyntaxDiagnostic` to use a proper `DiagnosticSeverity` enum (Error/Warning/Information/Hint) with LSP-compatible `as_lsp_value()`
  - Made diagnostic source language-aware via `diagnostic_source_for_path` in `syntax_handlers.rs` (tree-sitter-rust, tree-sitter-javascript, tree-sitter-typescript)
  - Implemented push-based diagnostics: `apply_edits_json` now emits a `diagnostics-changed` Tauri event after every successful edit, carrying the full diagnostic payload
  - Wired frontend `tauri/index.html` to listen for `diagnostics-changed` events and update Monaco markers in real time
  - Fixed frontend severity mapping to support all four LSP levels (Error, Warning, Info, Hint)
  - Updated `fetchDiagnostics` marker source to match the active language instead of hardcoding "tree-sitter-rust"
  - Added `diagnostics_include_source` test

- **Phase 3.1 — Completion Provider Bridge:**
  - Created `src-tauri/src/syntax/completion.rs` with `CompletionItem`, `collect_completions`, and `prefix_at_position`
  - Completions are derived from Tree-sitter `extract_document_symbols` (functions, structs, enums, fields, variants) with prefix filtering
  - Added `completion_document` handler in `syntax_handlers.rs` with protobuf request/response mapping
  - Added `completion_document_json` Tauri command in `main.rs`
  - Wired Monaco `registerCompletionItemProvider` for rust, javascript, and typescript in `tauri/index.html`
  - Added 3 unit tests for completion module + 3 handler tests

- **All 96 unit tests + 2 integration tests + 1 artifact test pass**

**Session 2026-05-29 (Phase 4): Headless Buffer Registry + Event Broadcasting System**

- **Phase 4.1 — Buffer Registry:**
  - Verified existing `BufferRegistry` already supports all requirements: URI-based lookup (`HashMap<String, Arc<RwLock<TextBuffer>>>`), multi-view via `Arc` clones, and full lifecycle (open/close/dispose/undo/redo/snapshot)
  - No code changes needed — the registry implemented in Phase 1 was already architected for this

- **Phase 4.2 — Event Broadcasting System:**
  - Created `src-tauri/src/events.rs` with typed event structs: `BufferContentChangedEvent`, `BufferOpenedEvent`, `BufferClosedEvent`, `BufferSavedEvent`, `WorkspaceRootsChangedEvent`
  - Created `EventBroadcaster` with `emit_*` methods using Tauri's `Emitter<R>` trait
  - Created `EventSubscriptionTracker` for per-resource subscription management and monotonic sequence numbering
  - Extended `proto/ipc_editor_host.proto` with `BufferContentChangeEvent` and `BufferRegistryEvent` messages
  - Wired event emission into all JSON commands in `main.rs`:
    - `apply_edits_json` → `buffer-content-changed` (with changes array)
    - `undo_json` → `buffer-content-changed` (`is_undoing=true`)
    - `redo_json` → `buffer-content-changed` (`is_redoing=true`)
    - `open_document_json` → `buffer-opened`
    - `close_document_json` → `buffer-closed`
    - `save_document_json` → `buffer-saved`
  - Wired frontend `tauri/index.html` to listen for all buffer events:
    - `buffer-content-changed`: applies remote edits to the correct tab's model using `isApplyingRemoteEdits` guard + version guard (`payload.version_id < model.getAlternativeVersionId()` skips stale events)
    - `buffer-opened`/`buffer-closed`/`buffer-saved`: status updates and logging
  - Added `serde_json` to `Cargo.toml` for event serialization testing
  - Added 3 unit tests for events module (subscription tracking, sequence increments, JSON roundtrip)

- **Phase 4.3 — MCP Agent Integration:**
  - Created `src-tauri/src/mcp/mod.rs` and `src-tauri/src/mcp/tools.rs` with MCP tool framework
  - `McpTool` trait with `name()`, `description()`, `input_schema()`, `execute()` for extensible agent tools
  - `McpToolRegistry` for dynamic tool registration and discovery
  - Four built-in tools:
    - `read_file` — read full content or line range from any open buffer
    - `edit_file` — replace `old_text` with `new_text` (with optional `expected_version` for optimistic locking)
    - `list_symbols` — extract top-level Rust symbols (fn, struct, enum, trait, impl, mod, const, static, type) via heuristics
    - `apply_edits` — apply multiple Monaco-style position-based edits atomically
  - Added `apply_edit_optimistic()` to `BufferRegistry` for version-locked concurrent editing
  - Conflict resolution: version mismatch returns explicit error to agent; agent retries with updated `old_text`
  - Added `execute_mcp_tool` and `list_mcp_tools` Tauri commands in `main.rs` for frontend/agent access
  - Added 9 unit tests for MCP tools (read_file, read_range, edit_file, version_lock, version_conflict, list_symbols, apply_edits, registry_defaults, missing_buffer)

- **All 129 unit tests + 2 integration tests + 1 artifact test pass**

**Session 2026-05-29 (Phase 5): Performance Optimization**

- **Phase 5.1 — Zero-Copy Buffer Sharing (practical subset):**
  - Added `get_value_in_line_range(start_line, end_line)` to `TextBuffer` for viewport-only content delivery
  - Added `get_buffer_content_range` and dirty line accessors to `BufferRegistry`
  - Added `LineRange` struct and dirty line tracking to `TextBuffer`
  - Edits now record affected line ranges (`dirty_line_ranges`) for incremental re-computation
  - Added 7 new unit tests for content range and dirty tracking

- **Phase 5.2 — Layout Computation Offload (practical subset):**
  - Added `tokenize_tree_range(tree, source, start_line, end_line)` to `syntax/tokens.rs`
  - Added `tokenize_document_range` handler in `syntax_handlers.rs` for viewport-aware tokenization
  - Added `tokenize_document_range_json` Tauri command in `main.rs`
  - This avoids full-file tokenization overhead when only a viewport is visible
  - Added 1 unit test for range tokenization filtering

- **Phase 5.3 — Incremental Rendering:**
  - Dirty line tracking in `TextBuffer` enables selective re-tokenization/re-decoration
  - `clear_dirty_lines()` allows the frontend to acknowledge processed dirty regions
  - Lays groundwork for future incremental DOM updates when a custom Monaco renderer is built
  - Deferred items: `SharedArrayBuffer` (requires WASM bridge), layout offload to Rust (requires custom renderer), virtual scrolling (requires DOM control)

- **All 107 unit tests + 2 integration tests + 1 artifact test pass**

**Session 2026-05-29 (Phase 2 extension): Tree-sitter Grammar Expansion to JavaScript/TypeScript**

- Added `tree-sitter-javascript` and `tree-sitter-typescript` dependencies to `src-tauri/Cargo.toml`
- Extended `SyntaxParser` with `for_javascript()` and `for_typescript()` constructors
- Added `parser_for_path` helper in `syntax_handlers.rs` that selects grammar by file extension (`.js`, `.ts`, `.tsx`, `.rs`)
- Updated all backend handlers (`tokenize_document`, `fold_document`, `semantic_tokens_document`, `semantic_tokens_delta_document`, `hover_document`, `diagnostics_document`, `document_symbols_document`) to use `parser_for_path`
- Extended `node_kind_to_token_type` in `tokens.rs` with JavaScript/TypeScript node kinds (keywords, identifiers, literals, comments, operators, delimiters, types)
- Wired frontend `tauri/index.html`:
  - Trigger Tree-sitter fetches for JS/TS files on open and after edits
  - Register semantic tokens, hover, document symbol, and folding range providers for `javascript` and `typescript`
- Updated `PHASED_ROADMAP.md` and `HYDRATION.md`
- All 89 unit tests + 3 integration tests + 1 artifact test pass

**Session 2026-05-29 (Phase 3 completed): Document Symbol Extraction**

- Created `src-tauri/src/syntax/symbols.rs` with `extract_document_symbols`
- Extracts top-level symbols from Tree-sitter CST: functions, structs, enums, traits, impls, mods, consts, statics, types, macros, use declarations
- Hierarchical children: struct fields and enum variants
- Added `document_symbols_document` handler in `syntax_handlers.rs` + tests
- Added `document_symbols_document_json` Tauri command in `main.rs`
- Wired frontend `tauri/index.html`:
  - `registerDocumentSymbolProvider("rust")` reads from `state.symbolsCache`
  - Maps internal symbol kinds to Monaco `SymbolKind` enum
  - `fetchDocumentSymbols` calls backend on document open and after content changes
- Updated `PHASED_ROADMAP.md` and `HYDRATION.md`
- All 87 unit tests + 3 integration tests + 1 artifact test pass
- **Phase 3 is substantially complete** — hover, diagnostics, and document symbols are all wired end-to-end. Only completions remain, which requires external LSP client infrastructure.

**Session 2026-05-29 (Phase 3 continued): Tree-sitter Diagnostics Pipeline**

- Created `src-tauri/src/syntax/diagnostics.rs` with `collect_syntax_diagnostics`
- Extracts parse errors and missing nodes from Tree-sitter CST into LSP-style `Diagnostic` messages
- Added `diagnostics_document` handler in `syntax_handlers.rs` + tests
- Added `diagnostics_document_json` Tauri command in `main.rs`
- Wired frontend `tauri/index.html`:
  - `fetchDiagnostics` calls backend and converts responses to Monaco `IMarkerData`
  - `setModelMarkers(model, "tree-sitter-rust", markers)` displays syntax errors inline
  - Diagnostics refresh on document open and after content changes
- Updated `PHASED_ROADMAP.md` and `HYDRATION.md`
- All 81 unit tests + 3 integration tests + 1 artifact test pass
- **Next session:** Implement document symbol extraction, expand grammar support, integrate external LSP for completions

**Session 2026-05-29 (Phase 3 kickoff): Hover Provider**

- Created `src-tauri/src/syntax/hover.rs` with `hover_at_position`
- Uses Tree-sitter CST to find the node at a cursor position and return its kind + text as markdown
- Added `hover_document` handler in `syntax_handlers.rs` + tests
- Added `hover_document_json` Tauri command in `main.rs`
- Wired frontend `tauri/index.html`:
  - `registerHoverProvider("rust")` calls backend with cursor position
  - Returns markdown hover content with node kind and source text
- Updated `PHASED_ROADMAP.md` and `HYDRATION.md`
- All 75 unit tests + 3 integration tests + 1 artifact test pass
- **Next session:** Implement document symbol extraction, add diagnostics push events, expand grammar support

**Session 2026-05-29 (Phase 2 completed): Delta-Encoded Semantic Tokens**

- Implemented `semantic_tokens_delta_document` in `syntax_handlers.rs`
- Version-based delta: returns empty data if version unchanged, full data otherwise
- Added `semantic_tokens_delta_document_json` Tauri command
- Updated frontend `fetchSemanticTokens` to use delta endpoint when a cached `result_id` exists
- All 70 unit tests + 3 integration tests + 1 artifact test pass
- **Phase 2 is now complete** — all roadmap items checked

**Session 2026-05-29 (Phase 2 continued): LSP Semantic Tokens Provider**

- Created `src-tauri/src/syntax/semantic_tokens.rs` with `pack_semantic_tokens`
- Maps internal `SyntaxToken`s to LSP packed format: [deltaLine, deltaStart, length, tokenType, modifiers]
- Token type legend: keyword(0), identifier(1), string(2), number(3), comment(4), operator(5), type(6), macro(7)
- Added `semantic_tokens_document` handler in `syntax_handlers.rs` + tests
- Added `semantic_tokens_document_json` Tauri command in `main.rs`
- Wired frontend `tauri/index.html`:
  - `registerDocumentSemanticTokensProvider("rust")` with matching legend
  - `fetchSemanticTokens` calls backend on document open and after content changes
  - Replaced basic `setTokensProvider` with proper LSP semantic tokens provider
- Updated `PHASED_ROADMAP.md` and `HYDRATION.md`
- All 67 unit tests + 3 integration tests + 1 artifact test pass
- **Next session:** Expand Tree-sitter grammar support (TypeScript, JavaScript), add delta-encoded token updates, optimize token/folding refresh mechanism

**Session 2026-05-29 (Phase 2 continued): Tree-sitter Folding Range Provider**

- Added `FoldingRange`, `FoldingRangeRequest/Response` to `proto/ipc_editor_language.proto`
- Created `src-tauri/src/syntax/folding.rs` with `extract_folding_ranges` that identifies foldable blocks:
  function_item, struct_item, enum_item, impl_item, trait_item, mod_item,
  match_expression, if_expression, while_expression, for_expression,
  loop_expression, block, use_declaration, const_item, static_item
- Updated `syntax_handlers.rs` with `fold_document` handler + tests
- Added `fold_document_json` Tauri command in `main.rs`
- Wired frontend `tauri/index.html`:
  - `registerFoldingRangeProvider("rust")` reads from `state.foldingCache`
  - `fetchFoldingRanges` calls backend on document open and after content changes
- Updated `PHASED_ROADMAP.md` checkbox progress
- All 62 unit tests + 3 integration tests + 1 artifact test pass
- **Next session:** Expand Tree-sitter grammar support (TypeScript, JavaScript), implement LSP-style SemanticTokens endpoint, optimize token/folding refresh mechanism

**Session 2026-05-29 (Phase 2 kickoff): Tree-sitter Syntax Highlighting**

- Added `tree-sitter = "0.24"` and `tree-sitter-rust = "0.23"` to `Cargo.toml`
- Created `src-tauri/src/syntax/` module with `parser.rs` and `tokens.rs`
- `SyntaxParser` wraps Tree-sitter with Rust grammar loading and full/incremental parse support
- `tokenize_tree` maps Tree-sitter CST node kinds to Monaco-compatible token types
- Added `SyntaxToken`, `TokenizationRequest/Response` to `proto/ipc_editor_language.proto`
- Created `src-tauri/src/syntax_handlers.rs` with `tokenize_document` function
- Added `tokenize_document_json` Tauri command in `main.rs`
- Wired frontend `tauri/index.html`:
  - `setTokensProvider("rust")` reads from `state.tokenCache`
  - `fetchTokens` calls backend on document open and after content changes
- All 56 unit tests + 3 integration tests + 1 artifact test pass
- **Next session:** Expand Tree-sitter grammar support (TypeScript, JavaScript), add FoldingRange, optimize token refresh mechanism

**Session 2026-05-29 (continued): Phase 1 Frontend Wiring**

- Extended `UndoRedoJsonResponse` in `main.rs` to include `changes: Vec<EditChangeJson>` so the frontend can apply returned undo/redo operations to Monaco
- Updated `undo_json` and `redo_json` Tauri commands to convert protobuf `ModelContentChange` responses into frontend-friendly JSON
- Wired `tauri/index.html` frontend shell to the Rust backend:
  - `closeTab` now calls `close_document_json` to properly release buffers in the `BufferRegistry`
  - `onDidChangeContent` listener syncs all Monaco edits (including implicit undo/redo) to Rust via `apply_edits_json`
  - Added `isApplyingRemoteEdits` guard flag for future two-way sync
- All 47 unit tests + 3 integration tests + 1 artifact test continue to pass
- **Next session:** Begin Phase 2 (Tree-sitter syntax highlighting integration)

**Session 2026-05-29: Phase 1 Foundation Implementation**

- Analyzed the existing codebase: Protobuf schemas were already well-defined, Rust backend had `host_handlers.rs` with workspace/file operations, and a Tauri frontend shell was functional
- Added `ropey = "1.6"` dependency to `Cargo.toml`
- Created `src-tauri/src/buffer/` module with 7 files: `mod.rs`, `position.rs`, `line_index.rs`, `content_change.rs`, `undo.rs`, `text_buffer.rs`, `registry.rs`
- Extended `proto/ipc_editor_host.proto` with buffer operation messages: `CloseDocumentRequest/Response`, `ApplyEditsRequest/Response`, `GetBufferSnapshotRequest/Response`, `UndoRequest/Response`, `RedoRequest/Response`
- Integrated `BufferRegistry` into `MonacoHostState` and all host handler functions
- Updated `main.rs` to pass state through to host handlers
- Fixed existing integration tests (`m4_integration.rs`) to use new function signatures
- Achieved 47/47 unit tests + 3/3 integration tests passing

**Session 2026-05-29 (Phase 6): Security Hardening**

- **Phase 6.1 — Memory Bounds Enforcement:**
  - Created `src-tauri/src/security/sandbox.rs` with `SecuritySandbox`, `ResourceQuota`, and `SandboxLimits`
  - Limits include: max buffer size, max open buffers, max total memory, max decorations, max operation duration, max file size, max undo stack size
  - Three preset profiles: `default()`, `strict()` (for untrusted extensions), `permissive()` (for built-ins)
  - Per-principal quota tracking with violation counting
  - Integrated into `MonacoHostState` and all buffer lifecycle operations (`open_document` checks before open, `close_document` records on close)
  - Added 5 unit tests for sandbox enforcement (buffer count, file size, total memory, usage summary, strict vs default)

- **Phase 6.2 — Capability-Based Security:**
  - Created `src-tauri/src/security/capabilities.rs` with `CapabilityRegistry`, `Permission` enum, `Principal`, and `AuditLog`
  - 11 permissions: ReadFile, WriteFile, CreateFile, DeleteFile, ExecuteCommand, NetworkAccess, ListDirectory, ReadWorkspaceConfig, WriteWorkspaceConfig, ClipboardAccess, ShowNotification, FullAccess
  - Positive-grant model: denied by default, `FullAccess` bypasses all checks
  - Revocable permissions via `revoke` and `revoke_all`
  - Append-only `AuditLog` with configurable capacity and automatic trimming
  - Pre-configured role presets: `setup_builtin_extension` (read-only), `setup_agent` (read/write/create), `setup_system` (full access)
  - Integrated capability checks into all host handlers:
    - `open_document` → ReadFile
    - `save_document` → WriteFile/CreateFile
    - `apply_edits` → WriteFile
    - `list_directory_json` → ListDirectory
  - Added security management Tauri commands:
    - `get_audit_log` — returns all security events
    - `get_sandbox_summary` — returns per-principal resource usage
    - `grant_permission` — grants a permission by string name
    - `get_permissions` — lists permissions for a principal
  - Default "user" principal granted safe file access in `MonacoHostState::new()`
  - Added 6 unit tests for capabilities (grant/check, revoke, full access bypass, audit recording, audit trimming, role presets)

- **All 120 unit tests + 2 integration tests + 1 artifact test pass**

**Session 2026-05-29 (Phase 7): Testing and Validation**

- **Phase 7.1 — Protobuf Serialization Tests:**
  - Created `src-tauri/tests/m7_protobuf_roundtrip.rs` with 14 roundtrip tests
  - Tests cover all major IPC message types: `OpenDocumentRequest/Response`, `ApplyEditsRequest/Response`, `CloseDocumentRequest`, `WorkspaceRoot`, `TokenizationRequest`, `DiagnosticRequest/Response`, `CompletionRequest/Response`, `HoverRequest`, `DocumentSymbolRequest`, `EditorHostEvent`, `BufferRegistryEvent`
  - Each test encodes to bytes with `prost::Message::encode_to_vec()` and decodes back, verifying all fields match

- **Phase 7.2 — Integration Tests:**
  - Expanded `src-tauri/tests/m4_integration.rs` with 4 new end-to-end tests:
    - `end_to_end_edit_undo_redo_save_workflow`: open → edit → undo → redo → get snapshot → save → verify disk → close
    - `multi_view_buffer_sync_via_registry`: two views open the same file, edit from one view is visible to the other via shared `BufferRegistry`
    - `security_permission_denial_blocks_operations`: revoke `ReadFile` permission, verify `open_document` is blocked
    - `security_sandbox_blocks_oversized_file`: attempt to open a 200MB+ file, verify sandbox memory limit is enforced

- **Phase 7.2 — Performance Regression Benchmarks:**
  - Created `src-tauri/tests/m7_buffer_bench.rs` with 7 benchmark tests:
    - `bench_small_file_insert`: 1000 iterations of single-character insert, asserts <1ms/op
    - `bench_undo_redo_cycle`: 1000 undo/redo cycles, asserts <1ms/cycle
    - `bench_line_access_large_file`: 30,000 line accesses on 1k-line file, asserts <10µs/access
    - `bench_registry_open_close`: 100 open/close cycles, asserts <1ms/cycle
    - `bench_content_range_large_file`: 3,000 range fetches on 10k-line file, asserts <100µs/call
    - `bench_dirty_line_tracking_overhead`: 10,000 edits with dirty tracking, asserts <100µs/op
    - `bench_tokenize_large_rust_file`: tokenize 1,000-function Rust file, asserts <500ms total
  - All benchmarks print timing diagnostics and fail if thresholds are exceeded (debug-build adjusted)

- **All 148 tests pass:** 120 lib unit tests + 14 protobuf roundtrip + 7 performance benchmarks + 6 integration tests + 1 artifact test

---

*Written: 2026-05-29. If this file drifts from `PHASED_ROADMAP.md`, treat that as a hydration bug and fix in the same session.*
