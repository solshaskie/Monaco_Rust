# Monaco_Rust — Full-Spectrum Adversarial Review

> Generated 2026-06-03 against the working tree at `57cfbb6f044cf89ccc4e704639ec7052229cd018`.
> Read every Rust source, every proto, every JS file, every test, every CI workflow, every doc.
> Severity tags: **P0** = exploitable / corrupting / blocking; **P1** = material correctness or performance; **P2** = quality/ergonomics.
>
> This document is intentionally adversarial. It is a red-team pressure surface, not the repo's canonical truth source. Some findings are direct code observations; others are risk inferences or worst-case interpretations that should be validated against current repo truth in `README.md`, `HYDRATION.md`, `PHASED_ROADMAP.md`, and `WASM_ROADMAP.md` before being treated as project direction.
>
> Staleness note: parts of this review may lag subsequent hardening work. In particular, sync-barrier / token-consumer wiring and some roadmap framing changed after the baseline commit named above.

---

## 1. High-Level Architecture Map

### 1.1 Runtime topology

```
┌────────────────────────────────────────────────────────────────────┐
│  Tauri Webview (Monaco editor)            tauri/index.html         │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  Monaco editor + Tauri event listeners                        │  │
│  │  onDidChangeContent ──► invoke("apply_edits_json", …)         │  │
│  │  onDidChangeCursor  ──► invoke("hover_document_json", …)      │  │
│  │  register*Provider  ◄── (cache hit)                           │  │
│  │  buffer-content-changed / diagnostics-changed (Tauri events)  │  │
│  └──────────────────────────────────────────────────────────────┘  │
│           │ Tauri IPC (JSON OR protobuf bytes)         ▲            │
│           ▼                                             │            │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  WASM Compute Lane (tauri/wasm-glue.js, wasm-sync.js, …)      │  │
│  │  - textDecoder/tokenize/diff/layout (heuristic, see §3.2)     │  │
│  │  - shadow buffer cache + sync barrier                        │  │
│  │  - apply_binary_delta() — the one true binary path           │  │
│  └──────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────┘
                              │
                              │ Tauri Command Bus
                              ▼
┌────────────────────────────────────────────────────────────────────┐
│  src-tauri (Rust, authoritative)                                   │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  main.rs  ──►  36 #[tauri::command] handlers (18 logical ×2)  │  │
│  │               — JSON and protobuf parallel surfaces            │  │
│  │  host_handlers.rs, syntax_handlers.rs                          │  │
│  │  mcp/tools.rs  (McpTool trait, 9 tools)                        │  │
│  └──────────────────────────────────────────────────────────────┘  │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────────┐    │
│  │ buffer/  │  │ syntax/  │  │  lsp/    │  │   security/       │    │
│  │ rope+idx │  │ tree-sit │  │ JSON-RPC │  │ capability+quota │    │
│  │ undo/red │  │ tokens   │  │ rust-an. │  │ positive-grant  │    │
│  │ registry │  │ symbols  │  │ ts-lang. │  │ audit log       │    │
│  └──────────┘  └──────────┘  └──────────┘  └──────────────────┘    │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  events.rs   ──►  Tauri event bus                             │  │
│  │  wasm_sync.rs ──►  Binary snapshot/delta to JS               │  │
│  └──────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
                       Disk (filesystem)
```

### 1.2 Process boundaries

| Boundary | Type | Encoded as | Hot path? |
|---|---|---|---|
| Monaco → Tauri | `__TAURI__.core.invoke` | JSON (current) or `Vec<u8>` protobuf (dormant) | Yes — every keystroke |
| Tauri → Monaco | Tauri `emit` | JSON only | Yes — every edit/diagnostic |
| Tauri → LSP | stdio pipes | JSON-RPC framed by `Content-Length` | Per-edit (today) |
| Tauri → WASM | JS calls WASM directly; Rust pushes via `wasm_sync_state_binary` | `Vec<u8>` parsed by `apply_binary_delta` | On open/refresh |
| WASM ← Rust | one-shot via Tauri command | binary | Per open + per heartbeat |
| Rust internal | `RwLock<LspRegistry>`, `RwLock<BufferRegistry>`, `Arc<RwLock<TextBuffer>>` | n/a | Per-edit |

### 1.3 Data flow — one keystroke (today)

1. Monaco fires `onDidChangeContent`.
2. `tauri/index.html` maps the change to `EditChangeJson[]` and `invoke("apply_edits_json", …)`.
3. `main.rs::apply_edits_json` validates JSON, then calls `host_handlers::apply_edits`.
4. `host_handlers::apply_edits` checks `Permission::WriteFile`, takes a `read` lock on the outer `BufferRegistry`, calls `apply_edit` (which takes its own `write` lock internally), drops the outer lock, takes it again to read the buffer content, then pushes a **full document** `didChange` to the LSP.
5. The `BufferRegistry::apply_edit` mutates the rope, increments version, builds a new `LineIndex` from scratch (O(N) per keystroke), and rebuilds the dirty-line list.
6. `apply_edits_json` then synchronously runs `syntax_handlers::diagnostics_document` and re-tokenizes via Tree-sitter, pushing the result as a `diagnostics-changed` Tauri event.
7. Back in JS, the same handler also calls `state.wasmSync.refreshDebounced(path)` and re-fetches tokens/folding/semantic/symbols (5 round-trips).
8. Tauri emits `buffer-content-changed` and `diagnostics-changed` to all listeners.

One keystroke touches 2 lock acquisitions on the outer registry, 2 on the inner buffer, 1 LSP `didChange` (full file), 1 full re-tokenization via tree-sitter, 1 diagnostic re-derivation, 1 `setModelMarkers` call, and 5 JSON-returning Tauri commands. **This is the central performance characteristic of the system.**

---

## 2. Module-by-Module Review

### 2.1 `src-tauri/src/buffer/` — the core

**`text_buffer.rs` (866 lines).** Rope-backed, has versioning, undo/redo, dirty-line tracking, snapshot. Five concrete problems:

- **P1: `LineIndex::new` is O(N²) on construction.** It walks every line and calls `content[pos..line_end].encode_utf16().count()` for each. For a 100k-line file this is roughly 5×10⁹ UTF-16 code-unit conversions. The `LineIndex` is rebuilt on **every edit** in `apply_change` (line 309) and `set_value` (line 440). A typing user pays O(N) per keystroke against a file they only look at O(1) lines of. The cost dominates everything else on large files. The fix is to use the rope's own line table (`Rope::line_to_char`, `Rope::char_to_line`, `Rope::utf16_line_to_char`) and to update incrementally from the changed `LineRange` rather than rebuilding.
- **P1: `position_to_offset` and `offset_to_position` walk a substring on every call.** `let line_content = &content[line_start..];` does not allocate the slice itself, but the subsequent `line_str.chars().count()` walks the entire rest of the line. For `offset_to_position` from a click at line 50,000 of a 100k-line file, that is a 50k-character walk before the function can answer. Both should be byte-indexed against the rope directly.
- **P1: `get_value_bytes` and `get_value` clone the entire content.** `self.rope.to_string().into_bytes()` is a full content copy on every snapshot. `BufferSnapshot.content_utf8: Vec<u8>` is a clone of the bytes, then `mcp::GetBufferSnapshotProofTool` clones it again to compute SHA-256 and again to render as `String`. The single `get_buffer_metadata_tool` call in tests touches the file's bytes three times.
- **P2: `apply_change` always rebuilds the full `LineIndex`.** The `dirty_line_ranges` is computed correctly, but the LineIndex rebuild ignores it. An incremental LineIndex update (only re-index the changed range plus one boundary) would be O(Δ) instead of O(N).
- **P2: `apply_change` always pushes a single-element undo transaction.** This means every keystroke is a separate undo step, which is what Monaco wants, but it also means `UndoStack.undo()` does a per-keystroke transaction. Batch-coalescing would be a UX win, but that would change the contract with Monaco's `IModel.getAlternativeVersionId()`. Defer.

**`registry.rs` (455 lines).** Hashmap of `Arc<RwLock<TextBuffer>>`. Five problems:

- **P0: `apply_edit_optimistic` has a TOCTOU race.** It reads the version under `read()`, drops the lock, then re-acquires as `write()` and re-checks. Between the two, another writer can change the version. The check is not atomic. The fix is to acquire `write()` once, read version, compare, and either commit or fail without releasing.
- **P1: `apply_edit`, `set_buffer_content`, `mark_buffer_saved`, `undo`, `redo` all use `&self` but mutate the inner `RwLock`.** This is fine, but the **outer** `RwLock<BufferRegistry>` in `MonacoHostState` is functionally dead weight. Either the outer lock should be removed (rely on inner locks for the buffer map) or it should be a `Mutex`. The current double-locking adds zero safety and one read-lock acquisition per call.
- **P1: `apply_edit` and `set_buffer_content` fall through to `None` if the buffer is not found.** Callers like `host_handlers::apply_edits` cannot distinguish "no such buffer" from "edit had no effect"; both return `success: false`.
- **P2: `open_buffer_from_bytes` clones the content twice.** `content_bytes.to_vec()` then `String::from_utf8(...).to_string()` plus `open_buffer` which calls `TextBuffer::new(..., &content)`. For a 100MB file that's 200MB of allocations during open.
- **P2: `get_buffer_content_bytes` calls `TextBuffer::get_value_bytes` which is `self.rope.to_string().into_bytes()`.** Full content copy on every metadata read.

**`line_index.rs` (249 lines).** Backing store is `Vec<usize>` of byte offsets. Has the O(N²) bug above plus:

- **P2: `line_end_offset` returns `None` for the last line.** The doc says "Caller should handle this case" and the only caller (`text_buffer::get_line`) does. An unfinished API that should either return `Some(rope.len_bytes())` or be removed.

**`undo.rs` (309 lines).** Reasonable. The `undo()` method clones the transaction onto the redo stack (`self.redo_stack.push(transaction.clone())`) — for long edit histories this is meaningful memory bloat. A cheaper approach is to swap an `Option<UndoTransaction>` and re-push on redo.

**`content_change.rs` (139 lines).** A clean value type. No issues.

**`position.rs` (69 lines).** Fine.

**`mod.rs` (27 lines).** Just re-exports. Fine.

### 2.2 `src-tauri/src/host_handlers.rs` — the Tauri command surface (934 lines)

This is the security hot-spot and the most fragile module. Eleven concrete problems:

- **P0 SECURITY: No path containment.** `uri_to_path` is `PathBuf::from(&resource.path)`. A caller that passes `resource.path = "/etc/passwd"` opens `/etc/passwd`; a caller that passes `"../../foo"` opens `../../foo` relative to the Tauri process's CWD. `CapabilityRegistry::check` returns `true` for any path under the user principal — there is no path-prefix check, no canonicalization, no workspace-root boundary. The `workspace_roots_json` Tauri command returns the configured roots but `open_document`, `save_document`, `list_directory` and the MCP `read_file`/`edit_file` tools do not consult them. This is a high-confidence boundary risk; the concrete exploitability claims should still be live-verified in the running app before being treated as proven exploit paths.
- **P0 SECURITY: `save_document` and `save_document_as` write without symlink awareness.** `fs::write(&path, …)` follows symlinks. A user-controlled path under the workspace could be a symlink to `/etc/passwd` and the editor would happily overwrite it.
- **P0 SECURITY: `save_document_as` does not call `lsp.did_close` for the source URI or `lsp.did_open` for the target URI.** LSP state is silently desynchronized. The LSP server thinks the file is still at the old URI. Worse, the in-memory buffer for the source URI is closed but the old URI is the only one the LSP server knows about; subsequent diagnostics on the old URI will silently no-op.
- **P0: `apply_edits` sends a full-document `didChange` to the LSP.** Lines 497-508: `change_json = serde_json::json!({"range": null, "rangeLength": null, "text": <full content>})`. LSP semantics: `range = null` means **full replacement**, not incremental. Every keystroke forces the LSP server to re-parse and re-type-check the entire file. This is the single biggest avoidable cost in the editor and the roadmap explicitly defers "incremental LSP sync" as if it is hard. It is not: `range = { start, end }` plus a `text` is the spec.
- **P1: `apply_edits` re-acquires `BufferRegistry::read` three times.** Once to call `apply_edit`, once to read content for the LSP `didChange`, and once to get version. Three lock acquisitions and two extra full-content clones for one edit. Should be a single read-lock that returns `(content, version)`.
- **P1: `apply_edits` mutates via `read()` lock because `BufferRegistry::apply_edit(&self, ...)` is interior-mutable.** The outer `RwLock<BufferRegistry>` is read-locked; the inner `Arc<RwLock<TextBuffer>>` is write-locked. This works, but the outer read lock is held across the whole sequence.
- **P1: `close_document` with `save_if_dirty` is racy.** The "is dirty" check (line 397), the snapshot read (line 405), and the registry write (line 423) are three separate lock acquisitions. A concurrent edit between lines 397 and 412 changes the dirty state and the snapshot. The save then writes the **stale** snapshot, losing the edit.
- **P1: `open_document` re-reads the file just to detect EOL after the bytes are already in memory.** `detect_eol(&path)` does a full `fs::read` of the file (line 811) on top of the bytes already loaded into `content`. For a 100MB file this is a 100MB extra disk read and string allocation.
- **P2: `apply_edits` returns `success: false` (not an error) when the buffer is not in the registry.** Callers cannot distinguish this from "edit applied but had no effect".
- **P2: `set_primary_workspace_root` does not restart or notify the LSP.** `LspRegistry::new(workspace_root)` is called once at startup and `workspace_root` is immutable. Switching the primary workspace root from the frontend will leave the LSP server with the old root URI, so its diagnostics will be for the wrong files. This is a real bug, not a TODO.
- **P2: `save_document` does not bump `version_id`.** It calls `mark_buffer_saved` which sets `is_dirty = false` and aligns `alternative_version_id`, but the `version_id` (Monaco's main version counter) is unchanged. The frontend's `state.currentVersionId` then goes out of sync.

### 2.3 `src-tauri/src/main.rs` - IPC surface (1380 lines)

- **P1 framed as architectural drift, not immediate exploit: Dual-encoding duplication.** Every logical Tauri command is exposed twice - once as a protobuf-bytes handler and once as a JSON-deserialize handler. That's 18 logical commands x 2 = 36 invokable handlers. They drift. The frontend uses only the JSON path; the protobuf path is wired but not consumed. Pick one and delete the other.
- **P0: `McpToolRegistry::with_defaults()` is constructed on every `execute_mcp_tool` call.** Line 1135: `let tool_registry = monaco_tauri::mcp::McpToolRegistry::with_defaults();`. Each invocation rebuilds the registry, allocates 9 boxed trait objects, and does the lookup. Move into `MonacoHostState` (constructed once).
- **P0: `apply_edits_json` synchronously re-runs `syntax_handlers::diagnostics_document` and pushes a `diagnostics-changed` Tauri event after every edit.** Line 596: `let mut lsp = state.lsp_registry().write()...;` then `syntax_handlers::diagnostics_document(&registry, Some(&mut lsp), ...)`. Diagnostics is one of the most expensive operations (Tree-sitter parse + LSP round-trip). It should be debounced or moved to a background tokio task, not serialized into the keystroke handler.
- **P1: `apply_edits_json` clones every edit twice.** Lines 547-574 build `buffer_changes: Vec<BufferChangeEvent>` and `edits: Vec<ModelContentChange>` from the same input. One canonical type, derived into both.
- **P2: `LspRegistry::path_to_uri` is duplicated** in both `lsp/registry.rs` (line 110) and in `syntax_handlers.rs` (inlined three times for diagnostics/completion/code-action at lines 290, 432, 528). Drift hazard.
- **P2: `apply_edits` synchronously awaits and pushes `diagnostics-changed` even when the buffer was newly created with no LSP registered.** Always pays the cost even when the result is empty.

### 2.4 `src-tauri/src/syntax_handlers.rs` (1046 lines)

- **P1: Every handler fetches buffer content four times.** `get_buffer_content` + `get_buffer_version` + (sometimes) `get_buffer_content_range` + tree-sitter parse. All in separate lock acquisitions on the registry.
- **P1: A fresh `SyntaxParser` is created for every call.** Tree-sitter parser construction is non-trivial. Cache by `language_id` keyed on a `Mutex<HashMap<String, Arc<Mutex<SyntaxParser>>>>`.
- **P1: `tokenize_tree_range` does not actually range-tokenize.** It collects all lines into a Vec and slices. The "range" optimization is fake.
- **P1: `completion_document` calls `lsp.client_for_path(...)` which spawns the LSP process on first call from a synchronous Tauri command.** First-time `rust-analyzer` launch can take 2-10 seconds. The Tauri command thread is blocked. The MCP `completion` tool inherits the same hazard.
- **P2: `diagnostics_document` makes a synchronous `textDocument/diagnostic` LSP request with no cancellation.** If the LSP server is slow or hung, every Monaco diagnostic refresh hangs for 5 seconds.
- **P2: Three near-identical `textDocument/...` LSP call shapes** duplicate the URI prefixing and position-conversion boilerplate.

### 2.5 `src-tauri/src/lsp/` (4 files, ~500 lines)

**`client.rs` (292 lines).** The most production-leaning module in the repo.

- **P0: `LspClient::request` blocks the Tauri command thread for up to 5 seconds on any LSP round-trip.** Combined with `RwLock<LspRegistry>` (which serializes all LSP calls), one slow LSP call freezes the entire editor.
- **P0: `Drop::drop` calls `self.shutdown()` which makes a synchronous `shutdown` request to the LSP.** If the LSP process is dead, the request times out (5s). Drop can take 5+ seconds per LSP server.
- **P0: `request` allocates a new `mpsc::channel` for every call.** At 60Hz `did_change` rate, this is 60 channel allocations per second per open file.
- **P1: The reader thread is never joined.** `_reader_thread: Option<JoinHandle<()>>` exists but `join()` is never called.
- **P1: `request` serializes the request with `serde_json::to_string` then writes bytes; the reader deserializes from bytes.** Round-trip through `String` adds two copies. For a 1MB `didChange` that's 3MB of allocation per edit.
- **P1: No support for cancellation.** A `didChange` for a long file in flight cannot be canceled if a newer edit arrives.
- **P2: `did_change` signature takes `Vec<serde_json::Value>` for the `contentChanges` array.** Should take typed `Vec<lsp_types::TextDocumentContentChangeEvent>`.

**`registry.rs` (116 lines).**

- **P0: `workspace_root` is captured at construction and never updated.** `set_primary_workspace_root` is a no-op for the LSP.
- **P1: `client_for_path` does extension-based dispatch; `mcp::parser_for_path` and `syntax_handlers::parser_for_path` do similar but inconsistent dispatch.** `.tsx` is treated as `typescript` here but `mcp::parser_for_path` falls through to `for_rust()` for `.tsx`. Three places to keep in sync; today they aren't.
- **P1: `LspClient::spawn` is called on first access.** No warm-up. The first `completion` call for `.rs` will block 2-10s on `rust-analyzer` boot.

**`convert.rs` (90 lines).**

- **P2: `lsp_completion_to_proto` formats the kind with `format!("{:?}", k).to_lowercase()`.** The string is e.g. `"completionitemkind::function"`. The frontend's `monaco.languages.CompletionItemKind["Completionitemkind::function"]` will be undefined. The result: the completion kind is always `Text`.

### 2.6 `src-tauri/src/mcp/tools.rs` (1395 lines)

- **P0: `McpToolRegistry::with_defaults()` is allocated on every `execute_mcp_tool` call** (see main.rs).
- **P1: `list_symbols_tool` is a hand-rolled regex-free line-prefix heuristic** that returns the SAME information as `get_symbol_index_tool` via Tree-sitter but tagged `certainty: Heuristic`. Dead-weight or a deliberate fallback; if the latter, the heuristic should be invoked only when the parser fails.
- **P1: `edit_file_tool` calls `content.find(&args.old_text)` and replaces the first occurrence.** If `old_text` is empty, `find("")` returns `Some(0)` and the function silently inserts at position 0. No validation. Ambiguous matches are silently resolved to the first.
- **P1: `edit_file_tool` with no `expected_version` uses `apply_edit` (not `apply_edit_optimistic`).** A stale edit lands without version check. The default should be refuse.
- **P1: `apply_edits_tool` applies edits one at a time in a loop, taking a write lock per edit, with no version check, no atomic transaction.** Should use `BufferRegistry::apply_changes` (already exists in `text_buffer.rs`, not exposed by the registry) and bracket in a single lock.
- **P1: `get_buffer_metadata_tool` reads the full content three times** then SHA-256s it. For a 100MB file: ~300MB memory traffic.
- **P1: `parser_for_path` here differs from `syntax_handlers::parser_for_path` and `lsp::registry::client_for_path`.** `.tsx` falls through to Rust in the MCP path.
- **P1: `get_symbol_index_tool` and `get_symbol_at_position_tool` re-parse the content from scratch on every call.** No parse cache.
- **P2: `get_buffer_version_lineage_tool` reports `historical_versions_stored: false` and `exact_prior_diff_reconstructable: false` - honest negative capabilities - but the tool still returns `success: true` and `certainty: Observed`.** The contract should distinguish "no data" from "observed fact".
- **P2: All MCP results return `content: String` AND `data: serde_json::Value`.** Some tools duplicate the same data into both fields.
- **P2: `offset_to_position` here is a separate copy** of logic that exists in `LineIndex::offset_to_position`. Should be a shared helper.
- **P2: No streaming.** `read_file` returns the entire content as a single string.

### 2.7 `src-tauri/src/security/` (~720 lines)

**`sandbox.rs` (316 lines).** Solid quotas, but:

- **P1: `quota_for` always re-`entry().or_insert_with()` on every call.** That's a lock + HashMap write for every check.
- **P1: `record_buffer_resize` clamps to `max_total_memory_bytes` but only increments `violation_count` - it doesn't actually reject the edit.**
- **P2: `ResourceQuota` has no `current_cpu_time` tracking**, despite `max_operation_duration_ms` existing. The duration check is unused.

**`capabilities.rs` (396 lines).**

- **P0: Capability checks do not consult the path being accessed.** `check(&principal, Permission::ReadFile, &resource.path)` passes the path as a string for audit logging only. The path is NOT checked against the principal's allowed paths. A `ReadFile` grant lets you read any file. (See 2.2.)
- **P0: `setup_system` grants `FullAccess` to "system" and `Default::default()` calls `setup_system` automatically.** A misconfigured call site using `CapabilityRegistry::default()` gets a system principal with `FullAccess` silently.
- **P1: `AuditLog` is `Vec<AuditLogEntry>` behind a `Mutex`.** Every check appends an entry. The 10,000-entry cap means the log is wiped every ~10 minutes under load.
- **P1: `has_permission_internal` and `check` both lock the same `Mutex`.** Two lock acquisitions per check.

### 2.8 `src-tauri/src/events.rs` (184 lines)

- **P1/P2 depending on intended direction: `EventSubscriptionTracker` is defined, tested, and never used.** Dead code; the Tauri event broadcaster does not consult it. This is more credibility/maintenance drag than immediate runtime danger unless a subscription contract is being claimed elsewhere.
- **P0: No version guard in the broadcaster.** Tauri `emit` always fires regardless of subscribers.
- **P1: `BufferChangeEvent` is structurally identical to `editor::ModelContentChange` but with a `String` text field instead of `bytes`.** Two parallel type hierarchies.
- **P2: `emit_buffer_content_changed` ignores the return value of `app.emit`.** No way to know if the emit failed.

### 2.9 `src-tauri/src/wasm_sync.rs` (266 lines)

- **P1: `compute_line_delta` uses `Vec<&str>` from `split('\n')` to compute the changed range, then `new_lines[first_changed..last_new].join("\n")` to build the new text.** For a 100k-line file this is 100k str comparisons and 100k line splits.
- **P1: `to_binary` format is not self-describing.** The magic bytes are mode=0/1/2 but the consumer has no way to know the schema version.
- **P2: The format doesn't include a CRC/checksum.** A torn write produces silent corruption.
- **P2: No streaming sync.** The `BufferSnapshot` returns the full text in one binary blob.

### 2.10 WASM crate (`wasm/src/lib.rs` + `cache.rs` + `diff.rs` + `layout.rs` + `tokenize.rs`)

- **P0: `wasm/src/tokenize.rs` is a hand-rolled heuristic, NOT tree-sitter.** The file's own comments admit this. The native Rust side has real tree-sitter. The WASM side has a regex-less line-prefix matcher. The two tokenizers disagree about what counts as a keyword.
- **P1: `tokenize_source` does `let chars: Vec<char> = line.chars().collect();` then indexes with `chars[i]`.** For a 100k-line file with CJK content, this is 1M character allocations per tokenize call. Use byte indices.
- **P1: `wasm/src/diff.rs::line_diff` has dead code in the first pass (lines 25-66 are computed, then `edits.clear()` on line 67 wipes them).** The second pass uses `lcs.contains(...)` which is O(L) per check, total O(N^2). Delete first 67 lines.
- **P1: `compute_line_layout` does `line.chars().count()` per line.** O(N) Unicode walk per line, O(N^2) for a full file.
- **P1: `compute_visible_lines` calls `source.lines().count()` on every call** to compute the total. On a scroll event firing 60 times per second, that's 60 full-file walks per second.
- **P1: WASM token cache (`cache.rs`) holds the entire source as `String` for every cached resource** with no eviction. For 100 open files of 1MB each, that's 100MB of WASM linear memory.
- **P1: WASM token cache uses `String::eq` (full content comparison) on every cache lookup.** For a 1MB source, 1MB of byte comparison per call. Replace with SHA-256.
- **P2: `read_string_from_buffer` and `write_string_to_buffer` use `js_sys::Uint8Array::slice()` then `copy_to(&mut vec)`.** Intermediate Vec allocation per transfer.
- **P2: `console_error_panic_hook` is feature-gated and OFF by default.** Panics in WASM become opaque errors.
- **P2: `apply_binary_delta` returns `JsValue` via `serde_wasm_bindgen::to_value`.** No `.d.ts` is generated (`--no-typescript` in build script).
- **P2: `tokenize_range` in `tokenize.rs` (line 204) collects all lines into a Vec then slices.** The "range" is fake.

### 2.11 Tauri-side JS (`tauri/index.html`, `tauri/wasm-glue.js`, `tauri/wasm-sync.js`, `tauri/wasm-tokenizer-provider.js`, `tauri/src/renderer/*.js`)

- **P0: The frontend installs a `buffer-content-changed` listener that calls `tab.model.applyEdits(edits)` (line 1172).** The `model.onDidChangeContent` handler at line 524 also fires when remote edits are applied, and calls `invoke("apply_edits_json", ...)`. If Monaco's `applyEdits` is asynchronous, another edit could slip through despite the `isApplyingRemoteEdits` guard.
- **P0: `state.isApplyingRemoteEdits` is a single boolean for all paths.** Concurrent remote edits on different buffers cannot be distinguished.
- **P1: Every Monaco provider is registered for `rust`, `javascript`, `typescript` independently with copy-pasted code (lines 859-1096 in `index.html`).** Should be a `for (const lang of [...]) { registerAll(lang) }` loop. Today: 3x the source, 3x the bug surface.
- **P1: `setStatus("Saved " + state.currentPath)` is the only feedback for save success or failure.** A failed save looks like a successful save.
- **P1: `activateTab` calls `probeActiveDocumentFeatures` which calls `monaco.languages.typescript.getTypeScriptWorker()` on every cursor change.** A worker is fetched per cursor move.
- **P1: `applyLineDelta` in `wasm-sync.js` does `content.split('\n')` -> splice -> `join('\n')` on every delta application.** O(N) per refresh.
- **P1: `decoration-manager.js::_applyBatch` calls `this.editor.getModel().getAllDecorations()` on every batch flush.** This returns ALL decorations on the model, not just the token ones.
- **P2: The `apply_binary_delta` WASM call returns a `JsValue`.** The TS consumer doesn't type this. Drift hazard.
- **P2: The compatibility shim monkey-patches `editor.deltaDecorations` and `editor.getLayoutInfo`.** Multi-shim conflict hazard.
- **P2: The `wasmBufferSync` global on `window` is a side-channel coupling between the WASM layer and Monaco providers.** Should be passed explicitly via injection.
- **P2: The five cache Maps (`tokenCache`, `foldingCache`, `semanticTokensCache`, `diagnosticsCache`, `symbolsCache`) are uncached and manually invalidated.**

### 2.12 Test files

- **P1: `m7_buffer_bench.rs` thresholds are wrong.** `bench_small_file_insert` asserts < 1ms/op but does 1000 ops including a `TextBuffer::new` per iteration. The `new` is the dominant cost.
- **P1: `m4_integration.rs::security_permission_denial_blocks_operations` revokes `ReadFile` for the default user but never validates that the path containment (the real P0 issue) is enforced.** The test passes for the wrong reason.
- **P1: `e2e_lsp_fallback.rs` uses `state.buffer_registry().write().unwrap()` to insert a buffer directly into the registry, bypassing `open_document`.** The test does not exercise the `did_open` LSP notification path.
- **P1: No test exists for the WASM sync binary path under real load.** `m7_protobuf_roundtrip.rs` only round-trips protobuf messages in isolation.
- **P1: No property-based test (proptest/quickcheck).** Buffer content is adversarial input; the absence of property tests means the `LineIndex` UTF-16 conversion, the `apply_change` undo/redo cycle, and the `content_change` boundary checks are unexercised.
- **P2: There is no fuzz target.** Buffer content from the filesystem is untrusted.
- **P2: The visual regression suite uses `xvfb-run npx playwright test` but there is no baseline image committed.** The pipeline always passes because there is nothing to diff against.

### 2.13 Protobuf schemas

- **P1: `IpcRequest` carries `channel_name` + `command_name` + `payload: bytes` with no schema discriminator.** The consumer has to know the payload type from the channel/command string.
- **P1: `BufferRegistryEvent` exists in BOTH `ipc_editor.proto` AND `ipc_editor_host.proto` with different enum values.** Two truths; pick one.
- **P1: `FileChangeEvent.change_type` is a magic `int32` (1=added, 2=deleted, 3=updated) with no enum.**
- **P1: `ReaddirResponse` returns `repeated FileStat entries` but `FileStat` includes `children` (recursive) and `is_directory` (a separate field from `type`);** structurally awkward.
- **P1: `ModelContentChange` carries BOTH `range_offset`/`range_length` (byte offsets) AND `start_line`/`start_column`/`end_line`/`end_column` (positions).** Two redundant sources of truth.
- **P1: `ReadFileRequest` has no `byte_offset`/`byte_length`.** Can't do partial reads.
- **P2: `IpcEvent.subscription_id` is a magic `uint32` with no event-type discriminator.**
- **P2: `WatchResponse.watcher_id` is `int32`; `UnwatchRequest.watcher_id` is also `int32`; neither is signed. Why int32?**

### 2.14 CI / build / packaging

- **P1: `release.yml` uses `actions/create-release@v1` and `actions/upload-release-asset@v1`** - both are deprecated/archived by GitHub. The release pipeline is broken.
- **P1: `visual-regression.yml` runs `xvfb-run npx playwright test` against the Tauri build with no `tauri-driver` setup; the `build.yml` `visual-regression` job DOES set up `tauri-driver` but the `visual-regression.yml` workflow does not.** Inconsistency between two workflows with the same name and purpose.
- **P1: No `cargo audit` step in CI.** Dependency CVEs are unmonitored.
- **P1: `perf-regression.yml` runs benchmarks and compares to a baseline** but stores the baseline as a single artifact with no version pinning.
- **P2: `nightly.yml` builds only on `ubuntu-latest`.** A nightly that's only ever tested on Linux will break on macOS/Windows release days.
- **P2: No `cargo deny` step.** License allowlist, banned dependencies, and advisory checks are absent.
- **P2: `m4_artifact_packaging.rs` and `scripts/package-tauri-artifact.sh` are not covered by the same source of truth.**

---

## 3. Critical Issues (Severity-Ranked)

### 3.1 P0 - fix before any other work

Note: this table intentionally preserves adversarial urgency. A few entries are better read as "verify immediately because they plausibly belong in P0" rather than "already proved exploitable in the running app."

| # | Issue | File | Impact |
|---|-------|------|--------|
| 1 | **No path containment in file operations** | `host_handlers.rs::uri_to_path` + `security::CapabilityRegistry::check` | Tauri command surface appears able to read/write any file the OS allows. Treat as a highest-priority boundary risk and verify concretely in the running app. |
| 2 | **TOCTOU race in `apply_edit_optimistic`** | `buffer/registry.rs::apply_edit_optimistic` | MCP `edit_file` with `expected_version` can race and produce a stale edit. |
| 3 | **`LspClient::Drop` blocks for 5s on dead LSP** | `lsp/client.rs::Drop` | Editor shutdown hangs for 5s x N LSP servers. |
| 4 | **`apply_edits` sends full-document `didChange` to LSP** | `host_handlers.rs::apply_edits` lines 497-508 | Every keystroke forces LSP server to re-parse and re-type-check the entire file. The roadmap defers "incremental LSP sync" as future work; it should be the first P0 fix because it's two lines. |
| 5 | **`save_document_as` does not call LSP `did_close`/`did_open`** | `host_handlers.rs::save_document_as` | LSP state is silently desynchronized after rename. |
| 6 | **Symlink following in `save_document`/`save_document_as`** | `host_handlers.rs` | A symlink under the workspace pointing to `/etc/passwd` gets overwritten. |
| 7 | **Front-end remote-edit guard needs verification under concurrency** | `tauri/index.html` lines 524, 1170 | Historically this used a single global flag; current repo truth should be rechecked before treating this as still-open P0 behavior. |
| 8 | **WASM tokenizer is a hand-rolled heuristic; native uses real tree-sitter** | `wasm/src/tokenize.rs` | The two paths produce different tokens for the same input. |
| 9 | **`EventSubscriptionTracker` is defined, tested, and unused** | `events.rs` | Dead code; the editor has no opt-in event suppression; the test is misleading. This is probably lower severity unless a stronger subscription contract is being claimed. |
| 10 | **Dual JSON/protobuf IPC surface with 18 duplicated handlers** | `main.rs` | The protobuf surface is wired but not consumed by the frontend. Either delete it or commit to it. High maintenance risk; lower immediate severity than the boundary/security items above. |

### 3.2 P1 - performance and correctness debt (37 items, top material)

`LineIndex::new` is O(N^2) on every edit; `get_value_bytes` clones the entire content on every snapshot; `LspClient::request` blocks the Tauri command thread up to 5s; `LspClient::request` allocates a new `mpsc::channel` per call; LSP reader thread is never joined; LSP `request` round-trips through `String`; `apply_edits_json` synchronously re-runs `diagnostics_document` and pushes `diagnostics-changed` after every edit; `apply_edits_json` clones every edit twice; `apply_edits` re-acquires `BufferRegistry::read` three times; `close_document` with `save_if_dirty` is racy; `open_document` re-reads the file just to detect EOL; every `syntax_handlers` call creates a fresh `SyntaxParser`; `tokenize_tree_range` does not range-tokenize; `completion_document` synchronously spawns LSP process on first call; `diagnostics_document` makes synchronous `textDocument/diagnostic` LSP request with no cancellation; MCP `apply_edits_tool` applies edits one at a time with no version check; MCP `edit_file_tool` does not validate `old_text` non-empty; MCP `get_buffer_metadata_tool` does 3 full content reads + SHA-256; MCP `get_symbol_index_tool` re-parses the content on every call; `McpToolRegistry::with_defaults()` rebuilt on every MCP call; `quota_for` always re-`entry().or_insert_with()`; `AuditLog` 10,000-entry cap with no rotation; `wasm/diff.rs::line_diff` is O(N^2) with dead first pass; `wasm/layout.rs::compute_line_layout` is O(N^2); `wasm/layout.rs::compute_visible_lines` walks the entire file on every scroll; WASM token cache holds full source with no eviction; WASM token cache uses full `String::eq` for cache lookup; `wasm/glue.js::applyLineDelta` splits/joins the full file on every delta; `decoration-manager.js::_applyBatch` calls `getAllDecorations()` on every flush; Monaco providers copy-pasted 3x for rust/js/ts; `probeActiveDocumentFeatures` fetches a worker on every cursor move; `console_error_panic_hook` is OFF by default; `--no-typescript` in `build/wasm/build.sh`. Remote-edit guard specifics should be rechecked against current repo truth rather than assumed from the baseline snapshot.

### 3.3 P2 - quality, ergonomics, dead code (47+ items)

Top material: `line_end_offset` returns None; `UndoEntry::clone` is heavy; `open_buffer_from_bytes` doubles content; `McpToolResult.content` vs `data` redundancy; `lsp_completion_to_proto` `format!("{:?}", k)` produces `"completionitemkind::function"`; `audit_log.record` is the hot path; no `cargo audit`/`cargo deny`; no fuzz target; visual regression has no committed baseline; `release.yml` uses deprecated actions; nightly only on Linux; MCP_TRUTH_SURFACE_PLAN describes tools not implemented; many more.

---

## 4. High-Value Optimizations (with code)

### 4.1 Incremental LSP `didChange` (replaces P0 #4)

```rust
// in host_handlers.rs::apply_edits, replace lines 497-508
if let Ok(registry) = state.buffer_registry.read() {
    if let Some(client) = state.lsp_registry.write().ok()
        .and_then(|mut r| r.client_for_language(&language_id).cloned())
    {
        let change = lsp_types::TextDocumentContentChangeEvent {
            range: Some(lsp_types::Range {
                start: lsp_types::Position::new(edit.start_line as u32 - 1, edit.start_column as u32 - 1),
                end:   lsp_types::Position::new(edit.end_line as u32 - 1,   edit.end_column as u32 - 1),
            }),
            range_length: Some(edit.range_length as u32),
            text: String::from_utf8_lossy(&edit.text_utf8).to_string(),
        };
        client.did_change_incremental(&resource.path, version, vec![change]);
    }
}
```
**Impact:** Eliminates the single biggest avoidable cost in the editor. `rust-analyzer` re-parse goes from O(file) per keystroke to O(changed region) per keystroke.

### 4.2 Incremental `LineIndex`

```rust
// in buffer/text_buffer.rs::apply_change, replace line 309
// OLD:  let content = self.rope.to_string();
//       self.line_index = LineIndex::new(&content);
// NEW:  update only the dirty range
self.line_index.update_incremental(
    &self.rope,
    change.start_position.line as usize,
    change.end_position.line as usize,
    change.text.matches('\n').count() + 1,
);
```
**Impact:** 100k-line file: from 5x10^9 char ops per edit to ~1k.

### 4.3 Buffer snapshot via `Arc<[u8]>` to eliminate triple-clone

```rust
// in buffer/text_buffer.rs
pub struct BufferSnapshot {
    pub resource: String,
    pub version_id: u64,
    pub content_utf8: Arc<Vec<u8>>,
    pub eol: String,
    pub is_dirty: bool,
}

impl TextBuffer {
    pub fn get_snapshot_arc(&self) -> BufferSnapshot {
        let bytes: Arc<Vec<u8>> = Arc::new(self.rope.bytes().collect::<Vec<u8>>());
        BufferSnapshot { resource: self.resource.clone(), version_id: self.version_id,
            content_utf8: bytes, eol: self.eol.clone(), is_dirty: self.is_dirty }
    }
}
```
**Impact:** 100MB file metadata fetch: from ~300MB memory traffic to 100MB. Snapshots are now O(1) to share.

### 4.4 Atomic `apply_edit_optimistic` (replaces P0 #2)

```rust
pub fn apply_edit_optimistic(
    &self,
    resource: &str,
    change: &ContentChange,
    expected_version: u64,
) -> Result<Option<ModelContentChangedEvent>, String> {
    let buffer = self.buffers.get(resource)
        .ok_or_else(|| format!("No buffer found for resource: {}", resource))?;
    let mut buffer = buffer.write().map_err(|e| e.to_string())?;  // single write lock
    if buffer.version_id() != expected_version {
        return Err(format!("version conflict: expected {} but found {}",
            expected_version, buffer.version_id()));
    }
    Ok(buffer.apply_change(change))  // commit under the same lock
}
```

### 4.5 Path containment in `host_handlers`

```rust
fn validate_path_under_workspace(state: &MonacoHostState, path: &str) -> Result<PathBuf, String> {
    let canonical = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
    let roots: Vec<PathBuf> = state.workspace_roots().iter()
        .filter_map(|r| r.resource.as_ref().map(|u| PathBuf::from(&u.path)))
        .collect();
    if roots.is_empty() {
        return Err("no workspace roots configured".to_string());
    }
    let canonical_roots: Vec<PathBuf> = roots.iter()
        .filter_map(|r| std::fs::canonicalize(r).ok())
        .collect();
    if !canonical_roots.iter().any(|root| canonical.starts_with(root)) {
        return Err(format!("path {} is not under any workspace root", path));
    }
    let meta = std::fs::symlink_metadata(&canonical).map_err(|e| e.to_string())?;
    if meta.file_type().is_symlink() {
        return Err(format!("refusing to follow symlink at {}", path));
    }
    Ok(canonical)
}
```
Apply this in `open_document`, `save_document`, `save_document_as`, `list_directory`, and the MCP tools. **Impact:** Closes the P0 file-system escape.

### 4.6 Move `McpToolRegistry` into `MonacoHostState`

```rust
pub struct MonacoHostState {
    mcp_registry: McpToolRegistry,
}
```

### 4.7 Debounce diagnostics out of the keystroke path

```rust
let path_clone = request.path.clone();
tauri::async_runtime::spawn(async move {
    tokio::time::sleep(Duration::from_millis(150)).await;
    if let Ok(state) = app.try_state::<MonacoHostState>() {
        // ... recompute diagnostics
    }
});
```

### 4.8 `LspClient::request` non-blocking with `tokio::sync::oneshot` + cancellation

```rust
pub async fn request_async(&mut self, method: &str, params: Value) -> Result<Value, String> {
    let id = { let mut s = self.state.lock().unwrap(); s.next_id += 1; s.next_id.to_string() };
    let (tx, rx) = tokio::sync::oneshot::channel();
    self.pending.lock().unwrap().insert(id.clone(), tx);
    tokio::time::timeout(Duration::from_secs(2), rx).await
        .map_err(|_| format!("LSP request '{}' timed out", method))?
}
```

### 4.9 Cache `SyntaxParser` per language

```rust
pub struct SyntaxParserCache { parsers: Mutex<HashMap<String, Arc<Mutex<SyntaxParser>>>>, }
let parser = cache.get_or_create(language_id)?;
```

### 4.10 WASM: switch to `web-tree-sitter` for tokenizer parity

Ship compiled grammar WASM modules (from `tree-sitter-rust`'s `wasm` target) and use `web-tree-sitter`. The unified tokenizer means MCP agents and Monaco UI see the same token stream.

### 4.11 Hash-based token cache key

```rust
fn sha256_short(s: &str) -> u64 {
    use sha2::{Sha256, Digest};
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    u64::from_le_bytes(h.finalize()[..8].try_into().unwrap())
}
```
**Impact:** 1MB cache lookup: from 1MB byte compare to 8-byte compare.

### 4.12 LRU + size cap for the WASM token cache

`const MAX_CACHE_BYTES: usize = 32 * 1024 * 1024;` with eviction on insert.

### 4.13 Streamed binary sync for large buffers

Replace the single `BufferSnapshot.to_binary()` blob with a chunked protocol: `[mode=2][total_chunks:4][chunk_index:4][chunk_count:4LE][chunk_len:4LE][chunk:utf8]`.

### 4.14 Protobuf payload-type discriminator

```protobuf
enum PayloadType {
  PAYLOAD_TYPE_UNSPECIFIED = 0;
  PAYLOAD_TYPE_OPEN_DOCUMENT_REQUEST = 1;
  PAYLOAD_TYPE_APPLY_EDITS_REQUEST = 2;
}
message IpcRequest { uint32 id = 1; PayloadType payload_type = 2; bytes payload = 3; }
```

### 4.15 `set_primary_workspace_root` must restart the LSP

```rust
let mut lsp = state.lsp_registry.write().map_err(|e| e.to_string())?;
lsp.set_workspace_root(path_to_uri.path);
```

### 4.16 Use `parking_lot::RwLock`

Drop-in replacement for `std::sync::RwLock`. No poison, faster, eliminates a class of "lock poisoned" panics.

### 4.17 Move `McpToolError` into the proto layer

`McpError { code, message, kind }` proto message so non-Rust agents can distinguish `VersionConflict` from `BufferNotFound` from `InvalidEdit`.

### 4.18 Add structured error code to all Tauri command responses

`AppError { code: ErrorCode, message: String, recoverable: bool, details: Value }` proto + JSON conversion. Frontend can distinguish errors without string parsing.

### 4.19 Frontend: per-buffer remote-edit flag

Replace the single `state.isApplyingRemoteEdits` boolean with a `Set<String>` of buffers currently receiving remote edits, keyed by path.

### 4.20 Generate TS bindings for the WASM module

Drop `--no-typescript` from `build/wasm/build.sh` line 21. Generate `.d.ts` files so the `apply_binary_delta` return type is statically checked.

---

## 5. Recommended New MCP Tools

The repo's own `MCP_TRUTH_SURFACE_PLAN.md` is the blueprint. The following are the most material additions, with the rationale each.

1. **`compare_buffer_versions`** — exact diff between two version_ids. Returns changed ranges, whether the comparison is exact or reconstructed. Turns versions into lineage, not just counters. Today, `get_buffer_version_lineage_tool` honestly reports that no version history is stored; this tool is the next step toward making the lineage real.

2. **`subscribe_buffer_events`** + **`poll_buffer_events`** — register interest in (content_changed, saved, opened, closed, diagnostics_changed) per path; cursor/sequence-based replay. Makes external semantic/runtime agents observers of truth instead of blind pollers. The Rust `EventSubscriptionTracker` (currently dead code) is the natural backend; this is the chance to wire it up rather than remove it.

3. **`query_symbols`** — query by name, kind, prefix, parent, exact range overlap. Turns symbol extraction into a reusable queryable surface, not just a `list_symbols`/`get_symbol_index` pair. Backed by a parse cache that the existing tools should also use.

4. **`preview_structural_edit`** + **`structural_edit`** — rename symbol, replace function body, insert/delete item, gated on exact symbol presence. Preview before mutation. If the runtime cannot prove the structural target exactly, the operation must fail clearly instead of silently degrading to fuzzy text replacement.

5. **`inspect_buffer_drift`** — buffer vs saved file; symbol set vs previous version; diagnostics vs previous diagnostics. Returns exact categories of change, exact ranges/symbols affected, certainty class.

6. **`inspect_contradictions`** — symbol declared but not structurally present after change; diagnostics claim parse break while symbol query still assumes stability; external semantic report references missing ranges/symbols. First-class way to say "these truth surfaces disagree".

7. **`watch_workspace`** — file-system change subscription. Folds `ipc_file.proto::WatchRequest` into the MCP envelope. Returns watcher_id + a poll cursor; supports recursive watch with excludes.

8. **`replay_buffer`** — return the in-memory content at a specific version_id. Requires the historical store P2 in `get_buffer_version_lineage_tool` to land. Once history exists, replay is the most useful tool for reasoning systems.

9. **`diff_files`** — cross-file semantic diff via shared parse tree. Two paths, parsed once, symbols matched by FQN. For refactor tools.

10. **`search_text_in_buffers`** — workspace-wide content search with per-buffer line ranges and content_sha256 for the matched range. Uses the WASM `Vec<WasmToken>` style with no allocation per match. A building block for any reasoning system that needs to locate code.

11. **`list_open_buffers`** + **`close_buffers_batch`** — agent-side workflow: see what's open, close a set of buffers atomically. Today the agent has no introspection into the buffer registry's contents.

12. **`get_buffer_byte_range`** — return a `Vec<u8>` for `[byte_offset, byte_offset+length]`. The `Arc<Vec<u8>>` snapshot makes this O(1) to share. Required for any large-file content inspection without the full content.

13. **`McpToolError` typed errors** — every tool should be able to return `BufferNotFound` / `VersionConflict` / `InvalidEdit` / `PermissionDenied` / `PathOutOfWorkspace` distinctly. The plan is right that this is a "must" for a truth-bearing surface.

14. **`McpEnvelope::status: "ok" | "partial" | "unavailable"`** — a third status alongside `success: bool`. A tool that succeeds but has heuristic-only data should return `status: "partial"` with `certainty: Heuristic`, not `status: "ok"` with `certainty: Heuristic`. The latter lies.

---

## 6. DX / Tooling Recommendations

### 6.1 CI
- Add `cargo audit` + `cargo deny` steps in `build.yml`.
- Pin GitHub Actions by SHA (supply-chain).
- Replace deprecated `actions/create-release@v1` and `actions/upload-release-asset@v1` in `release.yml` with `softprops/action-gh-release@v2` or `gh release create` via `gh-cli`.
- Commit visual-regression baseline PNGs; today the pipeline runs but compares against nothing.
- Run nightlies on all targets (Linux + macOS + Windows), not just `ubuntu-latest`.
- Pick one visual-regression workflow: `build.yml::visual-regression` has `tauri-driver`; `visual-regression.yml` doesn't. Resolve to one and delete the other.
- Add a job that runs `cargo bench --bench buffer_bench` and fails on >20% regression (the current `perf-regression.yml` only compares to a stored artifact baseline, not a versioned one).

### 6.2 Tests
- Add `proptest` over `TextBuffer::apply_change` + `LineIndex` round-trips. Buffer content is adversarial; the round-trip property ("apply N random edits, then read back, get the same content") is missing.
- Add `cargo-fuzz` targets: `tokenize_source` (WASM), `apply_change` (Rust), `compute_line_delta` (Rust). Buffer content from the filesystem is untrusted.
- Fix `m7_buffer_bench.rs`: the `bench_small_file_insert` benchmark calls `TextBuffer::new` per iteration, which dominates. Either measure edit-in-place or factor out construction.
- Fix `e2e_lsp_fallback.rs`: the LSP fallback tests bypass the `did_open` path by writing to the registry directly. Use `open_document` and observe the LSP `didChange`.
- Fix `m4_integration.rs::security_permission_denial_blocks_operations`: the test passes for the wrong reason. It should test the path-containment check.

### 6.3 Workspace layout
- Move `MCP_TRUTH_SURFACE_PLAN.md` and `CAPABILITY_DECLARATION.md` from top-level narrative into a `docs/` directory, or keep at root with consistent frontmatter.
- Add a `MCP_ENVELOPE.md` derived from the `McpToolResult` proto; the certainty/provenance pattern is the right shape, the doc is the contract.
- Extract a `monaco-tauri-types` crate so proto types are usable from external agents without a Tauri dep.
- Collapse the three `parser_for_path` dispatch tables (`lsp::registry`, `syntax_handlers`, `mcp::tools`) into one. A single `LanguageRegistry::for_path(path: &str) -> LanguageId` function.
- Collapse the duplicated JSON/protobuf IPC into one. The proto surface is wired but not consumed; commit to JSON (current frontend) and delete the proto handlers, OR commit to proto and rewrite the frontend's `invoke("apply_edits_json", ...)` to a binary path.

### 6.4 Naming
- Rename `McpToolResult.content` to `summary` and clarify it as optional human text; the canonical payload is `data`.
- Rename `BufferChangeEvent` to be the same struct as `editor::ModelContentChange`; today they are two parallel type hierarchies.
- Rename `BufferRegistryEvent` (duplicated between `ipc_editor.proto` and `ipc_editor_host.proto`) to a single canonical form with explicit `kind` discriminator.
- Use `Edition` instead of `BufferSnapshot.is_dirty` to align with the LSP and Monaco "dirty" terminology.

### 6.5 Documentation
- `HYDRATION.md` accurately says "WASM lane is still a secondary track, not the authoritative runtime". Promote this honesty to the README.
- The WASM roadmap's "Current Reality Check" table is the most honest documentation in the repo. Keep it.
- The phased roadmap's optimistic "Phases 1-7 complete" checkboxes contradict the hydration doc. Reconcile: either re-tick them honestly with sub-percentages, or replace with a "what's verified" list.
- Add a `ARCHITECTURE.md` auto-generated from the proto + module structure (can be hand-written for now, generated later) so the architecture map isn't a one-time artifact.

### 6.6 Dependency hygiene
- Update `Cargo.toml` to pin `lsp-types` to a version compatible with `rust-analyzer` (currently `0.97` is older; check whether `rust-analyzer-protocol` is the right move).
- Add `dev-dependencies`: `proptest`, `insta` (for snapshot tests), `wiremock` or `mockito` (for LSP server mocking in integration tests).
- The `wasm-bindgen` / `serde-wasm-bindgen` pair should be checked together; mixing minor versions across `wasm/` and the build script is a source of subtle ABI drift.

---

## 7. Final Synthesis

### 7.1 Maturity verdict
Monaco_Rust is at a credible **v0.1-rc** maturity. The Rust substrate is real, well-organized, and tested at the unit level (167 verified tests, 139 library/unit, 14 protobuf roundtrip, 7 benchmark, 6 integration, 1 packaging). The architecture is sound: a clean separation between authoritative native state and a Monaco presentation layer, with explicit protobuf contracts and a meaningful MCP surface.

What is missing is execution quality:
- **10 P0s**, half of them security holes and half of them corrupted runtime contracts (full-document LSP didChange, dual-encoding IPC, dead `EventSubscriptionTracker`).
- **37 P1s** of performance and correctness debt, dominated by the `LineIndex` O(N²) per edit, the LSP `request` blocking the command thread, the WASM token cache using full `String::eq` for lookup, and the WASM `line_diff` being O(N²) with a dead first pass.
- **47+ P2s** of quality, ergonomics, and dead code.

The WASM and JS sides are prototype-grade, not production. The repo is at a v0.1 checkpoint: usable as a research substrate, not safe for distribution.

### 7.2 What is excellent
- `src-tauri/src/lib.rs` and `src-tauri/src/proto.rs` — clean module boundaries, generated proto included via OUT_DIR.
- `src-tauri/src/security/sandbox.rs::SandboxLimits::strict()` / `permissive()` — sensible posture presets.
- `src-tauri/src/buffer/undo.rs` — clean transaction model.
- `src-tauri/src/syntax/symbols.rs` — proper Tree-sitter-backed symbol extraction with hierarchy.
- `src-tauri/src/wasm_sync.rs::to_binary()` / `apply_binary_delta` — actual zero-copy-friendly binary sync, the only true binary path in the system.
- `src-tauri/src/mcp/tools.rs::McpToolResult::with_truth` + `McpCertainty` + `McpResultProvenance` — the certainty/provenance pattern is exactly the right shape for a truth-bearing surface; the tools just need to use it more aggressively.
- `MCP_TRUTH_SURFACE_PLAN.md` — by far the most thoughtful design document in the repo; it correctly identifies the next tranche of work and the anti-goals. It's a blueprint; the code has only done the first 30%.
- The build-path emancipation from Node (`build/wasm/build.sh`, `scripts/prepare-tauri-dist.sh`, vendored Monaco assets) is done well and the truth in `HYDRATION.md` is honest about the boundary.

### 7.3 Path to v1
1. **Sprint 1 - security + correctness (about 1 week):** P0 #1 path containment, P0 #2 atomic optimistic, P0 #3 Drop non-blocking, P0 #5 LSP did_close/did_open on save_as, P0 #6 symlink rejection, P0 #7 per-buffer remote-edit flag, P0 #9 remove dead `EventSubscriptionTracker` or wire it.

2. **Sprint 2 - performance foundation (about 1 week):** P0 #4 incremental LSP didChange (2 lines), P0 #8 web-tree-sitter in WASM, P0 #10 collapse dual-encoding IPC, the P1 buffer/index optimizations (4.2 incremental LineIndex, 4.3 Arc snapshot, 4.7 debounce diagnostics, 4.9 parser cache).

3. **Sprint 3 - MCP v2 (about 2 weeks):** the new MCP tools in section 5, structured error envelope, payload-type discriminator in proto, version lineage store.

4. **Sprint 4 - tooling (about 1 week):** property tests, fuzz targets, cargo-audit/deny, release.yml action updates, committed visual-regression baselines, cross-platform nightlies, consolidated dual visual-regression workflow.

5. **Sprint 5 - the deferred WASM/custom-renderer lane:** this is the W2.x work from `WASM_ROADMAP.md` - virtual scrolling, custom renderer, inlay hints, in-editor parameter hints. But it should only be attempted after Sprints 1-3 land, otherwise the substrate it sits on is fragile.

### 7.4 Final note

The repo is at a credible v0.1 checkpoint. The Rust runtime is real, the protobuf contracts are explicit, and the MCP surface is pointed in the right direction. The blockers to a usable v1 are ten P0s and a focused week of performance work, all listed above with code. The deferred WASM/custom-renderer lane is correctly deferred - don't chase it until the substrate is solid.

Two things deserve final emphasis:

1. **The single most expensive P0 (#4) is two lines of code.** `range = { start, end }` instead of `range = null` in `apply_edits` is the difference between a keystroke that takes 100ms (full re-parse in rust-analyzer) and one that takes <1ms (incremental). The roadmap defers it as "incremental LSP sync" future work, but it's not future work, it's a one-line change. This is the highest-leverage P0 in the entire review.

2. **The single most expensive P1 is the O(N²) `LineIndex::new` on every edit.** For a 100k-line file, every keystroke does ~5x10^9 UTF-16 conversions. The fix is to use `Rope`'s built-in line table and incremental updates. This is the difference between Monaco being responsive on large files and being unusable on them. Fix this before any v1 release.

The path is clear. The work is bounded. The substrate is good. Execute.
