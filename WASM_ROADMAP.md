# WASM, Rendering, and Cross-Platform CI Roadmap

> **Purpose:** This document captures all deferred items from the main [`PHASED_ROADMAP.md`](./PHASED_ROADMAP.md) that require architectural infrastructure beyond the current Rust/Tauri scope.
> **Status:** Draft — to be refined before implementation begins.
> **Last updated:** 2026-05-29

---

## Context

Phases 1–7 of the Monaco_Rust refactoring are substantially complete (see [`HYDRATION.md`](./HYDRATION.md) for full status). The following capabilities were intentionally deferred because they require WASM compute modules — not a full WASM port of the backend.

### Architecture: Native Rust Backend + WASM Compute Modules (Hybrid)

This is the **JetBrains Fleet** model:

- **Rust backend stays native** (`src-tauri/src/`) — `BufferRegistry`, `TextBuffer`, `host_handlers`, security, events remain in the native Tauri process. Authoritative state lives here.
- **WASM modules handle compute** — tokenization, syntax parsing, diffing, layout math, semantic analysis. These run inside the Monaco webview and operate on buffer snapshots passed from Rust.
- **Monaco talks to WASM for compute** — the editor requests tokens, layout, diffs from WASM modules for near-zero-latency response.
- **Monaco talks to Rust for authoritative state** — open, save, edit, undo, redo, buffer registry operations go through Tauri IPC to the native backend.

**Why this architecture?**
- No duplicate runtimes — the buffer registry is not duplicated in WASM
- WASM modules can be swapped in/out (different language grammars, custom diff algorithms)
- Rust backend remains authoritative — single source of truth for file state
- Monaco gets high-performance compute without blocking the main thread
- Tokenization, layout, diffing happen in the webview thread pool without Tauri IPC roundtrips

This roadmap breaks those deferred items into focused, actionable phases.

---

## Phase W1: WASM Bridge

**Goal:** Enable zero-copy, bidirectional data sharing between the Rust backend and the Monaco frontend via WebAssembly.

### W1.1 WASM Compute Module Infrastructure
**New files:** `wasm/Cargo.toml`, `wasm/src/lib.rs`, `wasm/src/tokenize.rs`, `wasm/src/diff.rs`, `wasm/src/layout.rs`

- [x] Add a `wasm32-unknown-unknown` crate (`wasm/`) with `wasm-bindgen`, `js-sys`, `web-sys`
- [x] Implement `SharedArrayBuffer`-backed text slice for zero-copy JS ↔ WASM passing
- [ ] Pre-allocate linear memory for large files (pre-grow WASM memory to avoid reallocation stalls)
- [ ] Evaluate `memory64` proposal for buffers >4GB
- [x] Build pipeline: integrate `wasm-pack` into the build script (`build/wasm/build.script.ts`)
- [x] Load `.wasm` module in the frontend and expose compute functions via `wasm-bindgen`

**Success Criteria:**
- WASM module loads in the Tauri webview
- Buffer content snapshots pass from JS to WASM without string serialization
- WASM compute functions return results in <1ms for 1k-line files

### W1.2 JS Glue Layer (Monaco ↔ WASM ↔ Rust)
**New files:** `tauri/src/wasm-glue.ts`

- [x] Create `WasmComputeProvider` class that loads the WASM module and exposes:
  - `tokenize(source, language) → SemanticTokens`
  - `diff(oldText, newText) → ContentChange[]`
  - `layout(source, lineWidth, fontMetrics) → LineLayout[]`
- [x] On buffer open: fetch snapshot from Rust via Tauri IPC, pass bytes to WASM
- [ ] On edit: apply change to Rust backend, then pass new snapshot to WASM for re-tokenization
- [ ] Cache last snapshot + tokens in WASM to avoid full re-computation on minor edits

**Success Criteria:**
- Monaco's `setMonarchTokensProvider` can delegate to WASM tokenizer
- Typing latency remains <16ms (WASM re-tokenizes viewport, not full file)
- No duplicate buffer state in WASM — only snapshots for compute

### W1.3 Incremental Sync Protocol (Rust ↔ WASM)
**New files:** `src-tauri/src/wasm_sync.rs`, `wasm/src/sync.rs`

- [x] Encode `ContentChange` deltas as compact binary structs (not JSON) for Rust → WASM transfer
- [x] Implement snapshot diffing: Rust sends only changed lines to WASM, not the full buffer
- [ ] Add heartbeat/sync barrier for agent-driven bulk edits (WASM pauses tokenization until sync complete)

**Success Criteria:**
- 100+ agent edits/second processed without UI blocking
- Sync latency <4ms per batch
- WASM never has stale buffer state

---

## Phase W2: Custom Monaco Renderer (WASM Layout + Native State)

**Goal:** Replace Monaco's default DOM renderer with a WASM-computed, viewport-aware rendering layer. The native Rust backend remains authoritative for buffer state; WASM handles layout math and DOM orchestration.

### W2.1 Layout Computation Offload to WASM
**New files:** `wasm/src/layout.rs`, `tauri/src/renderer/layout.ts`

- [x] Move line height / character width calculations to WASM (not native Rust)
- [x] Implement monospace coordinate math in WASM (byte offset → screen position)
- [x] Compute viewport-visible lines only (leverage `get_value_in_line_range` from Phase 5)
- [x] Batch layout updates for scrolling (single RAF-bound update per frame)

**Success Criteria:**
- Scrolling maintains 60fps on 100k+ line files
- Layout thrashing eliminated
- Layout computation happens in the webview thread pool, not the Tauri backend

### W2.2 Virtual Scrolling DOM Layer
**New files:** `tauri/src/renderer/virtual_scroll.ts`

- [x] Render only visible lines + overscroll buffer (e.g. 50 lines above/below viewport)
- [x] Reuse DOM nodes on scroll (pooling)
- [x] Compute line heights from WASM layout data, not DOM measurement
- [ ] Handle variable-height lines (wrapped text, large fonts)

**Success Criteria:**
- 10k+ line files scroll smoothly without DOM node explosion
- Memory usage <50MB for 100k line file

### W2.3 Decoration and Glyph Rendering
**New files:** `tauri/src/renderer/decoration_manager.ts`

- [x] Batch decoration updates from WASM (syntax highlights, diagnostics, search results)
- [x] Compute minimal DOM mutations (dirty region tracking from Phase 5)
- [x] Offload semantic token classification to WASM compute module
- [ ] Support inline widgets (parameter hints, inlay hints) without full re-layout

### W2.4 WASM Tokenizer Compute Pool
**New files:** `wasm/src/tokenize.rs`, `wasm/src/tokenizer_pool.rs`

- [x] Run tree-sitter tokenization inside WASM (using `web-tree-sitter` or compiled grammars)
- [x] WASM → Tree-sitter → semantic tokens pipeline: JS requests tokens, WASM parses, result returned via `SharedArrayBuffer`
- [x] Prioritize viewport-visible lines first, then background-fill remaining lines
- [ ] Cancel in-flight tokenization jobs on rapid edits

**Success Criteria:**
- Decorations update without flicker
- Large file tokenization does not block the main thread
- Typing never waits on background tokenization

### W2.5 Monaco Compatibility Shim
**New files:** `tauri/src/renderer/compatibility.ts`

- [x] Compatibility layer for `deltaDecorations` (Monaco expects DOM-based decoration IDs; map to WASM-managed decoration handles)
- [x] Compatibility for `IEditorLayoutInfo` (Monaco queries layout geometry; bridge to WASM-computed layout)
- [x] Compatibility for `IContentWidget`, `IContentWidgetPosition` (inline widgets, parameter hints)
- [x] Compatibility for `ICodelens` and `IGlyphMarginWidget` (code lenses, breakpoint indicators)
- [x] Graceful degradation: if a Monaco API is not yet offloaded, proxy through to the default implementation

**Success Criteria:**
- Existing Monaco extensions/plugins work without modification
- No breaking changes to `editor.create()` or `model.deltaDecorations()` APIs

---

## Phase W3: Visual Regression and E2E Testing

**Goal:** Ensure frontend stability across Monaco upgrades and custom renderer changes.

### W3.1 Tauri E2E Test Harness
**New files:** `tests/e2e/`

- [x] Integrate Tauri's WebDriver-compatible testing API (or `tauri-driver`)
- [x] Automate the full app lifecycle: launch → open file → type → save → close
- [ ] Add test for each Tauri command exposed in `main.rs`
- [ ] Verify event broadcasting (`buffer-content-changed`, etc.) reaches the frontend

**Success Criteria:**
- E2E tests run headless in CI
- Regression caught before merge

### W3.2 Visual Regression Testing
**New files:** `tests/visual/`, `.github/workflows/visual-regression.yml`

- [x] Capture screenshots of the editor at fixed states (empty, file open, with errors, with completions)
- [x] Use Playwright or Puppeteer for screenshot comparison
- [ ] Establish baseline images per platform (macOS, Linux, Windows)
- [ ] Fail CI on >1% pixel diff outside known change zones

**Success Criteria:**
- UI changes that affect rendering are flagged automatically
- False positive rate <5%

### W3.3 Performance Regression Testing
**New files:** `tests/perf/`, `.github/workflows/perf-regression.yml`

- [x] Run `m7_buffer_bench.rs` benchmarks in CI on every PR
- [ ] Track timing history and fail on >20% regression
- [ ] Profile WASM boundary crossing overhead separately
- [ ] Add memory profiling (heap usage during large file open)

**Success Criteria:**
- Performance regressions caught in CI before merge
- Historical benchmark data persisted as CI artifacts

---

## Phase W4: Cross-Platform CI and Distribution

**Goal:** Build and test on all supported platforms automatically.

### W4.1 Multi-Platform Build Pipeline
**New files:** `.github/workflows/build.yml`

- [x] Build Tauri app for Linux (x86_64, aarch64), macOS (x86_64, Apple Silicon), Windows (x64)
- [x] Cache `cargo` and `node_modules` for sub-5min builds
- [ ] Run full test suite (unit + integration + e2e) on each platform
- [ ] Produce signed artifacts for nightly releases

**Success Criteria:**
- CI completes in <15 minutes per platform
- All 157+ tests pass on every platform

### W4.2 Release Automation
**New files:** `.github/workflows/release.yml`

- [x] Auto-generate release notes from `CHANGELOG.md`
- [x] Attach platform-specific `.tar.gz`, `.zip` artifacts
- [x] Publish to GitHub Releases with semantic versioning
- [ ] Optionally publish to `crates.io` for the `monaco-tauri` library crate

**Success Criteria:**
- One-click release from tag push
- No manual build steps required

---

## Dependencies Between Phases

```
W1.1 Shared Memory Transport
    |
    +--> W1.2 WASM-Backed Text Model
    |       |
    |       +--> W2.1 Layout Computation Offload
    |               |
    |               +--> W2.2 Virtual Scrolling DOM Layer
    |                       |
    |                       +--> W2.3 Decoration and Glyph Rendering
    |                               |
    |                               +--> W2.4 Rust-Side Tokenizer Worker Pool
    |                                       |
    |                                       +--> W2.5 Monaco Compatibility Shim
    |
    +--> W1.3 Incremental Sync Protocol
            |
            +--> W3.3 Performance Regression Testing

W3.1 Tauri E2E Test Harness
    |
    +--> W3.2 Visual Regression Testing
            |
            +--> W4.1 Multi-Platform Build Pipeline
                    |
                    +--> W4.2 Release Automation
```

---

## Deferred Items from Parent Roadmap

| Parent Phase | Deferred Item | Moved To |
|--------------|---------------|----------|
| Phase 5.1 | `SharedArrayBuffer` zero-copy access | W1.1 |
| Phase 5.2 | Layout offload to Rust | W2.1 |
| Phase 5.2 | Monospace coordinate math natively | W2.1 |
| Phase 5.2 | Batch layout updates for scrolling | W2.2 |
| Phase 5.3 | Calculate minimal DOM updates | W2.3 |
| Phase 5.3 | Support virtual scrolling for large files | W2.2 |
| Phase 6.1 | Audit all WASM boundary crossings | W1.1 |
| Phase 7.3 | Update smoke tests for Tauri | W3.1 |
| Phase 7.3 | Add visual regression testing | W3.2 |
| Phase 7.3 | Cross-platform compatibility tests | W4.1 |

---

## Estimated Timeline

| Phase | Estimated Duration | Prerequisites |
|-------|-------------------|---------------|
| W1: WASM Bridge | 3–4 weeks | Phases 1–7 complete |
| W2: Custom Renderer | 4–6 weeks | W1 complete |
| W3: Visual Regression | 2–3 weeks | W2 complete |
| W4: Cross-Platform CI | 1–2 weeks | W3 complete |
| **Total** | **10–15 weeks** | — |

---

## References

- [`PHASED_ROADMAP.md`](./PHASED_ROADMAP.md) — Main project roadmap
- [`HYDRATION.md`](./HYDRATION.md) — Current project state and session history
- [Monaco Editor Custom Renderer Guide](https://microsoft.github.io/monaco-editor/)
- [Tauri Testing Documentation](https://tauri.app/v1/guides/testing/)
- [WASM Bindgen Guide](https://rustwasm.github.io/docs/wasm-bindgen/)
- [Tauri CI/CD](https://tauri.app/v1/guides/building/cross-platform/)
