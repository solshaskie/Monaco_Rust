# Monaco Rust

A native desktop code editor built on [Monaco Editor](https://microsoft.github.io/monaco-editor/) (VS Code's editor), [Tauri](https://tauri.app/) (Rust native backend), and WebAssembly compute modules.

## Architecture

**Hybrid: Native Rust backend + WASM compute modules inside Monaco webview.**

- **Rust backend** (`src-tauri/`) — authoritative buffer state, file system, protobuf IPC
- **Monaco frontend** (`tauri/`) — editor UI, served in Tauri webview
- **WASM compute** (`wasm/`) — tokenization, diffing, layout, semantic analysis
- **Incremental sync** (`src-tauri/src/wasm_sync.rs`) — Rust sends only changed lines to WASM

## Quick Start

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri:dev

# Build release binary
npm run tauri:build
```

## Project Structure

| Directory | Purpose |
|-----------|---------|
| `src-tauri/` | Rust backend — buffer registry, file I/O, Tauri commands |
| `tauri/` | Frontend — Monaco editor, WASM glue, renderer |
| `wasm/` | WASM compute crate — tokenize, diff, layout (Rust → wasm-bindgen) |
| `proto/` | Protobuf schemas for Rust ↔ frontend IPC |
| `monaco-lsp-client/` | LSP client adapter for Monaco |
| `build/wasm/` | WASM build automation |
| `scripts/` | Tauri packaging scripts |
| `test/e2e/` | Playwright E2E tests |
| `.github/workflows/` | CI — Rust tests, WASM tests, multi-platform builds |

## Key Features

- **WASM compute offload** — Tokenization, diffing, and layout run in WASM inside the webview
- **Zero-copy buffer passing** — `Uint8Array` / `SharedArrayBuffer` for JS ↔ WASM data
- **Incremental sync** — Only changed line deltas sent from Rust to WASM
- **Virtual scroll renderer** — DOM node pooling for 100k+ line files
- **Security sandbox** — Capability-based permissions for file system access
- **MCP agent integration** — Model Context Protocol tools for agent-driven editing

## Testing

```bash
# Rust backend tests
cd src-tauri && cargo test --all-targets

# WASM crate tests
cd wasm && cargo test

# E2E tests
npm run test:e2e
```

## Roadmap

- `WASM_ROADMAP.md` — WASM bridge, custom renderer, testing, CI
- `PHASED_ROADMAP.md` — Core architecture, buffer management, security, agent integration
- `HYDRATION.md` — Session history and phase snapshots

## License

MIT — see [LICENSE.txt](./LICENSE.txt). Monaco Editor is © Microsoft Corporation, used under its MIT license.
