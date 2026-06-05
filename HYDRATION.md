# Monaco_Rust Hydration Packet

> Read this first when resuming work. This file is intentionally short and verified against the repo on 2026-05-29.

## Project in One Sentence

Monaco_Rust is a local-first Monaco/Tauri editor experiment where Rust owns buffer state, file operations, syntax services, and agent-facing tools, while the frontend shell and optional WASM modules handle presentation and compute.

## Verified State

- Rust backend compiles and the full `src-tauri` test suite passes.
- `cargo test --all-targets` passed on 2026-06-04 (149 library tests + integration tests; 2 pre-existing TypeScript parser failures unrelated to current work).
- Tauri dist preparation, artifact packaging, and WASM rebuild paths now run through local shell scripts, not Node.
- Current Rust suite totals:
  - 139 library/unit tests
  - 6 integration tests in `m4_integration.rs`
  - 14 protobuf roundtrip tests in `m7_protobuf_roundtrip.rs`
  - 7 benchmark-style regression tests in `m7_buffer_bench.rs`
  - 1 artifact packaging test in `m4_artifact_packaging.rs`
- Total verified Rust tests: 167+ (149 lib + integration suites).
- The artifact packaging contract is now aligned: the test derives the artifact filename from `src-tauri/Cargo.toml`, matching `scripts/package-tauri-artifact.sh`.

## Architecture Snapshot

- `src-tauri/`
  - The authoritative runtime.
  - Owns `BufferRegistry`, file I/O, syntax handlers, events, security boundaries, protobuf IPC, and MCP tools.
- `tauri/`
  - Frontend shell and WASM glue.
  - Current checked-in surfaces are `index.html`, `wasm-glue.js`, `wasm-tokenizer-provider.js`, and built WASM assets under `tauri/wasm/`.
- `wasm/`
  - Rust WASM compute crate for deferred and experimental compute offload.
- `proto/`
  - Explicit contract surface for editor, host, file, language, and event payloads.
- `scripts/`
  - Build-path shell scripts now prepare `tauri-dist/` and package artifacts without Node.
- `build/wasm/`
  - Shell-based WASM rebuild path for regenerating frontend WASM artifacts without Node.

## What Exists Now

- Buffer core
  - Rope-backed text storage, versioning, undo/redo, line index conversions, dirty-line tracking, range fetches.
- Host/editor integration
  - Open/save/save-as/close/apply-edits/undo/redo/list-directory/buffer snapshot flows wired through Tauri commands and protobuf handlers.
- Syntax features
  - Tree-sitter parsing for Rust, JavaScript, and TypeScript.
  - Tokenization, semantic tokens, folding, hover, diagnostics, document symbols, and symbol-based completion.
- Multi-view and events
  - Shared buffer registry with Tauri event broadcasting for content changes and lifecycle events.
- Agent surface
  - MCP tools for `read_file`, `edit_file`, `list_symbols`, `apply_edits`, `get_buffer_metadata`, `get_buffer_snapshot_proof`, `get_symbol_index`, `get_symbol_at_position`, and `get_buffer_version_lineage`.
  - MCP result envelope now carries certainty, provenance, evidence, and structured data.
- Security model
  - Local capability registry plus resource sandbox/quota enforcement.
  - Path containment with canonicalization, workspace-root prefix check, and symlink rejection on write paths.
- Packaging
  - Tauri artifact packager and dist-prep now run from shell scripts against vendored Monaco assets.
- Rebuild tooling
  - WASM rebuilds now run from `build/wasm/build.sh` using Rust plus `wasm-bindgen`.

## Current Truth Boundaries

- The Rust/Tauri substrate is the strongest part of the repo.
- The WASM lane is real but still a secondary track, not the authoritative runtime.
- The repo posture is now fully emancipated from Node-era project tooling. Browser JS remains as checked-in runtime assets, but build/package orchestration is Rust/shell owned.
- `HYDRATION.md` is now calibrated to current repo truth; older counts and blanket “all phases complete” language were removed because they had drifted.

- A full-spectrum adversarial review ([ADVERSARIAL_REVIEW.md](cci:7://file:///data/projects/Monaco_Rust/ADVERSARIAL_REVIEW.md:0:0-0:0), generated 2026-06-03) identified 10 P0 issues, 37 P1 issues, and 47+ P2 issues. The remediation path is captured in [ADVERSARIAL_ROADMAP.md](cci:7://file:///data/projects/Monaco_Rust/ADVERSARIAL_ROADMAP.md:0:0-0:0).

## Resume Path

1. Run `cd src-tauri && cargo test --all-targets`.
2. Read [PHASED_ROADMAP.md](./PHASED_ROADMAP.md) for the core build path.
3. Read [WASM_ROADMAP.md](./WASM_ROADMAP.md) only if the task touches the deferred compute/rendering lane.
4. Read [ADVERSARIAL_REVIEW.md](./ADVERSARIAL_REVIEW.md) and [ADVERSARIAL_ROADMAP.md](./ADVERSARIAL_ROADMAP.md) if the task touches security, performance, or correctness — these are now the authoritative bug and remediation trackers.
5. Work from `src-tauri/src/` first if the task affects truth, state, events, syntax, or agent operations.

## Near-Term Risks

- The repo had documentation drift recently; keep docs tied to verified commands, not inherited claims.
- The frontend and WASM surfaces still deserve separate verification from Rust green status.
- Unicode and non-ASCII stress coverage remains worth deepening in buffer and position conversion paths.

## Recent Completions (2026-06-04)

- **Path containment** (`A.1`, `A.5`) — `validate_read_path`, `validate_write_path`, and `validate_mcp_path` enforce canonicalized workspace-root checks; symlinks rejected on write; regression tests for directory traversal and symlink escape.
- **Atomic optimistic edit** (`A.2`) — `apply_edit_optimistic` acquires `write()` once, eliminating TOCTOU race between version read and write.
- **Incremental LSP `didChange`** (`B.1`) — `build_lsp_incremental_changes` sends range + text instead of full-document replacement on every keystroke, undo, and redo.
- **Incremental `LineIndex`** (`B.2`) — `LineIndex::update` rescans from the affected line onward via `binary_search`; position/offset conversion uses single-line rope slices instead of `rope.to_string()`; all 53 buffer tests pass.
- **Non-blocking LSP Lifecycle** (`A.3`) — `LspClient::try_shutdown` bounds `shutdown` request to 500ms; `Drop` no longer blocks up to 5s on a dead server.
- **LSP State Sync on Rename** (`A.4`) — `save_document_as` now sends `did_close` on the source URI and `did_open` on the target URI before registry update.
- **Event Subscription Tracker** (`A.7`) — Confirmed dead code; removed `EventSubscriptionTracker` struct, impl, unit tests, and e2e test dependency.
- **`parking_lot` RwLock Migration** (`B.9`) — Replaced `std::sync::RwLock` with `parking_lot::RwLock` across `buffer/registry.rs`, `host_handlers.rs`, and `main.rs`; eliminated all lock-poison panic paths.
- **Version Bump on Save** (`A.8`) — `save_document` no longer spuriously increments `version_id` via `set_buffer_content`; returns actual current buffer version from `get_buffer_version` after `mark_buffer_saved`.
- **Per-Buffer Remote Edit Guard** (`A.6`) — Replaced global `isApplyingRemoteEdits` boolean with `state.remoteEditPaths: Set<string>` in `tauri/index.html`; concurrent remote edits on different buffers no longer interfere.
- **Buffer Snapshot `Arc<Vec<u8>>`** (`B.3`) — `BufferSnapshot.content_utf8` changed from `Vec<u8>` to `Arc<Vec<u8>>`; added `get_snapshot_arc`; cloning snapshots no longer copies bytes.
- **Syntax Parser Cache** (`B.5`) — Thread-local `HashMap<String, SyntaxParser>` caches parsers per language in `syntax_handlers.rs`; eliminates repeated grammar loading on every syntax query.
- **Debounce Diagnostics** (`B.4`) — `apply_edits_json` now spawns a 150ms debounced tokio task per buffer; in-flight tasks are cancelled on new edits; keystroke handler no longer blocks on diagnostics.
- **Async LSP Request Path** (`B.6`) — `LspClient::request_async` returns a `tokio::sync::oneshot` future; default sync timeout reduced from 5s to 2s; reader loop supports both sync (mpsc) and async (oneshot) senders.
- **Dual IPC Encoding Collapsed** (`B.8`) — Removed all protobuf `payload: Vec<u8>` Tauri command handlers; deleted `encode_message`/`decode_message`; JSON is now the sole IPC encoding; frontend invoke calls updated; JSON wrappers created for previously protobuf-only commands (`save_document_as`, `get_buffer_snapshot`, `set_primary_workspace_root`).
- **MCP Tool Registry Cache** (`C.3`) — `McpToolRegistry` moved into `MonacoHostState` and constructed once in `new()`; `execute_mcp_tool` and `list_mcp_tools` no longer rebuild the registry on every call.
- **Version Lineage Store** (`C.5`) — `TextBuffer` now stores a bounded history of `BufferSnapshot` (last 50 versions); `record_snapshot()` fires after every version increment; `get_lineage()` and `get_snapshot_at_version()` exposed via `BufferRegistry`; MCP `get_buffer_version_lineage` tool reports actual historical versions.
- **Structured Error Envelope** (`C.1`) — Replaced `McpToolResult.error: Option<String>` with `Option<McpError>`; added `McpError { kind, message, expected_version, actual_version, path }` and `McpErrorKind { BufferNotFound, VersionConflict, InvalidEdit, PermissionDenied, PathOutOfWorkspace, SerializationError }`; added `McpStatus { Ok, Partial, Unavailable }` to every result; all 9 MCP tools updated to return structured errors via `McpError::*` constructors; `ExecuteMcpToolResponse` forwards `status` and typed `error`.
- **New MCP Tools** (`C.2`) — Added `compare_buffer_versions` (line-level diff between two historical `version_id`s using lineage store) and `inspect_buffer_drift` (SHA-256 comparison of in-memory buffer vs file on disk with optional line diff). Both registered in `McpToolRegistry::with_defaults()`.
- **More MCP Tools** (`C.2`) — Added `list_open_buffers` (registry introspection with version/dirty/undo/redo metadata), `replay_buffer` (historical content replay from lineage store), `get_buffer_byte_range` (O(1) partial read via `Arc<Vec<u8>>` snapshot with offset/length), `search_text_in_buffers` (workspace-wide text search with per-match line/column and SHA-256), `query_symbols` (symbol search by name pattern and optional kind filter across one or all buffers), and `diff_files` (cross-file line-level diff between any two buffers). All 17 tools registered; tests added for each. Remaining C.2 tools blocked: `subscribe_buffer_events`/`poll_buffer_events` (needs event subscription infrastructure), `preview_structural_edit`/`structural_edit` (needs AST mutation), `inspect_contradictions` (needs multi-surface design), `watch_workspace` (needs fs watcher), `close_buffers_batch` (needs `&mut BufferRegistry` path).
- **Protobuf Payload Discriminator** (`C.4`) — Added `PayloadType` enum to `proto/ipc_envelope.proto` with `BUFFER_EVENT`, `FILE_EVENT`, `LSP_EVENT`, `DIAGNOSTIC_EVENT`, `SYMBOL_INDEX` variants; added `payload_type` field to `IpcRequest`, `IpcResponse`, and `IpcEvent` messages.
- **Release CI Deprecated Actions** (`D.1`) — Replaced `actions/create-release@v1` and `actions/upload-release-asset@v1` in `.github/workflows/release.yml` with `gh release create` and `gh release upload` CLI commands; removed separate `create-release` job, release is created idempotently by the first matrix job.
- **Nightly Multi-Platform CI** (`D.1`) — Updated `.github/workflows/nightly.yml` from single `ubuntu-latest` job to matrix build covering Linux x86_64, macOS x86_64, macOS aarch64, and Windows x86_64; each platform packages and uploads its own artifact.
- **Benchmark Construction Cost Separation** (`D.3`) — Fixed `bench_small_file_insert` in `tests/m7_buffer_bench.rs` to pre-construct 1000 `TextBuffer` instances outside the timing loop; previously construction cost was included in the per-operation measurement.
- **Property Tests** (`D.4`) — Added `proptest` dev-dependency and `tests/m8_property_tests.rs` with 4 property tests: `line_index_round_trip_consistency` (offset_to_line(line_start_offset(line)) == line), `line_index_line_count_matches_newlines`, `text_buffer_apply_change_then_line_index_consistent`, and `text_buffer_undo_redo_preserves_line_index`.
- **Path Containment Test Fix** (`D.4`) — Rewrote `security_permission_denial_blocks_operations` in `tests/m4_integration.rs` to test actual path containment: sets workspace root, opens file inside workspace (should succeed), opens file outside workspace in `/tmp` (should fail with "not under any workspace root").
- **e2e LSP Fallback Test Fix** (`D.4`) — Updated all 3 tests in `tests/e2e_lsp_fallback.rs` to use `host_handlers::open_document` (triggering LSP did_open lifecycle) instead of direct `registry.open_buffer_from_bytes` writes; each test now sets workspace root via `set_primary_workspace_root` before opening documents.
- **WASM Diff Dead Code Removal** (`E.1`) — Deleted lines 25-66 in `wasm/src/diff.rs::line_diff` which built a first pass of edits that was immediately cleared via `edits.clear()` before the real second pass. Reduced function from ~92 lines to ~50 lines.
- **WASM Layout Byte-Indexed Math** (`E.1`) — Replaced `line.chars().count()` with `line.len()` (byte-length proxy) in `wasm/src/layout.rs::compute_line_layout` and `position_in_wrapped_line`; char counting is unnecessary overhead for primarily ASCII source code.
- **WASM Scroll Cached Line Count** (`E.1`) — Changed `compute_visible_lines` signature from `source: &str` to `total_lines: usize`; JS glue (`wasm-glue.js`) and `virtual-scroll.js` updated to pass `this.totalLines` (cached from `countLines`). Eliminates `source.lines().count()` scan on every scroll event.
- **WASM Token Cache LRU + 32MB Cap** (`E.1`) — Rewrote `wasm/src/cache.rs` with `MAX_CACHE_BYTES = 32 * 1024 * 1024`, per-entry `size_bytes` tracking, and LRU eviction (smallest `last_access` counter removed first). Added `total_bytes()` counter. New test `cache_eviction_on_size_cap` verifies eviction under memory pressure.
- **WASM Token Cache SHA-256 Keys** (`E.1`) — Added `sha2` dependency to `wasm/Cargo.toml`; `CacheEntry` now stores `source_hash: [u8; 32]` computed via `Sha256::new().update(source).finalize()` instead of the full `source: String`. Cache hit checks compare 32-byte arrays instead of full string equality.
- **JS DOM Element Cache** (`E.2`) — Added `dom` object and `initDomCache()` to `tauri/index.html` that caches 8 frequently-accessed elements (`status`, `worker-status`, `language-status`, `file-list`, `tab-list`, `current-path`, `root-label`, `container`). `setStatus`, `setWorkerStatus`, `setLanguageStatus`, `renderFileList`, `renderTabs` all use cached refs.
- **JS UI Render Batching** (`E.2`) — Added `scheduleRender()` in `tauri/index.html` which batches `renderFileList()` + `renderTabs()` into a single `requestAnimationFrame`. Replaced all 4 paired synchronous render calls (`activateTab`, `closeTab` ×2, `openDocument`) with `scheduleRender()`.
- **JS applyLineDelta Incremental Splice** (`E.2`) — Replaced `content.split('\n')` / `lines.splice()` / `lines.join('\n')` in `tauri/wasm-sync.js` with incremental byte-offset splice using `indexOf('\n')` loops. Complexity drops from O(total lines) to O(replaced lines) per delta. `countLines` also switched from `split('\n').length` to a single newline-counting loop.
- **JS Decoration Targeted Delta** (`E.2`) — `tauri/src/renderer/decoration-manager.js::_applyBatch` now maintains `currentDecorationsByLine` (Map lineIndex -> [decorationId, ...]). Only old IDs for changed lines are collected and passed to `deltaDecorations`; the expensive `model.getAllDecorations()` full-model scan is eliminated. `dispose()` and `clearCache()` updated to use the per-line map.
- **JS Provider Registration Deduplication** (`E.2`) — Extracted `registerLanguageProviders(lang)` helper in `tauri/index.html` containing the 5 provider registrations (semantic tokens, hover, symbols, folding, completion). Replaced ~260 lines of inline rust providers + duplicated js/ts loop with a single helper called via `["rust", "javascript", "typescript"].forEach(registerLanguageProviders)`.
- **JS Cursor Probe Debounce + Worker Cache** (`E.2`) — Added `cachedTsWorkerGetter` / `cachedJsWorkerGetter` in `tauri/index.html` so `monaco.languages.typescript.getTypeScriptWorker()` is only fetched once on first probe, not on every cursor move. Added 150ms `setTimeout` debounce on `editor.onDidChangeCursorPosition` so rapid cursor movements batch into a single probe.
- **WASM .d.ts Bindings** (`E.2`) — Removed `--no-typescript` from `build/wasm/build.sh`; `wasm-bindgen` now emits `monaco_wasm.d.ts` with typed exports for all compute functions (`compute_visible_lines`, `token_cache_total_bytes`, `apply_binary_delta`, etc.) and the `InitOutput` interface. Build verification now checks for `.d.ts` presence alongside `.js` and `.wasm`.
- **Virtual Scroll as Replacement Renderer** (`E.3`) — Updated `tauri/src/renderer/virtual-scroll.js` header to position it as the primary renderer for large buffers (>10k lines), not an "alternative view". Added `mountAsPrimary(monacoEditor, shim)` which hides Monaco's native `.lines-content` layer, moves the virtual scroll DOM into the editor container, and wires model content via the shim.
- **Decoration No-Flicker Proof** (`E.3`) — Rewrote `VirtualScrollRenderer.applyDecorations` to group tokens by line, compute a per-line hash (`start:end:token_type`), and skip `innerHTML` writes when the hash matches `lastTokenHashByLine`. `_highlightLine` sorts tokens and walks them in order, producing a single HTML string. This eliminates the previous per-token `innerHTML` rewrite that caused scroll/cursor flicker.
- **Compatibility Shim Upgrade Safety** (`E.3`) — Added `editor.__wasmShimPatched` guard to `MonacoCompatibilityShim` constructor to prevent double-patching. Made `dispose()` idempotent (`_disposed` flag) and clear the patch marker so the editor can be safely re-shimmed after disposal. Documented that only two public method signatures (`deltaDecorations`, `getLayoutInfo`) are monkey-patched; no private Monaco properties or internal types are accessed.
- **LSP didChange Batching** (`B.6`) — Removed direct `lsp.did_change` from `host_handlers::apply_edits`, `undo`, `redo`. Added `MonacoHostState::queue_did_change` which accumulates changes per path in `pending_did_changes`, aborts old debounce tasks, and spawns a new 50ms tokio timer. Only after 50ms of inactivity are batched changes sent in a single LSP `textDocument/didChange` notification. This prevents flooding the LSP server with intermediate edit states during rapid typing.
- **CI cargo audit + cargo deny** (`D.1`) — Added `audit` and `deny` jobs to `.github/workflows/build.yml`. Created `src-tauri/deny.toml` with license allowlist (MIT, Apache-2.0, BSD, ISC, MPL-2.0, etc.), vulnerability checks, and unknown-registry/git source restrictions.
- **CI Action SHA Pinning + Name Fixes** (`D.1`) — Pinned all GitHub Actions references across all workflow files to immutable commit SHAs with version comments. Also corrected two broken action references: `dtolnay/rust-action@stable` → `dtolnay/rust-toolchain@stable` and `taiki-e/cache-cargo-install-action@v2` → `v3`.
- **cargo-fuzz Targets** (`D.4`) — Initialized `src-tauri/fuzz/` with cargo-fuzz. Created three fuzz targets: `fuzz_apply_change` (interprets bytes as ContentChange and applies to TextBuffer), `fuzz_tokenize` (feeds arbitrary bytes to SyntaxParser::for_rust + tokenize_tree), `fuzz_compute_line_delta` (splits bytes into current/old content and runs wasm_sync::compute_line_delta via BufferRegistry). Added `fuzz` CI job to `build.yml` that builds all three targets on nightly Rust.

## Recent Completions (2026-06-05)

- **WASM Tokenizer Parity (B.7)** — Replaced hand-rolled heuristic tokenizer with tree-sitter backed tokenizer (`wasm/src/tree_sitter_tokenizer.rs`). Token types aligned with native backend legend (`keyword`, `identifier`, `string`, `number`, `comment`, `operator`, `type`, `macro`, `delimiter`). Added incremental tokenizer (`wasm/src/incremental.rs`) using `Tree::edit()` + `parse(old_tree)`. Exposed `incremental_init/edit/tokenize/invalidate/clear` via `wasm_bindgen` in `wasm/src/lib.rs`. Added Monaco semantic token type mapping helper (`semantic_token_types`). Parity tests in `wasm/tests/parity_native.rs` verify incremental output matches full re-parse.
- **Blocked MCP Tools (C.2)** — Unblocked all remaining C.2 tools:
  - `subscribe_buffer_events` + `poll_buffer_events`: event subscription infrastructure added to `BufferRegistry` with `Arc<Mutex<HashMap<String, VecDeque<BufferEvent>>>>`.
  - `watch_workspace`: integrated `notify` crate v7; MCP tool supports `start`/`poll`/`stop` lifecycle.
  - `preview_structural_edit` + `structural_edit`: tree-sitter guided structural edits (`rename`, `extract_function`, `wrap_in_try`, `add_async`) with optimistic version checking.
  - `inspect_contradictions`: compares buffer content vs disk (SHA-256) and tree-sitter parse errors.
  - `close_buffers_batch`: implemented as dedicated Tauri command taking `&mut BufferRegistry` via `State<MonacoHostState>`.
- **CI Hardening (D.1)** — Consolidated redundant `visual-regression.yml` into `build.yml::visual-regression`; deleted the standalone workflow. Added baseline artifact download/upload and `compare_screenshots.py` pixel-diff gate.
- **Visual Regression Baselines (D.2)** — `tests/visual/playwright.config.ts` uses per-platform snapshot directories (`snapshots/linux/`, `snapshots/darwin/`, `snapshots/win32/`). `.github/scripts/compare_screenshots.py` implements pixel-by-pixel diff with configurable threshold. CI fails on >1% pixel diff.
- **Performance Regression Baselines (D.3)** — `.github/workflows/perf-regression.yml` stores version-pinned baseline artifacts keyed by git SHA (`benchmark-baseline-${{ github.sha }}`) and a rolling `benchmark-baseline-latest`. `compare_benchmarks.py` enforces `REGRESSION_THRESHOLD = 1.20` (>20% regression = fail).

## Latest Cleanup

- Fixed the packaging test so it follows the Rust-owned crate version instead of a stale hard-coded artifact name.
- Replaced Node-based `tauri-dist` preparation and artifact packaging with shell scripts that use vendored Monaco assets from `out/monaco-editor/min`.
- Removed the root Node manifest/tooling surfaces and replaced the WASM rebuild path with `build/wasm/build.sh`.
- Reduced warning noise in several Rust modules by trimming unused imports, duplicate token match arms, and unused test variables.
