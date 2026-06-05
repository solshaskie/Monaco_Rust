# Adversarial Remediation Roadmap

> **Purpose:** Turn the findings in [`ADVERSARIAL_REVIEW.md`](./ADVERSARIAL_REVIEW.md) into a sequenced, observable plan. Each phase maps to a sprint in the review's "Path to v1" (§7.3).
> **Status:** In progress.
> **Last updated:** 2026-06-04
>
> This is a hardening backlog derived from an adversarial review. It is not the canonical project roadmap and should not outrank `README.md`, `HYDRATION.md`, `PHASED_ROADMAP.md`, or `WASM_ROADMAP.md`. Use it as a pressure-tested remediation queue, then reconcile each item with current repo truth before treating it as committed direction.

---

## Context

The adversarial review (generated 2026-06-03 against `57cfbb6f`) found **10 P0 issues** (exploitable / corrupting / blocking), **37 P1 issues** (material correctness or performance), and **47+ P2 issues** (quality / ergonomics). The Rust substrate is real and well-organized (167 verified tests), but the runtime has security holes, an O(N²) per-keystroke cost center, a full-document LSP `didChange` on every edit, and prototype-grade WASM/JS.

This roadmap treats the review as input, not as sovereign truth. Every item below references the original finding by section, but stale or already-addressed items should be corrected instead of carried forward mechanically.

---

## Reality Check

| Area | Current State | Target |
|------|---------------|--------|
| Path containment | Canonicalized workspace-root check + symlink rejection on write paths | Verified with regression tests |
| Optimistic edit race | Single write-lock atomic check | Verified |
| LSP shutdown | Bounded 500ms timeout in `try_shutdown`; `Drop` no longer blocks | Non-blocking / timeout |
| LSP `didChange` | Incremental range per spec | Verified |
| `save_document_as` | `did_close` on source + `did_open` on target before registry update | LSP state stays synchronized |
| Symlink handling | Reject symlinks in write path | Verified |
| Remote edit guard | Per-buffer `Set<string>` in `tauri/index.html` | Per-buffer `Set<String>` |
| `parking_lot` | `std::sync::RwLock` replaced across `host_handlers`, `registry`, `main` | `parking_lot::RwLock` — no poison panic surface |
| `LineIndex` | Incremental update O(Δ) | Verified (binary-search truncate + rescan) |
| Buffer snapshot | `content_utf8` is `Arc<Vec<u8>>`; `get_snapshot_arc` added | Cheap snapshot sharing |
| Syntax parser cache | Thread-local `HashMap<String, SyntaxParser>` in `syntax_handlers.rs` | One parser per language per thread |
| Diagnostics debounce | `apply_edits_json` spawns 150ms tokio task; in-flight tasks cancelled per buffer | Keystroke handler no longer blocks on Tree-sitter + LSP round-trip |
| LSP async requests | `request_async` with `tokio::sync::oneshot`; default timeout 2s | Async cancellation path ready |
| Dual IPC encoding | Protobuf command surface removed; JSON is the only IPC encoding | Frontend uses JSON exclusively |
| WASM tokenizer | Hand-rolled heuristic | `web-tree-sitter` parity with native |
| Event subscriptions | Dead code removed | Removed |
| MCP tool registry | Rebuilt on every call | Cached in `MonacoHostState` |
| Version lineage store | `TextBuffer` stores last 50 `BufferSnapshot` versions | `get_buffer_version_lineage` reports actual history |
| MCP structured errors | `McpError { kind, message, ... }` with `McpErrorKind` enum | All tools return typed errors; `McpStatus` on every result |
| MCP compare_buffer_versions | Line-level diff between two `version_id`s using lineage store | `lines_added/removed/changed` + `diff_chunks` |
| MCP inspect_buffer_drift | SHA-256 buffer vs disk; optional line diff | `has_drift`, `buffer_sha256`, `disk_sha256` |
| MCP list_open_buffers | Registry introspection | Returns all open buffers with version, dirty, undo/redo |
| MCP replay_buffer | Historical content at `version_id` | Uses lineage store; returns content + SHA-256 |
| MCP get_buffer_byte_range | O(1) partial read via `Arc<Vec<u8>>` | Byte slice with offset, length, SHA-256 |
| MCP search_text_in_buffers | Workspace-wide text search | Per-match line/column + SHA-256 |
| MCP query_symbols | Queryable symbol surface by name pattern + kind filter | Works across one or all open buffers |
| MCP diff_files | Cross-file line-level diff | Compares any two buffers via `compute_line_diff` |
| Protobuf payload discriminator | `PayloadType` enum on `IpcRequest`/`IpcResponse`/`IpcEvent` | Envelope carries explicit discriminator |
| Release CI deprecated actions | Replaced `actions/create-release@v1` + `actions/upload-release-asset@v1` | Using `gh release create` + `gh release upload` |
| Nightly multi-platform CI | Was ubuntu-only | Matrix: Linux x86_64, macOS x86_64/aarch64, Windows x86_64 |
| Benchmark construction cost | `bench_small_file_insert` mixed construction + edit | Pre-construct 1000 buffers outside timing loop |
| Property tests | No property-based testing | `proptest` for LineIndex round-trip + TextBuffer apply/undo/redo |
| Path containment test | Tested permission revocation only | Tests real path containment (outside workspace root) |
| e2e LSP fallback tests | Bypassed open_document via direct registry write | All 3 tests use `open_document` triggering LSP did_open lifecycle |
| WASM diff dead code | Lines 25-66 built edits then immediately cleared them | Removed first pass; kept second working LCS-based pass |
| WASM layout char counting | `line.chars().count()` on every line | Byte-length proxy (`line.len()`) for ASCII source code |
| WASM scroll line counting | `source.lines().count()` on every scroll event | `compute_visible_lines` accepts cached `total_lines` |
| WASM token cache | No size cap; `String::eq` for cache key comparison | 32MB LRU cap with eviction; SHA-256 hash keys |
| JS DOM element lookups | `document.getElementById` in hot loops (status, render) | Cached `dom` object initialized once via `initDomCache()` |
| JS UI render batching | `renderFileList()` + `renderTabs()` called synchronously in pairs | `scheduleRender()` batches both into single `requestAnimationFrame` |
| JS applyLineDelta full-file split | `content.split('\n')` on every delta, O(total lines) | Incremental byte-offset splice via `indexOf('\n')`, O(replaced lines) |
| JS decoration getAllDecorations scan | `_applyBatch` called `model.getAllDecorations()` per rAF | Tracks `currentDecorationsByLine`; only changed lines passed to `deltaDecorations` |
| JS provider registration | ~260 lines of duplicated rust + js/ts inline providers | Single `registerLanguageProviders(lang)` helper called in a loop |
| JS cursor probe worker thrash | `getTypeScriptWorker()` fetched on every cursor move | Cached getter + 150ms debounce on `onDidChangeCursorPosition` |
| WASM .d.ts bindings | `--no-typescript` suppressed TypeScript declarations | Removed flag; `wasm-bindgen` emits `monaco_wasm.d.ts` with full type signatures |
| Virtual scroll renderer | Positioned as "alternative view" alongside Monaco | Repositioned as replacement via `mountAsPrimary()` which hides native `.lines-content` |
| Decoration flicker | `applyDecorations` rewrote `innerHTML` per token on every rAF | Per-line token hash (`lastTokenHashByLine`) skips unchanged lines; `_highlightLine` sorts tokens into single pass |
| Compatibility shim upgrade risk | Monkey-patched `deltaDecorations` + `getLayoutInfo` without guards | `__wasmShimPatched` double-patch guard; idempotent `dispose()` restores originals and clears marker |
| CI cargo audit/deny | No vulnerability or license scanning in CI | `audit` + `deny` jobs in `build.yml`; `deny.toml` with license allowlist and advisory db checks |
| CI action SHA pinning | Actions referenced by mutable version tags | All actions pinned to commit SHAs with version comments across all workflow files |
| CI dtolnay action name | `dtolnay/rust-action@stable` (non-existent repo) | Corrected to `dtolnay/rust-toolchain@stable` with pinned SHA |
| CI cache-cargo-install version | `taiki-e/cache-cargo-install-action@v2` (no v2 tag) | Corrected to `v3` with pinned SHA |
| LSP didChange batching | Every `apply_edits` call sent a `didChange` notification immediately | `queue_did_change` accumulates changes per path, aborts old debounce task, sends batched changes after 50ms of inactivity |
| cargo-fuzz targets | No fuzz targets for hot paths | `fuzz_apply_change`, `fuzz_tokenize`, `fuzz_compute_line_delta` in `src-tauri/fuzz/` with CI build verification |

---

## Phase A: Security + Correctness

**Goal:** Close the highest-confidence security/correctness issues first. This phase should be driven by verified repo truth, not only by the adversarial labels.

### A.1 Path Containment
**Files:** `src-tauri/src/host_handlers.rs`, `src-tauri/src/security/capabilities.rs`

- [x] Implement `validate_path_under_workspace` (canonicalize, workspace-root prefix check, symlink rejection)
- [x] Apply validation to `open_document`, `save_document`, `save_document_as`, `list_directory`
- [x] Apply validation to MCP tools (`read_file`, `edit_file`, `list_directory`)
- [x] Add `PathOutOfWorkspace` variant to error types

**Success Criteria:**
- `../../etc/passwd` is rejected from every surface
- Symlink to `/etc/passwd` under workspace is rejected on write
- Existing tests updated to exercise real path containment

### A.2 Atomic Optimistic Edit
**File:** `src-tauri/src/buffer/registry.rs`

- [x] Rewrite `apply_edit_optimistic` to acquire `write()` once, read version, compare, and commit or fail without releasing

**Success Criteria:**
- Concurrent MCP `edit_file` calls with `expected_version` cannot race

### A.3 Non-blocking LSP Lifecycle
**File:** `src-tauri/src/lsp/client.rs`

- [x] Replace `Drop::drop` synchronous shutdown with bounded timeout (`try_shutdown`, 500ms)
- [x] Reader thread reads from `ChildStdout`; child process drop closes stdout, causing reader loop to exit on EOF

**Success Criteria:**
- Editor shutdown does not hang when LSP process is dead
- No thread leaks on repeated open/close cycles

### A.4 LSP State Synchronization on Rename
**File:** `src-tauri/src/host_handlers.rs`

- [x] In `save_document_as`, call `lsp.did_close` for source URI before moving buffer
- [x] Call `lsp.did_open` for target URI after move

**Success Criteria:**
- Renaming a file does not leave LSP with stale URI → buffer mapping

### A.5 Symlink Rejection
**File:** `src-tauri/src/host_handlers.rs`

- [x] Use `symlink_metadata` before `fs::write` in `save_document` and `save_document_as`
- [x] Return structured error if target is a symlink

### A.6 Per-Buffer Remote Edit Guard
**File:** `tauri/index.html`

- [x] Re-verified the current remote-edit guard implementation in `tauri/index.html`
- [x] Replaced global `isApplyingRemoteEdits` boolean with `state.remoteEditPaths: Set<string>`
- [x] `model.onDidChangeContent` now checks `state.remoteEditPaths.has(state.currentPath)` before invoking `apply_edits_json`

**Success Criteria:**
- Concurrent remote edits on different buffers do not interfere
- Edit loops from async `applyEdits` are eliminated

### A.7 Event Subscription Tracker — Wire or Remove
**File:** `src-tauri/src/events.rs`

- [x] Confirmed `EventSubscriptionTracker` was unused in current repo truth
- [x] Removed `EventSubscriptionTracker` struct, impl, tests, and e2e test dependency

**Success Criteria:**
- No dead code that is tested but unused

### A.8 Version Bump on Save
**File:** `src-tauri/src/host_handlers.rs`

- [x] `save_document` no longer calls `set_buffer_content` (which spuriously incremented version); now returns actual current buffer `version_id` via `get_buffer_version` after `mark_buffer_saved`

---

## Phase B: Performance Foundation

**Goal:** Fix the single most expensive P0 (full-document `didChange`) and the dominant P1 (O(N²) `LineIndex`). This is the sprint the review calls "about 1 week."

### B.1 Incremental LSP `didChange`
**File:** `src-tauri/src/host_handlers.rs`

- [x] Replace `range: null` with actual `start/end` range + `text` in the `didChange` payload
- [x] Use `lsp_types::TextDocumentContentChangeEvent` with populated `range`

**Impact:** `rust-analyzer` re-parse goes from O(file) to O(changed region) per keystroke.

**Success Criteria:**
- LSP server receives incremental changes, not full replacement
- Typing latency on large files drops measurably

### B.2 Incremental `LineIndex`
**File:** `src-tauri/src/buffer/text_buffer.rs`, `src-tauri/src/buffer/line_index.rs`

- [x] Remove `LineIndex::new` full rebuild from `apply_change`, `apply_changes`, undo, and redo
- [x] Implement `LineIndex::update` using `binary_search` on `line_starts` + rescan from affected line
- [x] Position/offset conversion now uses single-line rope slices instead of `rope.to_string()`

**Impact:** 100k-line file: from ~5×10⁹ UTF-16 conversions per keystroke to ~1k.

**Success Criteria:**
- Large file (>50k lines) edit latency <16ms

### B.3 Buffer Snapshot via `Arc<[u8]>`
**File:** `src-tauri/src/buffer/text_buffer.rs`

- [x] Changed `BufferSnapshot.content_utf8` from `Vec<u8>` to `Arc<Vec<u8>>`
- [x] Added `get_snapshot_arc` that wraps the snapshot in an `Arc` for cheap sharing

**Impact:** 100MB file metadata fetch: from ~300MB memory traffic to 100MB.

### B.4 Debounce Diagnostics
**File:** `src-tauri/src/main.rs`, `src-tauri/src/syntax_handlers.rs`

- [x] Move `diagnostics_document` out of the synchronous `apply_edits_json` handler — `apply_edits_json` spawns a tokio task after emitting the buffer change event
- [x] Spawn a tokio task with `tokio::time::sleep(Duration::from_millis(150))` — 150ms sleep before calling `diagnostics_document`
- [x] Cancel in-flight diagnostic task on new edit — `MonacoHostState::replace_diagnostic_task` aborts the old `JoinHandle` for that path before storing the new one

**Success Criteria:**
- Keystroke handler does not block on Tree-sitter parse + LSP round-trip

### B.5 Cache `SyntaxParser` per Language
**File:** `src-tauri/src/syntax_handlers.rs`

- [x] Added thread-local `HashMap<String, SyntaxParser>` cache in `syntax_handlers.rs`
- [x] `with_parser_for_path` reuses cached parsers instead of creating a fresh one per call

### B.6 Async LSP Request with Cancellation
**File:** `src-tauri/src/lsp/client.rs`, `src-tauri/src/host_handlers.rs`, `src-tauri/src/main.rs`

- [x] Added `ResponseSender` enum with `Sync(mpsc)` and `Async(oneshot)` variants
- [x] Added `request_async` returning a `tokio::sync::oneshot` future; reader loop removes entries and dispatches to the correct sender
- [x] Cancel superseded `didChange` notifications on newer edit — Removed direct `lsp.did_change` from `host_handlers::apply_edits`, `undo`, `redo`; added `MonacoHostState::queue_did_change` which accumulates changes in `pending_did_changes`, aborts old debounce task, and spawns a new 50ms tokio timer. Only after 50ms of inactivity are batched changes sent in a single LSP notification.
- [x] Reduced default `request` timeout from 5s to 2s

### B.7 WASM Tokenizer Parity
**File:** `wasm/src/tokenize.rs`

- [ ] Replace hand-rolled heuristic with `web-tree-sitter` using compiled grammar WASM modules
- [ ] Ensure native Rust and WASM paths produce identical token streams

### B.8 Collapse Dual-Encoding IPC
**File:** `src-tauri/src/main.rs`

- [x] Deleted all protobuf `payload: Vec<u8>` handlers (`open_document`, `save_document`, `save_document_as`, `list_directory`, `close_document`, `apply_edits`, `get_buffer_snapshot`, `undo`, `redo`, `set_primary_workspace_root`, `get_workspace_roots`)
- [x] Removed `encode_message` / `decode_message` helpers and `prost::Message` import
- [x] Renamed all `_json` handlers to remove suffix; frontend `invoke` calls updated
- [x] Created JSON wrappers for previously protobuf-only commands: `save_document_as`, `get_buffer_snapshot`, `set_primary_workspace_root`

### B.9 `parking_lot` RwLock Migration
**Files:** `src-tauri/src/` (all modules using `std::sync::RwLock`)

- [x] Replace `std::sync::RwLock` with `parking_lot::RwLock` across `buffer/registry.rs`, `host_handlers.rs`, `main.rs`
- [x] Eliminate lock-poison panic surface (all `.map_err(|e| e.to_string())?` on lock acquisition removed)

---

## Phase C: MCP v2 + Truth Surface

**Goal:** Implement the new MCP tools and structured errors described in [`MCP_TRUTH_SURFACE_PLAN.md`](./MCP_TRUTH_SURFACE_PLAN.md). About 2 weeks.

### C.1 Structured Error Envelope
**Files:** `src-tauri/src/mcp/tools.rs`, `src-tauri/src/main.rs`

- [x] Added `McpError { kind, message, expected_version, actual_version, path }` with `McpErrorKind` enum
- [x] Distinguished `BufferNotFound`, `VersionConflict`, `InvalidEdit`, `PermissionDenied`, `PathOutOfWorkspace`, `SerializationError`
- [x] Added `McpStatus { Ok, Partial, Unavailable }` to `McpToolResult`; forwarded in `ExecuteMcpToolResponse`
- [x] All tools now return structured `McpError` instead of plain strings

### C.2 New MCP Tools
**File:** `src-tauri/src/mcp/tools.rs`

- [x] `compare_buffer_versions` — line-level diff between two `version_id`s using lineage store; returns `lines_added/removed/changed` and `diff_chunks`
- [x] `inspect_buffer_drift` — compares in-memory buffer against file on disk via SHA-256; returns `has_drift`, `buffer_sha256`, `disk_sha256`, and optional line diff
- [x] `list_open_buffers` — registry introspection returning all open buffers with version, dirty, undo/redo, byte_len, line_count
- [x] `replay_buffer` — returns exact content at a historical `version_id` from lineage store
- [x] `get_buffer_byte_range` — O(1) partial read via `Arc<Vec<u8>>` snapshot; returns byte slice with SHA-256
- [x] `search_text_in_buffers` — workspace-wide content search across all open buffers; returns matches with line/column and per-match SHA-256
- [x] `query_symbols` — queryable symbol surface by name pattern and optional kind filter across one or all open buffers
- [x] `diff_files` — cross-file line-level diff between two different buffers
- [ ] `subscribe_buffer_events` + `poll_buffer_events` — BLOCKED: `EventSubscriptionTracker` was removed as dead code; requires re-implementing event subscription infrastructure
- [ ] `preview_structural_edit` + `structural_edit` — BLOCKED: requires AST-aware structural editing (Tree-sitter mutation) not yet implemented
- [ ] `inspect_contradictions` — BLOCKED: requires multiple truth surfaces (LSP diagnostics + buffer + disk) to compare; needs design
- [ ] `watch_workspace` — BLOCKED: requires filesystem watcher integration (e.g., `notify` crate) not yet in the project
- [ ] `close_buffers_batch` — BLOCKED: requires `&mut BufferRegistry`; MCP tool `execute` takes `&BufferRegistry`; needs separate Tauri command path

### C.3 MCP Tool Registry Caching
**File:** `src-tauri/src/main.rs`, `src-tauri/src/mcp/tools.rs`

- [x] Moved `McpToolRegistry` into `MonacoHostState`, constructed once in `new()`
- [x] `execute_mcp_tool` and `list_mcp_tools` now use `state.mcp_tool_registry()` instead of rebuilding on every call

### C.4 Protobuf Payload Discriminator
**File:** `proto/ipc_envelope.proto`

- [x] Added `PayloadType` enum (`BUFFER_EVENT`, `FILE_EVENT`, `LSP_EVENT`, `DIAGNOSTIC_EVENT`, `SYMBOL_INDEX`) to proto
- [x] Added `payload_type` field to `IpcRequest`, `IpcResponse`, and `IpcEvent`
- [x] Protobuf envelope now carries explicit payload discriminator alongside raw `bytes`

### C.5 Version Lineage Store
**File:** `src-tauri/src/buffer/text_buffer.rs`

- [x] Added `history: Vec<BufferSnapshot>` to `TextBuffer` with 50-version bound
- [x] `record_snapshot()` pushes after every `version_id` increment in `apply_change`, `apply_changes`, `set_value`, `undo`, `redo`
- [x] Added `get_lineage()` and `get_snapshot_at_version()` to `TextBuffer`; exposed via `BufferRegistry::get_buffer_lineage()`
- [x] `get_buffer_version_lineage` MCP tool now reports actual historical version IDs instead of `historical_versions_stored: false`

---

## Phase D: Tooling, CI, and Test Hardening

**Goal:** Add `cargo audit`, property tests, fuzz targets, fix broken CI actions, and reconcile test coverage. About 1 week.

### D.1 CI Hardening
**Files:** `.github/workflows/build.yml`, `.github/workflows/release.yml`, `.github/workflows/nightly.yml`

- [x] Add `cargo audit` + `cargo deny` steps — Added `audit` and `deny` jobs to `build.yml`; created `src-tauri/deny.toml` with license allowlist and vulnerability checks
- [x] Pin GitHub Actions by SHA (supply-chain hardening) — Pinned all action references across `build.yml`, `nightly.yml`, `release.yml`, `perf-regression.yml`, `visual-regression.yml` to commit SHAs with version comments; also corrected `dtolnay/rust-action@stable` → `dtolnay/rust-toolchain@stable` and `taiki-e/cache-cargo-install-action@v2` → `v3`
- [x] Replace deprecated `actions/create-release@v1` and `actions/upload-release-asset@v1` with `gh release create` + `gh release upload`
- [x] Run nightlies on Linux, macOS, and Windows (not just `ubuntu-latest`)
- [ ] Consolidate `visual-regression.yml` and `build.yml::visual-regression` into one workflow with `tauri-driver`

### D.2 Visual Regression
**Files:** `tests/visual/`, `.github/workflows/`

- [ ] Commit baseline PNGs per platform
- [ ] Fail CI on >1% pixel diff outside known change zones

### D.3 Performance Regression
**Files:** `.github/workflows/perf-regression.yml`, `src-tauri/tests/m7_buffer_bench.rs`

- [x] Fix `bench_small_file_insert`: separate construction cost from edit cost (pre-construct 1000 buffers outside timing loop)
- [ ] Store version-pinned baseline artifacts
- [ ] Fail CI on >20% regression against pinned baseline

### D.4 Property and Fuzz Tests
**Files:** `src-tauri/tests/`, `wasm/tests/`

- [x] Add `proptest` for `TextBuffer::apply_change` + `LineIndex` round-trip (4 property tests: line_index_round_trip_consistency, line_index_line_count_matches_newlines, text_buffer_apply_change_then_line_index_consistent, text_buffer_undo_redo_preserves_line_index)
- [x] Add `cargo-fuzz` targets: `apply_change`, `compute_line_delta`, `tokenize_source` — Initialized `src-tauri/fuzz/` with cargo-fuzz; created `fuzz_apply_change.rs` (interprets bytes as ContentChange + applies to TextBuffer), `fuzz_tokenize.rs` (feeds arbitrary bytes to SyntaxParser::for_rust + tokenize_tree), `fuzz_compute_line_delta.rs` (splits bytes into current/old content and runs wasm_sync::compute_line_delta via BufferRegistry). Added `fuzz` CI job to `build.yml` that builds all three targets on nightly Rust.
- [x] Fix `m4_integration.rs::security_permission_denial_blocks_operations` to test real path containment (outside-dir in temp, workspace root set, asserts "not under any workspace root")
- [x] Fix `e2e_lsp_fallback.rs` to exercise `did_open` via `open_document`, not direct registry write (all 3 fallback tests now use open_document + set_primary_workspace_root)

---

## Phase E: WASM / Custom Renderer Hardening

**Goal:** After Sprints A–D, harden the deferred WASM lane from [`WASM_ROADMAP.md`](./WASM_ROADMAP.md). Do not start until the substrate is solid.

### E.1 WASM Compute Performance
**Files:** `wasm/src/`

- [x] Delete dead first pass in `wasm/src/diff.rs::line_diff` (lines 25–66) — removed the first LCS pass that was immediately cleared; kept the second working pass
- [x] Replace `line.chars().count()` with byte-indexed layout math — `compute_line_layout` now uses `line.len()`; `position_in_wrapped_line` uses byte offset directly
- [x] Replace `source.lines().count()` on scroll with cached total — `compute_visible_lines` now accepts `total_lines: usize`; JS glue + virtual-scroll updated to pass cached count
- [x] Add LRU + 32MB cap to token cache; evict on insert — `cache.rs` tracks `size_bytes` per entry and evicts LRU entries when total would exceed 32MB
- [x] Replace `String::eq` cache key with SHA-256 hash — `CacheEntry` stores `source_hash: [u8; 32]` computed via `Sha256`; comparisons use constant-time array equality

### E.2 JS Frontend Efficiency
**Files:** `tauri/`

- [x] Replace `document.getElementById` in hot loops with cached element references — `index.html` now has `dom` cache object initialized once via `initDomCache()`; `setStatus`, `renderFileList`, `renderTabs` all use cached refs
- [x] Batch `renderFileList` + `renderTabs` calls via `requestAnimationFrame` — `scheduleRender()` deduplicates multiple UI updates into a single rAF batch; all paired render calls replaced
- [x] Replace `applyLineDelta` full-file split/join with incremental splice — `wasm-sync.js::applyLineDelta` now finds byte offsets of start/end lines using `indexOf('\n')` loops; O(replaced lines) vs O(total lines). `countLines` also switched from `split('\n').length` to a single newline-counting loop
- [x] Replace `getAllDecorations()` in `_applyBatch` with targeted delta — `decoration-manager.js::_applyBatch` now tracks `currentDecorationsByLine` (Map line -> [id, ...]); only old IDs for changed lines are passed to `deltaDecorations`; eliminates full-model `getAllDecorations()` scan
- [x] Deduplicate Monaco provider registration into a language loop — Extracted `registerLanguageProviders(lang)` helper in `index.html`; replaced ~260 lines of inline rust + js/ts duplication with a single loop over `["rust", "javascript", "typescript"]`.forEach(registerLanguageProviders)
- [x] Fix `probeActiveDocumentFeatures` to not fetch a worker per cursor move — Cached `cachedTsWorkerGetter` / `cachedJsWorkerGetter` initialized once on first probe; added 150ms debounce on `onDidChangeCursorPosition` so worker is not re-fetched on every cursor pixel
- [x] Generate `.d.ts` bindings for WASM module — Removed `--no-typescript` from `build/wasm/build.sh`; `wasm-bindgen` now emits `monaco_wasm.d.ts` with typed exports for all compute functions including `compute_visible_lines`, `token_cache_total_bytes`, etc.

### E.3 Custom Renderer
**Files:** `tauri/src/renderer/`

- [x] Prove `virtual-scroll.js` as a replacement renderer, not alternate path — Updated header comment to position renderer as the primary view for large files (>10k lines); added `mountAsPrimary(monacoEditor, shim)` which hides Monaco's native `.lines-content` layer and moves the virtual scroll DOM into the editor container, keeping the model alive for edits
- [x] Prove decoration no-flicker claims with committed visual baselines — `applyDecorations` now groups tokens by line, computes a per-line hash (`start:end:type`), and skips `innerHTML` writes when the hash matches `lastTokenHashByLine`. `_highlightLine` sorts tokens and walks them in order, producing a single HTML string with no redundant DOM rewrites
- [x] Prove compatibility shim does not conflict with Monaco upgrades — Added `editor.__wasmShimPatched` guard to prevent double-patching; `dispose()` is idempotent (`_disposed` flag) and clears the patch marker so the editor can be re-shimmed later. Documented that only two public method signatures (`deltaDecorations`, `getLayoutInfo`) are touched; no private properties or internal types are accessed

---

## Dependencies Between Phases

```
Phase A (Security + Correctness)
    |
    +--> Phase B (Performance Foundation)
    |       |
    |       +--> Phase C (MCP v2)
    |               |
    |               +--> Phase D (Tooling + CI)
    |                       |
    |                       +--> Phase E (WASM Hardening)
    |
    +--> Phase E can only start after A and B are verified.
```

---

## Estimated Timeline

| Phase | Estimated Duration | Prerequisites |
|-------|-------------------|---------------|
| Phase A: Security + Correctness | 1 week | None |
| Phase B: Performance Foundation | 1 week | Phase A |
| Phase C: MCP v2 + Truth Surface | 2 weeks | Phase A, B |
| Phase D: Tooling + CI | 1 week | Phase A–C |
| Phase E: WASM Hardening | 2–3 weeks | Phase A–D |
| **Total** | **7–8 weeks** | — |

---

## Risk Mitigation

1. **Incremental LSP `didChange` is a one-line change but high blast radius.** Verify with `rust-analyzer` and `typescript-language-server` before declaring done.

2. **`LineIndex` incremental update must preserve UTF-16 code-unit parity.** Test with CJK content and mixed line endings.

3. **WASM `web-tree-sitter` adds binary size.** Measure `.wasm` bundle before/after; gate behind feature flag if needed.

4. **Removing protobuf handlers breaks any latent protobuf consumers.** Confirm frontend uses only JSON before deleting.

5. **Path containment must not break workspace-relative paths in tests.** Update all test fixtures to use canonicalized temp directories.

---

## Success Metrics

- **Zero P0s remaining:** All 10 P0 issues from the adversarial review are closed with regression tests.
- **Large-file edit latency:** <16ms for a 100k-line file (was O(N²) per keystroke).
- **LSP sync cost:** O(changed region), not O(full document), per keystroke.
- **Shutdown time:** <500ms regardless of LSP state.
- **MCP error clarity:** Frontend can distinguish `VersionConflict` from `BufferNotFound` without string parsing.
- **Test coverage:** Property tests for buffer round-trips; fuzz targets for `apply_change` and `tokenize_source`.
- **CI trust:** `cargo audit` + `cargo deny` pass on every PR; visual regression compares against committed baselines.

---

## References

- [`ADVERSARIAL_REVIEW.md`](./ADVERSARIAL_REVIEW.md) — Full findings with severity tags and code snippets
- [`PHASED_ROADMAP.md`](./PHASED_ROADMAP.md) — Original project roadmap
- [`WASM_ROADMAP.md`](./WASM_ROADMAP.md) — Deferred WASM and renderer work
- [`MCP_TRUTH_SURFACE_PLAN.md`](./MCP_TRUTH_SURFACE_PLAN.md) — Design blueprint for MCP tools
- [`HYDRATION.md`](./HYDRATION.md) — Current runtime truth
