# Monaco Rust

A native desktop code editor built on [Monaco Editor](https://microsoft.github.io/monaco-editor/) (VS Code's editor), [Tauri](https://tauri.app/) (Rust native backend), and WebAssembly compute modules.

Runtime posture: this repo now stands on a local Rust/Tauri runtime plus checked-in browser assets. The old Node-based build and packaging chain has been removed from the project.

## Architecture

**Hybrid: Native Rust backend + WASM compute modules inside Monaco webview.**

- **Rust backend** (`src-tauri/`) — authoritative buffer state, file system, protobuf IPC
- **Monaco frontend** (`tauri/`) — editor UI, served in Tauri webview
- **WASM compute** (`wasm/`) — tokenization, diffing, layout, semantic analysis
- **Incremental sync** (`src-tauri/src/wasm_sync.rs`) — Rust sends only changed lines to WASM

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
| `proto/` | Protobuf schemas for Rust ↔ frontend IPC |
| `build/wasm/` | WASM build automation |
| `scripts/` | Shell-based Tauri packaging scripts |
| `.github/workflows/` | CI and release automation |

## Current State

- **Rust/Tauri runtime is the authoritative substrate** — buffer state, file operations, syntax services, and MCP tools are real and verified.
- **WASM compute lane exists** — the `wasm/` crate, generated browser bundle, and frontend glue are present for tokenization, diffing, and layout helpers.
- **Incremental sync path now exists end-to-end** — `src-tauri/src/wasm_sync.rs` plus the frontend WASM sync manager can prime and refresh Rust-owned snapshot/delta state on open and edit flows, and the current token consumers now respect the sync barrier during burst edits. The binary transport path exists too, but it is not yet the default frontend path.
- **Renderer experiments exist** — virtual-scroll, decoration, and compatibility adapters are in-repo, but they should be treated as prototype surfaces rather than fully proven replacements for Monaco's default renderer.
- **Security sandbox** — capability-based permissions for file system access.
- **MCP agent integration** — Model Context Protocol tools for agent-driven editing and inspection.
- **Emancipated build path** — Tauri prep/build no longer depends on project-level Node tooling.

## Testing

```bash
# Rust backend tests
cd src-tauri && cargo test --all-targets

# Prepare frontend bundle without Node
bash ./scripts/prepare-tauri-dist.sh

# WASM crate tests
cd wasm && cargo test
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
