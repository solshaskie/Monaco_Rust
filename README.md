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
- **Virtual scroll replacement renderer proven** — `virtual-scroll.js` can mount as the primary renderer for large buffers (>10k lines) via `mountAsPrimary()`. Per-line token hashing eliminates decoration flicker. The `compatibility.js` shim has double-patch guards and idempotent disposal for safe Monaco upgrades.
- **Security sandbox** — capability-based permissions for file system access with real path containment tests.
- **MCP agent integration** — 22+ Model Context Protocol tools for agent-driven editing, inspection, structural review, event subscription, workspace watching, and contradiction detection.
- **LSP performance hardened** — debounced diagnostics (150ms), batched `didChange` notifications (50ms), and cached `SyntaxParser` per language.
- **CI hardened** — cargo audit + cargo deny, SHA-pinned actions, `dtolnay/rust-toolchain` correction, cargo-fuzz targets on nightly, visual regression baselines with >1% pixel-diff gating, and performance regression baselines with >20% regression gate.
- **Emancipated build path** — Tauri prep/build no longer depends on project-level Node tooling.

## Testing

```bash
# Rust backend tests (156 passed; 2 pre-existing TypeScript parser failures unrelated to current changes)
cd src-tauri && cargo test --all-targets

# Prepare frontend bundle without Node
bash ./scripts/prepare-tauri-dist.sh

# WASM crate tests (29 passed)
cd wasm && cargo test

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
