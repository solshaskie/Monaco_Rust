# Monaco to Tauri/Rust/Protobuf Refactoring Roadmap

This document outlines a phased approach to refactoring Monaco Editor to integrate with the Tauri/Rust/Protobuf architecture. The goal is to create a high-performance, agentic workspace with unified serialization, native security boundaries, and radical performance improvements.

> **Status on 2026-06-03:** Phases 1 through 7 of the core Monaco_Rust refactor are substantially complete. The deferred WASM/custom-renderer lane ([`WASM_ROADMAP.md`](./WASM_ROADMAP.md)), external LSP integration (completion, diagnostics, code actions), and host-level testing (cross-platform CI, E2E proofs, visual regression pipeline) are now all landed. Read [`README.md`](./README.md) and [`HYDRATION.md`](./HYDRATION.md) for current runtime truth.
>
> **Post-review state:** A full-spectrum adversarial review ([`ADVERSARIAL_REVIEW.md`](./ADVERSARIAL_REVIEW.md)) identified 10 P0 security/correctness issues, 37 P1 performance/correctness issues, and 47+ P2 quality issues. The sequenced remediation plan lives in [`ADVERSARIAL_ROADMAP.md`](./ADVERSARIAL_ROADMAP.md) and should be consulted before any new feature work.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                     Tauri Webview Container                      │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │           TypeScript Layout & Rendering Shim                 ││
│  │  (Gutter, Scrollbars, DOM Events, Visual Overlays)          ││
│  └───────────────────────────┬─────────────────────────────────┘│
│                              │ WASM Bridge (Uint8Array)          │
│                              ▼                                  │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                  Rust/WASM Core Engine                       ││
│  │  • Piece Tree Buffer      • Protobuf Consumer               ││
│  │  • Tree-sitter Lexer      • Delta-Diff Calculation          ││
│  │  • Headless Buffer Registry • Language Features              ││
│  └─────────────────────────────────────────────────────────────┘│
└───────────────────────────┬─────────────────────────────────────┘
                            │ Tauri IPC (Protobuf)
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Tauri Backend (Rust)                          │
│  • File System Operations    • Workspace Management              │
│  • Extension Host Bridge     • MCP Server Integration            │
└─────────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Foundation - Protobuf Text Buffer Layer

**Goal:** Replace Monaco's JavaScript TextModel with a Rust-backed text buffer that communicates via protobuf.

### 1.1 Extend Protobuf Schemas
**Files to modify:** `proto/ipc_editor.proto`

- [x] Define `TextBuffer` message with piece-tree structure
- [x] Define `TextBufferDelta` for incremental edits
- [x] Define `UndoRedoState` for history management
- [x] Mirror Monaco's `IModelContentChangedEvent` exactly in protobuf

**Success Criteria:**
- Protobuf schemas compile without errors
- Schemas match Monaco's internal interfaces

### 1.2 Implement Rust Text Buffer Core
**New files:** `src-tauri/src/buffer/`

- [x] Implement `Rope` data structure in Rust (`ropey` crate)
- [x] Implement line splitting and indexing
- [x] Implement undo/redo stack with transaction support
- [x] Add UTF-8 content handling (avoid UTF-16 conversions)

**Dependencies to add to Cargo.toml:**
```toml
ropey = "1.6"  # or custom piece-tree implementation
```

**Success Criteria:**
- Unit tests pass for buffer operations
- Performance benchmarks show sub-millisecond edit operations

### 1.3 Create WASM Bridge Module (Deferred; now tracked in `WASM_ROADMAP.md`)
**New files:** `src-tauri/src/wasm_bridge.rs`

- [x] Compile text buffer to WASM using `wasm-bindgen` (completed in `WASM_ROADMAP.md` W1.1)
- [x] Implement `Uint8Array` ↔ Rust byte slice conversion (completed in `WASM_ROADMAP.md` W1.1)
- [x] Create memory-efficient buffer sharing mechanism (completed in `WASM_ROADMAP.md` W1.1–W1.3)
- [x] Expose buffer operations via WASM FFI (completed in `WASM_ROADMAP.md` W1.2)

**Dependencies:**
```toml
wasm-bindgen = "0.2"
js-sys = "0.3"
wasm-bindgen-futures = "0.4"
```

**Success Criteria:**
- WASM module loads in browser
- Text operations work across JS ↔ WASM boundary
- No memory leaks in boundary crossing

Current note: the repo now has a hybrid WASM compute lane under `wasm/` and `tauri/wasm-*`, but not the original plan's full text-buffer-to-WASM authority shift. That follow-on work lives in [`WASM_ROADMAP.md`](./WASM_ROADMAP.md).

### 1.4 Integrate with Existing Host Handlers
**Files to modify:** `src-tauri/src/host_handlers.rs`

- [x] Connect `open_document` to Rust buffer registry
- [x] Connect `save_document` to Rust buffer persistence
- [x] Add buffer version tracking for conflict resolution

**Success Criteria:**
- Existing tests pass with new buffer backend
- Round-trip save/open maintains data integrity

---

## Phase 2: Syntax Highlighting - Tree-sitter Integration

**Goal:** Replace Monarch/regex-based tokenization with Tree-sitter for precise, incremental syntax highlighting.

### 2.1 Tree-sitter Setup
**New files:** `src-tauri/src/syntax/`

- [x] Add `tree-sitter`, `tree-sitter-rust`, `tree-sitter-javascript`, `tree-sitter-typescript` crates
- [x] Create grammar loader for Rust, JavaScript, and TypeScript
- [x] Implement incremental CST (Concrete Syntax Tree) computation
- [x] Cache parsed trees for unchanged document regions

**Dependencies:**
```toml
tree-sitter = "0.24"
tree-sitter-rust = "0.23"
tree-sitter-javascript = "0.23"
tree-sitter-typescript = "0.23"
```

**Success Criteria:**
- Tree-sitter parses files correctly in WASM
- Incremental updates work on character-by-character input

Current note: Tree-sitter parsing is real and verified in the native Rust path today. The "in WASM" portion remains part of the separate hybrid compute lane rather than the core completed refactor.

### 2.2 Token Stream Generation
**New files:** `src-tauri/src/syntax/tokens.rs`

- [x] Convert CST to Monaco-compatible token stream
- [x] Implement token scope classification
- [x] Generate protobuf `SemanticTokens` messages (LSP-style)
- [x] Wire LSP semantic tokens provider to Monaco frontend
- [x] Support delta-encoded token updates (version-based delta with full-data fallback)

**Success Criteria:**
- Token output matches Monaco's expected format
- Semantic tokens render correctly in editor

### 2.3 Language Feature Protobuf Messages
**Files to modify:** `proto/ipc_editor_language.proto`

- [x] Add `SyntaxToken` message type
- [x] Add `TokenizationRequest/Response` messages
- [x] Add `FoldingRange` messages for code folding
- [x] Implement Tree-sitter CST folding range extraction
- [x] Wire folding range provider to Monaco frontend

**Success Criteria:**
- Protobuf schemas compile
- Token data serializes/deserializes correctly

---

## Phase 3: Language Features - Native Transport Layer

**Goal:** Route Monaco's language feature providers through the Tauri/Protobuf backend instead of web workers.

### 3.1 Completion Provider Bridge (Tree-sitter symbol-based MVP + external LSP)
**New files:** `src-tauri/src/syntax/completion.rs`, `src-tauri/src/lsp/`

- [x] Implement `CompletionRequest` handler in Rust
- [x] Connect to external LSP client (rust-analyzer, typescript-language-server) via `LspClient`/`LspRegistry`
- [x] Return `CompletionResponse` via protobuf
- [x] Cache completion results for repeated triggers (client-side via Monaco)
- [x] Fallback to Tree-sitter document symbols when LSP server is unavailable

**Success Criteria:**
- Autocomplete works with native backend using Tree-sitter document symbols
- Latency is lower than web worker approach

Current note: the Tree-sitter-backed native MVP is landed, and external LSP completion is now wired with automatic fallback to Tree-sitter symbols.

### 3.2 Diagnostics Pipeline
**New files:** `src-tauri/src/syntax/diagnostics.rs`, `src-tauri/src/lsp/convert.rs`

- [x] Implement diagnostic collection from LSP (merged with Tree-sitter parse errors)
- [x] Basic syntax error reporting via Tree-sitter parse errors
- [x] Wire diagnostics to Monaco markers via `setModelMarkers`
- [x] Push `DiagnosticResponse` via Tauri events on buffer edits
- [x] Support diagnostic severity levels (Error, Warning, Information, Hint)
- [x] Implement diagnostic code actions via `textDocument/codeAction`

**Success Criteria:**
- Errors/warnings appear in editor
- Diagnostics update on file changes
- Push diagnostics reduce frontend polling

### 3.3 Hover and Document Symbols
**New files:** `src-tauri/src/syntax/hover.rs`, `src-tauri/src/syntax/symbols.rs`

- [x] Implement hover information retrieval via Tree-sitter CST
- [x] Implement document symbol extraction (functions, structs, enums, traits, impls, mods, consts, statics, types, macros, use declarations)
- [x] Support symbol hierarchy (struct fields, enum variants)
- [x] Wire document symbol provider to Monaco frontend

**Success Criteria:**
- Hover shows documentation on cursor
- Outline view displays document structure

---

## Phase 4: Headless Buffer Registry

**Goal:** Implement a headless document state management system for multi-view and agent-driven editing.

### 4.1 Buffer Registry Implementation
**New files:** `src-tauri/src/buffer/registry.rs`

- [x] Create centralized buffer registry in Rust
- [x] Implement URI-based buffer lookup
- [x] Support multiple views of same buffer (via `Arc<RwLock<TextBuffer>>`)
- [x] Handle buffer lifecycle (open/close/dispose)

**Success Criteria:**
- Multiple editors can view same file
- Changes sync across all views

### 4.2 Event Broadcasting System
**New files:** `src-tauri/src/events.rs`

- [x] Implement protobuf event schema (`BufferRegistryEvent`, `BufferContentChangeEvent`)
- [x] Create subscription mechanism for buffer events (`EventSubscriptionTracker`)
- [x] Support `BufferRegistryEvent` push notifications via Tauri events
- [x] Handle event ordering and version conflicts (frontend version guard)

**Success Criteria:**
- Events propagate to all subscribers
- No race conditions in concurrent edits

### 4.3 MCP Agent Integration
**New files:** `src-tauri/src/mcp/tools.rs`

- [x] Expose buffer operations via MCP tools (`read_file`, `edit_file`, `list_symbols`, `apply_edits`)
- [x] Support agent-driven file mutations (agent edits via `edit_file` tool with old_text/new_text)
- [x] Implement optimistic locking for concurrent edits (`apply_edit_optimistic` in BufferRegistry with version checks)
- [x] Add conflict resolution strategies (version conflict error returned to agent, agent retries with updated content)
- [x] Extend the MCP seam into a truth-bearing surface with explicit certainty/provenance/evidence metadata
- [x] Add exact buffer/symbol inspection tools (`get_buffer_metadata`, `get_buffer_snapshot_proof`, `get_symbol_index`, `get_symbol_at_position`, `get_buffer_version_lineage`)

**Success Criteria:**
- AI agents can modify files programmatically
- Changes appear in UI without page refresh

---

## Phase 5: Performance Optimization

**Goal:** Optimize the critical path for editing performance and memory efficiency.

### 5.1 Zero-Copy Buffer Sharing
**New files:** `src-tauri/src/buffer/text_buffer.rs` (enhanced)

- [x] Implement content range fetching (`get_value_in_line_range`) for viewport-only content delivery
- [x] Add dirty line tracking to avoid full-file re-computation
- [x] Use `SharedArrayBuffer` for zero-copy access (completed in `WASM_ROADMAP.md` W1.1)

**Success Criteria:**
- Large file edits (>1MB) complete in <16ms
- Memory usage stable during extended editing

Current note: viewport-range fetching, dirty-line tracking, and `SharedArrayBuffer` zero-copy are all landed.

### 5.2 Layout Computation Offload
**New files:** `src-tauri/src/syntax_handlers.rs` (enhanced)

- [x] Compute viewport-visible tokens only (`tokenize_document_range`)
- [x] Move line height calculations to Rust (completed in `WASM_ROADMAP.md` W2.1)
- [x] Implement monospace coordinate math natively (completed in `WASM_ROADMAP.md` W2.1)
- [x] Batch layout updates for scrolling (completed in `WASM_ROADMAP.md` W2.2)

**Success Criteria:**
- Scrolling maintains 60fps
- Layout thrashing eliminated

### 5.3 Incremental Rendering
**New files:** `src-tauri/src/buffer/text_buffer.rs` (enhanced)

- [x] Implement dirty region tracking (`dirty_line_ranges` in TextBuffer)
- [x] Track affected line ranges per edit for selective re-tokenization
- [x] Calculate minimal DOM updates (completed in `WASM_ROADMAP.md` W2.3)
- [x] Support virtual scrolling for large files (completed in `WASM_ROADMAP.md` W2.2)

**Success Criteria:**
- Large files (10k+ lines) scroll smoothly
- Decorations update without flicker

---

## Phase 6: Security Hardening

**Goal:** Leverage Rust's memory safety for secure extension isolation.

### 6.1 Memory Bounds Enforcement
**New files:** `src-tauri/src/security/sandbox.rs`

- [x] Implement memory limits for decorations (`max_decorations_per_buffer` in `SandboxLimits`)
- [x] Add resource quotas per extension (`ResourceQuota` with per-principal tracking)
- [x] Enforce timeout limits on operations (`max_operation_duration_ms`)
- [x] Audit all WASM boundary crossings (completed in `WASM_ROADMAP.md` W1.1)

**Success Criteria:**
- Malicious extensions cannot crash editor
- Resource exhaustion attacks prevented

Current note: the Rust-side sandbox/capability posture is real and WASM boundary crossings have been audited.

### 6.2 Capability-Based Security
**New files:** `src-tauri/src/security/capabilities.rs`

- [x] Define permission model for extensions (`Permission` enum: ReadFile, WriteFile, CreateFile, DeleteFile, ExecuteCommand, NetworkAccess, ListDirectory, etc.)
- [x] Implement capability checking in Rust (`CapabilityRegistry` with positive-grant model)
- [x] Add audit logging for security events (`AuditLog` with append-only entries)
- [x] Support revocable permissions (`revoke`, `revoke_all`)

**Success Criteria:**
- Extensions only access authorized resources
- Permission violations are logged and blocked

---

## Phase 7: Testing and Validation

**Goal:** Ensure reliability and correctness of the refactored system.

### 7.1 Unit Tests
**Existing files:** `src-tauri/src/` (embedded `#[cfg(test)]` modules)

- [x] Buffer operation tests (TextBuffer: 12 tests, BufferRegistry: 10 tests, UndoStack: 5 tests)
- [x] Protobuf serialization tests (`tests/m7_protobuf_roundtrip.rs`: 14 roundtrip tests covering all IPC message types)
- [x] WASM bridge tests (completed in `WASM_ROADMAP.md` W3.1–W3.3)
- [x] Language feature tests (syntax_handlers: 20 tests, syntax modules: ~20 tests)

### 7.2 Integration Tests
**New files:** `src-tauri/tests/`

- [x] End-to-end editing workflows (`m4_integration.rs`: open → edit → undo → redo → save → close)
- [x] Multi-view synchronization tests (`m4_integration.rs`: shared buffer via registry, edits visible to all views)
- [x] Security integration tests (`m4_integration.rs`: permission denial, sandbox oversized file blocking)
- [x] Performance regression tests (`tests/m7_buffer_bench.rs`: small insert, undo/redo, line access, registry cycles, content range, dirty tracking, tokenization)

### 7.3 Smoke Tests
**Files:** `test/smoke/` (existing Playwright infrastructure for web builds)

- [x] Update existing smoke tests for Tauri (`tests/e2e_command_surface.rs`, `tests/e2e_event_broadcast.rs`, `tests/e2e_lsp_fallback.rs`)
- [x] Add visual regression testing (`tests/visual/` Playwright pipeline with `tauri-driver`)
- [x] Cross-platform compatibility tests (macOS x86_64/aarch64, Windows x86_64, Linux x86_64/aarch64 CI builds)

Current note: repo-owned Rust integration/e2e proofs are present, cross-platform CI builds run on every PR, and the visual regression pipeline (`tests/visual/`) is wired into CI.

---

## Implementation Timeline

| Phase | Estimated Duration | Dependencies |
|-------|-------------------|--------------|
| Phase 1: Foundation | 2-3 weeks | None |
| Phase 2: Syntax Highlighting | 2 weeks | Phase 1 |
| Phase 3: Language Features | 2-3 weeks | Phase 1, 2 |
| Phase 4: Headless Registry | 2 weeks | Phase 1, 3 |
| Phase 5: Performance | 2-3 weeks | Phase 1-4 |
| Phase 6: Security | 1-2 weeks | Phase 1-5 |
| Phase 7: Testing | 2 weeks | Phase 1-6 (ongoing) |

**Total Estimated Time:** 13-17 weeks

---

## Risk Mitigation

1. **WASM Performance:** Profile early and often. Have fallback to JS implementation if WASM bridge overhead is too high.

2. **Protobuf Compatibility:** Version all protobuf schemas. Implement migration strategies for schema changes.

3. **Memory Leaks:** Use Rust's ownership model strictly. Add memory profiling to CI pipeline.

4. **Browser Compatibility:** Test SharedArrayBuffer availability. Provide graceful degradation.

5. **LSP Integration:** Maintain compatibility with existing LSP servers. Don't break existing integrations.

---

## Success Metrics

- **Edit Latency:** <16ms for typical edits (maintains 60fps)
- **Memory Usage:** <50MB for 10k line file
- **Startup Time:** <2s to interactive editor
- **Large File Support:** Smooth editing for files up to 1MB
- **Agent Throughput:** Support 100+ edits/second from MCP agents
- **Zero Crashes:** No memory safety violations in production

---

## References

- [Monaco Editor Documentation](https://microsoft.github.io/monaco-editor/)
- [Tauri Documentation](https://tauri.app/)
- [Protobuf Language Guide](https://protobuf.dev/)
- [Tree-sitter Documentation](https://tree-sitter.github.io/)
- [WASM Bindgen Guide](https://rustwasm.github.io/docs/wasm-bindgen/)

---

## Deferred Work

All previously deferred items are now landed:
- WASM bridge, custom renderer, and cross-platform CI infrastructure are complete (see [`WASM_ROADMAP.md`](./WASM_ROADMAP.md)).
- External LSP integration (completion, diagnostics, code actions) is wired via `src-tauri/src/lsp/`.

Remaining future enhancements (not blockers):
- LSP workspace symbols, rename, and call hierarchy.
- Incremental LSP sync (currently sends full document on each change).
- Tauri WebDriver E2E against a running app (existing `tests/visual/` pipeline is the foundation).
