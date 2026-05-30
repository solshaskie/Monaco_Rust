# Monaco_Rust Hydration Packet

> Read this first when resuming work. This file is intentionally short and verified against the repo on 2026-05-29.

## Project in One Sentence

Monaco_Rust is a local-first Monaco/Tauri editor experiment where Rust owns buffer state, file operations, syntax services, and agent-facing tools, while the frontend shell and optional WASM modules handle presentation and compute.

## Verified State

- Rust backend compiles and the full `src-tauri` test suite passes.
- `cargo test --all-targets` passed on 2026-05-29.
- Tauri dist preparation, artifact packaging, and WASM rebuild paths now run through local shell scripts, not Node.
- Current Rust suite totals:
  - 139 library/unit tests
  - 6 integration tests in `m4_integration.rs`
  - 14 protobuf roundtrip tests in `m7_protobuf_roundtrip.rs`
  - 7 benchmark-style regression tests in `m7_buffer_bench.rs`
  - 1 artifact packaging test in `m4_artifact_packaging.rs`
- Total verified Rust tests: 167
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
- Packaging
  - Tauri artifact packager and dist-prep now run from shell scripts against vendored Monaco assets.
- Rebuild tooling
  - WASM rebuilds now run from `build/wasm/build.sh` using Rust plus `wasm-bindgen`.

## Current Truth Boundaries

- The Rust/Tauri substrate is the strongest part of the repo.
- The WASM lane is real but still a secondary track, not the authoritative runtime.
- The repo posture is now fully emancipated from Node-era project tooling. Browser JS remains as checked-in runtime assets, but build/package orchestration is Rust/shell owned.
- `HYDRATION.md` is now calibrated to current repo truth; older counts and blanket “all phases complete” language were removed because they had drifted.

## Resume Path

1. Run `cd src-tauri && cargo test --all-targets`.
2. Read [PHASED_ROADMAP.md](./PHASED_ROADMAP.md) for the core build path.
3. Read [WASM_ROADMAP.md](./WASM_ROADMAP.md) only if the task touches the deferred compute/rendering lane.
4. Work from `src-tauri/src/` first if the task affects truth, state, events, syntax, or agent operations.

## Near-Term Risks

- The repo had documentation drift recently; keep docs tied to verified commands, not inherited claims.
- The frontend and WASM surfaces still deserve separate verification from Rust green status.
- Unicode and non-ASCII stress coverage remains worth deepening in buffer and position conversion paths.

## Latest Cleanup

- Fixed the packaging test so it follows the Rust-owned crate version instead of a stale hard-coded artifact name.
- Replaced Node-based `tauri-dist` preparation and artifact packaging with shell scripts that use vendored Monaco assets from `out/monaco-editor/min`.
- Removed the root Node manifest/tooling surfaces and replaced the WASM rebuild path with `build/wasm/build.sh`.
- Reduced warning noise in several Rust modules by trimming unused imports, duplicate token match arms, and unused test variables.
