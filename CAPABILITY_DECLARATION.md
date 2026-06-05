# Capability Declaration

## Purpose

This document is Monaco_Rust's sovereign announcement to adjacent projects.

It states, plainly and without product costume, what this asset brings to the table now.

It is not a roadmap.
It is not a wish list.
It is a declaration of available posture, available seams, and intended contribution.

## Identity

Monaco_Rust is a local-first editing and truth-surface substrate.

It is built to take the veteran strengths of Monaco and rehouse them inside a sovereign Rust/Tauri runtime that can be shaped for governed local work rather than inherited platform assumptions.

Monaco_Rust does not claim to be the whole workspace organism.
It claims to be a disciplined surface and runtime seam for exact text, exact state, exact structure, and higher-order integration.

Its strongest completed layer today is the sovereign Rust/Tauri runtime and contract surface.
Its hybrid WASM/rendering lane is real and increasingly capable, but remains additive rather than authoritative.

## What Monaco_Rust Brings

### 1. A Sovereign Local Editor Runtime

Monaco_Rust brings an editor/runtime posture where authoritative state does not live in an opaque browser/tooling stack.

It owns locally:

- buffer state
- file I/O
- edit application
- version tracking
- undo/redo
- snapshot retrieval
- sparse large-file viewport sessions

This makes it useful wherever another project needs a precise human-facing editing surface backed by governed local authority.

It also now has the first explicit seam for host-owned large-file inspection: Rust can hold a file as a sparse session and serve viewport slices without requiring the normal editable buffer path to hydrate the whole document.

### 2. Explicit Contracts Instead of Hidden Coupling

Monaco_Rust brings explicit type contracts for editor, host, file, language, and event surfaces.

Internal Rust types are generated from protobuf schemas; the IPC boundary between Rust and the frontend is JSON-encoded over Tauri's invoke API. That means adjacent projects can integrate with it through known shapes instead of accidental internal assumptions.

It is suitable for projects that value:

- schema clarity
- replayable state transitions
- stable transport boundaries
- inspectable message surfaces

### 3. A Human-Facing Surface for Truth Work

Monaco_Rust brings more than raw editing.

It brings a place where exact source state, exact structure, exact diagnostics, and exact events can be surfaced to a human without collapsing them into summary-only views.

That makes it a strong candidate surface for:

- review overlays
- contradiction overlays
- semantic drift warnings
- lineage and provenance views
- exact diff and symbol inspection

### 4. A Host for Semantic Organs

Monaco_Rust does not need to own every higher capability itself.

What it brings is a strong host seam for semantic organs supplied by adjacent systems.

Examples of what can be invited in:

- Relatent-style semantic indexing
- stable symbol identity
- drift analysis
- proof and uncertainty overlays

What it already provides directly:

- structural editing (`preview_structural_edit`, `structural_edit`) via tree-sitter AST-guided text replacement
- contradiction detection (`inspect_contradictions`) comparing buffer, disk, and diagnostic truth surfaces

Its role in that relationship is not to become those systems.
Its role is to give them a sovereign local surface in which to appear, interact, and be governed.

### 5. Agent-Facing Capability Seams

Monaco_Rust brings a bounded MCP-style tool surface for agent-driven editing and inspection.

Current agent-facing capabilities include:

- `read_file`
- `edit_file`
- `apply_edits`
- `list_symbols`
- `get_symbol_at_position`
- `get_buffer_metadata`
- `get_buffer_version_lineage`
- `get_buffer_snapshot_proof`
- `get_symbol_index`
- `compare_buffer_versions`
- `inspect_buffer_drift`
- `diff_files`
- `list_open_buffers`
- `replay_buffer`
- `get_buffer_byte_range`
- `search_text_in_buffers`
- `query_symbols`
- `subscribe_buffer_events`
- `poll_buffer_events`
- `close_buffers_batch`
- `watch_workspace`
- `preview_structural_edit`
- `structural_edit`
- `inspect_contradictions`

These are meaningful because they operate against the same authoritative runtime state as the UI, not a parallel shadow layer.

### 6. Evented Multi-View State

Monaco_Rust brings shared buffer state with event broadcasting and in-memory event subscriptions.

The `BufferRegistry` maintains bounded per-buffer event queues (`subscribe_buffer_events` / `poll_buffer_events`) so agents and external processes can observe open, edit, save, and close lifecycle events without polling the full state.

Workspace-level filesystem watching (`watch_workspace`) is also available via the `notify` crate, enabling external change detection for files outside the active buffer set.

That makes it suitable for futures where:

- multiple views observe the same file
- external semantic/runtime processes report changes back into the surface
- agent actions need visible synchronization instead of hidden mutation
- external watchers need to react to filesystem changes without re-scanning

### 7. Emancipated Local Build Posture

Monaco_Rust brings a build/runtime posture that is no longer ruled by Node-era project tooling.

The repo now stands on:

- Rust/Tauri runtime ownership
- checked-in browser assets
- shell-based dist preparation
- shell-based artifact packaging
- shell-based WASM rebuild flow

That matters because it reduces inherited constraint and makes the asset easier to reshape around purpose.

### 8. Standalone and Donor Readiness

Monaco_Rust now clears a real standalone finish bar for its core substrate.

The Rust/Tauri runtime, contract layer, MCP surface, and host-level test suite are green and usable in their own right.

That makes the project suitable not only as a local editor/runtime in direct use, but as donor architecture for future work such as:

- VS Code Rust refactor experiments
- extension-host restructuring
- local-first editing/runtime optimization work
- new governed workstation ideas built on exact state and explicit seams

## What Monaco_Rust Does Not Claim

Monaco_Rust does not claim:

- to be a complete IDE replacement today
- to be the sovereign workspace authority for every project concern
- to replace semantic systems like Relatent
- to replace reasoning systems like RLM
- to be the final truth engine by itself

It is a surface and runtime substrate.
Its strength is in being exact, governable, and open to purposeful integration.

## Best-Fit Role in the Project Vault

Monaco_Rust is best understood as:

- the sovereign editor/runtime surface
- the local cockpit glass for exact text and exact structure
- the host for semantic and reasoning organs supplied by adjacent projects

In that vault-wide role split:

- local runtimes present truth
- reasoning systems range over truth
- semantic systems classify and relate truth
- Monaco_Rust provides the precise human-facing place where those results can be seen and acted on

## Integration Posture

Projects should approach Monaco_Rust as:

- a hostable local editing surface
- a contract-bearing runtime seam
- a candidate truth workstation front-end
- a component that can be shaped without inherited VS Code product assumptions

The strongest integrations will preserve this rule:

Monaco_Rust should remain the sovereign surface for local edit/state presentation, while external systems contribute interpretation, analysis, or reasoning through explicit, inspectable seams.

## Declaration

Monaco_Rust is available as a sovereign local capability.

It brings:

- exact editing
- exact state
- explicit contracts
- evented runtime truth
- agent-facing seams
- a strong surface for semantic and reasoning augmentation

It is not asking to inherit the old world.
It is announcing readiness to serve a new one.
