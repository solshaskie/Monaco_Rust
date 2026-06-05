# Monaco Rust

A native desktop code editor built on [Monaco Editor](https://microsoft.github.io/monaco-editor/) (VS Code's editor), [Tauri](https://tauri.app/) (Rust native backend), and WebAssembly compute modules.

Runtime posture: this repo now stands on a local Rust/Tauri runtime plus checked-in browser assets. The old Node-based build and packaging chain has been removed from the project.

## Architecture

**Hybrid: Native Rust backend + WASM compute modules inside Monaco webview.**

- **Rust backend** (`src-tauri/`) — authoritative buffer state, file system, LSP lifecycle, MCP tools, JSON IPC
- **Monaco frontend** (`tauri/`) — editor UI, served in Tauri webview
- **WASM compute** (`wasm/`) — tokenization, diffing, layout, LRU-cached tokenization with 32MB cap and SHA-256 keying
- **Incremental sync** (`src-tauri/src/wasm_sync.rs`) — Rust sends only changed lines to WASM; binary delta format available

## Quick Start

```bash
# Prepare vendored frontend assets
bash ./scripts/prepare-tauri-dist.sh

# Run in development mode
cargo tauri dev --config src-tauri/tauri.conf.json

# Build release binary
cargo tauri build --config src-tauri/tauri.conf.json
```

## Project Structure

| Directory | Purpose |
|-----------|---------|
| `src-tauri/` | Rust backend — buffer registry, file I/O, Tauri commands |
| `tauri/` | Frontend — Monaco editor, WASM glue, renderer |
| `tauri-dist/` | Prepared static frontend bundle served by Tauri |
| `wasm/` | WASM compute crate — tokenize, diff, layout (Rust → wasm-bindgen) |
| `proto/` | Protobuf schemas (generate internal Rust types; wire transport is JSON) |
| `build/wasm/` | WASM build automation |
| `scripts/` | Shell-based Tauri packaging scripts |
| `.github/workflows/` | CI and release automation |

## Current State

- **Rust/Tauri runtime is the authoritative substrate** — buffer state, file operations, LSP client lifecycle, syntax services, and MCP tools are real and verified.
- **WASM compute lane hardened** — `wasm/` crate ships tree-sitter backed tokenizer (`tree_sitter_tokenizer.rs`) with native parity, true incremental tokenization via `Tree::edit()` + `parse(old_tree)` (`incremental.rs`), dead-code-free `diff.rs`, byte-indexed `layout.rs`, LRU-cached `cache.rs` (32MB cap, SHA-256 keying), and `.d.ts` bindings via `wasm-bindgen`.
- **Incremental sync path exists end-to-end** — `src-tauri/src/wasm_sync.rs` plus the frontend WASM sync manager prime and refresh Rust-owned snapshot/delta state on open and edit flows. Token consumers respect the sync barrier during burst edits.
- **Custom renderer lane is real but still mixed-maturity** — large models (>=10k lines) now auto-promote into the `virtual-scroll.js` primary renderer path through `compatibility.js`, and viewport token results are applied to both Monaco decorations and the virtual renderer surface. Treat this as a serious proving lane, not yet the same maturity tier as the Rust substrate.
- **Renderer/WASM perf proof surfaces now exist** — the repo now carries repeatable benchmark-style checks for Rust->WASM burst sync deltas plus WASM layout/viewport helpers, and the perf workflow records them alongside the existing Rust substrate benchmarks.
- **Renderer-specific large-buffer proof now exists** — `tests/visual/renderer-large-buffer.spec.ts` remains a repo-owned proof surface for large-model promotion, scroll/update behavior, and clean downshift back to Monaco's native path, though the broader screenshot/Playwright lane still needs transport cleanup before it should be treated as the authoritative live-app harness.
- **Host-owned sparse large-file seam now exists** — `open_large_document`, `read_large_document_viewport`, and `close_large_document` provide a read-only sparse session path where Rust keeps file authority and serves viewport slices without hydrating the normal in-memory editor buffer. The live app now uses this path for oversized files, and `tests/visual/sparse-large-document.webdriver.mjs` is the direct live-app WebDriver proof surface for that seam. This is the first honest step toward gigabyte-file credibility.
- **Security sandbox** — capability-based permissions for file system access with real path containment tests.
- **MCP agent integration** — 22+ Model Context Protocol tools for agent-driven editing, inspection, structural review, event subscription, workspace watching, and contradiction detection.
- **LSP performance hardened** — debounced diagnostics (150ms), batched `didChange` notifications (50ms), and cached `SyntaxParser` per language.
- **CI hardened** — cargo audit + cargo deny, SHA-pinned actions, `dtolnay/rust-toolchain` correction, cargo-fuzz targets on nightly, and performance regression baselines with >20% regression gate. The visual/screenshot lane exists in-repo but still needs harness cleanup before it should be treated as the same maturity tier.
- **Emancipated build path** — Tauri prep/build no longer depends on project-level Node tooling.
- **Standalone finish bar now clears** — the full `src-tauri` suite passes green, making the core Tauri/Rust/protobuf-contract substrate viable both as a standalone local editor runtime and as donor architecture for future systems.

## Testing

```bash
# Rust backend tests (full suite green: 159 library/unit + 44 integration/e2e/property/bench/package tests)
cd src-tauri && cargo test --all-targets

# Prepare frontend bundle without Node
bash ./scripts/prepare-tauri-dist.sh

# WASM crate tests (29 passed)
cd wasm && cargo test

# Live sparse large-document proof (requires local WebKitWebDriver / webkit2gtk-driver)
node tests/visual/sparse-large-document.webdriver.mjs

# cargo-fuzz targets (requires nightly)
cd src-tauri && cargo +nightly fuzz build fuzz_apply_change
cargo +nightly fuzz build fuzz_tokenize
cargo +nightly fuzz build fuzz_compute_line_delta
```

## Roadmap

- `WASM_ROADMAP.md` — WASM bridge, custom renderer, testing, CI
- `PHASED_ROADMAP.md` — Core architecture, buffer management, security, agent integration
- `ADVERSARIAL_REVIEW.md` — Full-spectrum adversarial review of security, correctness, and performance issues
- `ADVERSARIAL_ROADMAP.md` — Sequenced remediation plan derived from the adversarial review
- `HYDRATION.md` — Session history and phase snapshots
- `CAPABILITY_DECLARATION.md` — sovereign outward-facing statement of what Monaco_Rust brings to adjacent projects
- `MCP_TRUTH_SURFACE_PLAN.md` — implementation-facing plan for turning the MCP seam into a truth-bearing external interface

## Emancipation Status

- `done`: Monaco runtime/build path no longer depends on Node for dist preparation, packaging, or project-level tooling.
- `done`: Tauri serves vendored Monaco assets from `out/monaco-editor/min` through `tauri-dist/`.
- `done`: WASM rebuild path is shell-based via `build/wasm/build.sh` plus Rust/wasm-bindgen tooling.

## License

MIT — see [LICENSE.txt](./LICENSE.txt). Monaco Editor is © Microsoft Corporation, used under its MIT license.
