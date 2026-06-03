# Adversarial Remediation Roadmap

> **Purpose:** Turn the findings in [`ADVERSARIAL_REVIEW.md`](./ADVERSARIAL_REVIEW.md) into a sequenced, observable plan. Each phase maps to a sprint in the review's "Path to v1" (§7.3).
> **Status:** Not started.
> **Last updated:** 2026-06-03

---

## Context

The adversarial review (generated 2026-06-03 against `57cfbb6f`) found **10 P0 issues** (exploitable / corrupting / blocking), **37 P1 issues** (material correctness or performance), and **47+ P2 issues** (quality / ergonomics). The Rust substrate is real and well-organized (167 verified tests), but the runtime has security holes, an O(N²) per-keystroke cost center, a full-document LSP `didChange` on every edit, and prototype-grade WASM/JS.

This roadmap treats the review as the spec. Every item below references the original finding by section.

---

## Reality Check

| Area | Current State | Target |
|------|---------------|--------|
| Path containment | None; `uri_to_path` is `PathBuf::from` | Canonicalized workspace-root check |
| Optimistic edit race | TOCTOU read-then-write | Single write-lock atomic check |
| LSP shutdown | `Drop` blocks 5s on dead server | Non-blocking / timeout |
| LSP `didChange` | Full document replacement every keystroke | Incremental range per spec |
| `save_document_as` | No `did_close`/`did_open` | LSP state stays synchronized |
| Symlink handling | Follows symlinks on write | Reject symlinks in write path |
| Remote edit guard | Single global boolean | Per-buffer `Set<String>` |
| Dual IPC encoding | 36 handlers (18 logical × JSON + protobuf) | One encoding, one handler set |
| `LineIndex` | Rebuilt O(N²) on every edit | Incremental update O(Δ) |
| WASM tokenizer | Hand-rolled heuristic | `web-tree-sitter` parity with native |
| Event subscriptions | `EventSubscriptionTracker` dead code | Wired or removed |
| MCP tool registry | Rebuilt on every call | Cached in `MonacoHostState` |

---

## Phase A: Security + Correctness

**Goal:** Close the 10 P0 issues before any other work. This is the sprint the review calls "about 1 week."

### A.1 Path Containment
**Files:** `src-tauri/src/host_handlers.rs`, `src-tauri/src/security/capabilities.rs`

- [ ] Implement `validate_path_under_workspace` (canonicalize, workspace-root prefix check, symlink rejection)
- [ ] Apply validation to `open_document`, `save_document`, `save_document_as`, `list_directory`
- [ ] Apply validation to MCP tools (`read_file`, `edit_file`, `list_directory`)
- [ ] Add `PathOutOfWorkspace` variant to error types

**Success Criteria:**
- `../../etc/passwd` is rejected from every surface
- Symlink to `/etc/passwd` under workspace is rejected on write
- Existing tests updated to exercise real path containment

### A.2 Atomic Optimistic Edit
**File:** `src-tauri/src/buffer/registry.rs`

- [ ] Rewrite `apply_edit_optimistic` to acquire `write()` once, read version, compare, and commit or fail without releasing

**Success Criteria:**
- Concurrent MCP `edit_file` calls with `expected_version` cannot race

### A.3 Non-blocking LSP Lifecycle
**File:** `src-tauri/src/lsp/client.rs`

- [ ] Replace `Drop::drop` synchronous shutdown with fire-and-forget or bounded timeout (`try_shutdown`)
- [ ] Ensure reader thread is joined or detached cleanly

**Success Criteria:**
- Editor shutdown does not hang when LSP process is dead
- No thread leaks on repeated open/close cycles

### A.4 LSP State Synchronization on Rename
**File:** `src-tauri/src/host_handlers.rs`

- [ ] In `save_document_as`, call `lsp.did_close` for source URI before moving buffer
- [ ] Call `lsp.did_open` for target URI after move

**Success Criteria:**
- Renaming a file does not leave LSP with stale URI → buffer mapping

### A.5 Symlink Rejection
**File:** `src-tauri/src/host_handlers.rs`

- [ ] Use `symlink_metadata` before `fs::write` in `save_document` and `save_document_as`
- [ ] Return structured error if target is a symlink

### A.6 Per-Buffer Remote Edit Guard
**File:** `tauri/index.html`

- [ ] Replace `state.isApplyingRemoteEdits: boolean` with `state.remoteEditPaths: Set<string>`
- [ ] Guard `invoke("apply_edits_json")` per buffer path

**Success Criteria:**
- Concurrent remote edits on different buffers do not interfere
- Edit loops from async `applyEdits` are eliminated

### A.7 Event Subscription Tracker — Wire or Remove
**File:** `src-tauri/src/events.rs`

- [ ] Either integrate `EventSubscriptionTracker` into the Tauri `emit` path, or delete it and its tests

**Success Criteria:**
- No dead code that is tested but unused

### A.8 Version Bump on Save
**File:** `src-tauri/src/host_handlers.rs`

- [ ] Ensure `save_document` increments `version_id` so frontend `state.currentVersionId` stays synchronized

---

## Phase B: Performance Foundation

**Goal:** Fix the single most expensive P0 (full-document `didChange`) and the dominant P1 (O(N²) `LineIndex`). This is the sprint the review calls "about 1 week."

### B.1 Incremental LSP `didChange`
**File:** `src-tauri/src/host_handlers.rs`

- [ ] Replace `range: null` with actual `start/end` range + `text` in the `didChange` payload
- [ ] Use `lsp_types::TextDocumentContentChangeEvent` with populated `range`

**Impact:** `rust-analyzer` re-parse goes from O(file) to O(changed region) per keystroke.

**Success Criteria:**
- LSP server receives incremental changes, not full replacement
- Typing latency on large files drops measurably

### B.2 Incremental `LineIndex`
**File:** `src-tauri/src/buffer/text_buffer.rs`, `src-tauri/src/buffer/line_index.rs`

- [ ] Remove `LineIndex::new` full rebuild from `apply_change` and `set_value`
- [ ] Implement `LineIndex::update_incremental` using rope's built-in line table (`line_to_char`, `char_to_line`)
- [ ] Update only the dirty range plus one boundary line

**Impact:** 100k-line file: from ~5×10⁹ UTF-16 conversions per keystroke to ~1k.

**Success Criteria:**
- Large file (>50k lines) edit latency <16ms

### B.3 Buffer Snapshot via `Arc<[u8]>`
**File:** `src-tauri/src/buffer/text_buffer.rs`

- [ ] Change `BufferSnapshot.content_utf8` from `Vec<u8>` to `Arc<Vec<u8>>`
- [ ] Add `get_snapshot_arc` that clones the Arc instead of the bytes

**Impact:** 100MB file metadata fetch: from ~300MB memory traffic to 100MB.

### B.4 Debounce Diagnostics
**File:** `src-tauri/src/main.rs`, `src-tauri/src/syntax_handlers.rs`

- [ ] Move `diagnostics_document` out of the synchronous `apply_edits_json` handler
- [ ] Spawn a tokio task with `tokio::time::sleep(Duration::from_millis(150))`
- [ ] Cancel in-flight diagnostic task on new edit

**Success Criteria:**
- Keystroke handler does not block on Tree-sitter parse + LSP round-trip

### B.5 Cache `SyntaxParser` per Language
**File:** `src-tauri/src/syntax_handlers.rs`

- [ ] Add `SyntaxParserCache: Mutex<HashMap<String, Arc<Mutex<SyntaxParser>>>>`
- [ ] Reuse parser instead of creating a fresh one per call

### B.6 Async LSP Request with Cancellation
**File:** `src-tauri/src/lsp/client.rs`

- [ ] Replace synchronous `mpsc::channel` per call with `tokio::sync::oneshot`
- [ ] Add `request_async` returning a future; store pending request IDs
- [ ] Cancel superseded `didChange` requests on newer edit
- [ ] Reduce timeout from 5s to 2s

### B.7 WASM Tokenizer Parity
**File:** `wasm/src/tokenize.rs`

- [ ] Replace hand-rolled heuristic with `web-tree-sitter` using compiled grammar WASM modules
- [ ] Ensure native Rust and WASM paths produce identical token streams

### B.8 Collapse Dual-Encoding IPC
**File:** `src-tauri/src/main.rs`

- [ ] Delete either the JSON or protobuf parallel handler set
- [ ] If keeping protobuf, migrate frontend `invoke` calls to binary path
- [ ] If keeping JSON, remove protobuf command surface

### B.9 `parking_lot` RwLock Migration
**Files:** `src-tauri/src/` (all modules using `std::sync::RwLock`)

- [ ] Replace `std::sync::RwLock` with `parking_lot::RwLock`
- [ ] Eliminate lock-poison panic surface

---

## Phase C: MCP v2 + Truth Surface

**Goal:** Implement the new MCP tools and structured errors described in [`MCP_TRUTH_SURFACE_PLAN.md`](./MCP_TRUTH_SURFACE_PLAN.md). About 2 weeks.

### C.1 Structured Error Envelope
**Files:** `proto/ipc_envelope.proto`, `src-tauri/src/mcp/tools.rs`

- [ ] Add `McpError { code, message, kind }` proto message
- [ ] Distinguish `BufferNotFound`, `VersionConflict`, `InvalidEdit`, `PermissionDenied`, `PathOutOfWorkspace`
- [ ] Add `McpEnvelope.status: "ok" | "partial" | "unavailable"` alongside `success: bool`

### C.2 New MCP Tools
**File:** `src-tauri/src/mcp/tools.rs`

- [ ] `compare_buffer_versions` — exact diff between two `version_id`s
- [ ] `subscribe_buffer_events` + `poll_buffer_events` — wire `EventSubscriptionTracker`
- [ ] `query_symbols` — queryable symbol surface backed by parse cache
- [ ] `preview_structural_edit` + `structural_edit` — gated exact-symbol mutation
- [ ] `inspect_buffer_drift` — buffer vs saved file, symbol/diagnostic delta
- [ ] `inspect_contradictions` — cross-surface disagreement detection
- [ ] `watch_workspace` — fs-change subscription with `watcher_id` + poll cursor
- [ ] `replay_buffer` — content at historical `version_id` (requires lineage store)
- [ ] `diff_files` — cross-file semantic diff via shared parse tree
- [ ] `search_text_in_buffers` — workspace-wide content search with SHA-256 per match
- [ ] `list_open_buffers` + `close_buffers_batch` — registry introspection
- [ ] `get_buffer_byte_range` — O(1) partial read via `Arc<Vec<u8>>` snapshot

### C.3 MCP Tool Registry Caching
**File:** `src-tauri/src/main.rs`, `src-tauri/src/mcp/tools.rs`

- [ ] Move `McpToolRegistry` into `MonacoHostState`, constructed once
- [ ] Remove `with_defaults()` rebuild on every `execute_mcp_tool` call

### C.4 Protobuf Payload Discriminator
**File:** `proto/ipc_envelope.proto`

- [ ] Add `PayloadType` enum to `IpcRequest`
- [ ] Add `PayloadType` enum to `IpcEvent`

### C.5 Version Lineage Store
**File:** `src-tauri/src/buffer/text_buffer.rs`

- [ ] Store `BufferSnapshot` history (bounded, e.g. last 50 versions)
- [ ] Enable `replay_buffer` and `compare_buffer_versions` tools

---

## Phase D: Tooling, CI, and Test Hardening

**Goal:** Add `cargo audit`, property tests, fuzz targets, fix broken CI actions, and reconcile test coverage. About 1 week.

### D.1 CI Hardening
**Files:** `.github/workflows/build.yml`, `.github/workflows/release.yml`, `.github/workflows/nightly.yml`

- [ ] Add `cargo audit` + `cargo deny` steps
- [ ] Pin GitHub Actions by SHA (supply-chain hardening)
- [ ] Replace deprecated `actions/create-release@v1` and `actions/upload-release-asset@v1` with `softprops/action-gh-release@v2` or `gh-cli`
- [ ] Run nightlies on Linux, macOS, and Windows (not just `ubuntu-latest`)
- [ ] Consolidate `visual-regression.yml` and `build.yml::visual-regression` into one workflow with `tauri-driver`

### D.2 Visual Regression
**Files:** `tests/visual/`, `.github/workflows/`

- [ ] Commit baseline PNGs per platform
- [ ] Fail CI on >1% pixel diff outside known change zones

### D.3 Performance Regression
**Files:** `.github/workflows/perf-regression.yml`, `src-tauri/tests/m7_buffer_bench.rs`

- [ ] Fix `bench_small_file_insert`: separate construction cost from edit cost
- [ ] Store version-pinned baseline artifacts
- [ ] Fail CI on >20% regression against pinned baseline

### D.4 Property and Fuzz Tests
**Files:** `src-tauri/tests/`, `wasm/tests/`

- [ ] Add `proptest` for `TextBuffer::apply_change` + `LineIndex` round-trip
- [ ] Add `cargo-fuzz` targets: `apply_change`, `compute_line_delta`, `tokenize_source`
- [ ] Fix `m4_integration.rs::security_permission_denial_blocks_operations` to test real path containment
- [ ] Fix `e2e_lsp_fallback.rs` to exercise `did_open` via `open_document`, not direct registry write

---

## Phase E: WASM / Custom Renderer Hardening

**Goal:** After Sprints A–D, harden the deferred WASM lane from [`WASM_ROADMAP.md`](./WASM_ROADMAP.md). Do not start until the substrate is solid.

### E.1 WASM Compute Performance
**Files:** `wasm/src/`

- [ ] Delete dead first pass in `wasm/src/diff.rs::line_diff` (lines 25–66)
- [ ] Replace `line.chars().count()` with byte-indexed layout math
- [ ] Replace `source.lines().count()` on scroll with cached total
- [ ] Add LRU + 32MB cap to token cache; evict on insert
- [ ] Replace `String::eq` cache key with SHA-256 hash

### E.2 JS Frontend Efficiency
**Files:** `tauri/`

- [ ] Replace `applyLineDelta` full-file split/join with incremental splice
- [ ] Replace `getAllDecorations()` in `_applyBatch` with targeted delta
- [ ] Deduplicate Monaco provider registration into a language loop
- [ ] Fix `probeActiveDocumentFeatures` to not fetch a worker per cursor move
- [ ] Generate `.d.ts` bindings for WASM module (remove `--no-typescript` from build script)

### E.3 Custom Renderer
**Files:** `tauri/src/renderer/`

- [ ] Prove `virtual-scroll.js` as a replacement renderer, not alternate path
- [ ] Prove decoration no-flicker claims with committed visual baselines
- [ ] Prove compatibility shim does not conflict with Monaco upgrades

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
