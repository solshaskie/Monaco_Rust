# MCP Truth Surface Plan

## Purpose

This document defines the next disciplined expansion of Monaco_Rust's MCP surface.

The goal is not to add more tools for their own sake.

The goal is to turn the MCP seam into a truthful external interface over Monaco_Rust's sovereign runtime:

- exact symbol queries
- exact buffer/version lineage
- structural edits instead of text-only edits
- proof and evidence-bearing responses
- event subscriptions and change watchers
- contradiction and drift inspection hooks
- explicit certainty and provenance on every nontrivial result

This is the path from "agent-editable editor runtime" to "truth-bearing editor runtime with inspectable external seams."

## Current Starting Point

Today Monaco_Rust already has a real MCP foothold:

- `read_file`
- `edit_file`
- `list_symbols`
- `apply_edits`
- `get_buffer_metadata`
- `get_buffer_snapshot_proof`

These sit on top of the same `BufferRegistry` and versioned runtime state as the UI.

That is the right base.

The first truth-surface tranche is now partially landed:

- `McpToolResult` carries certainty, provenance, evidence, and structured data.
- `get_buffer_metadata` returns exact runtime buffer facts.
- `get_buffer_snapshot_proof` returns exact snapshot evidence with SHA-256 content proof.
- `get_symbol_index` returns structured Tree-sitter symbol facts.
- `get_symbol_at_position` returns the deepest exact symbol at a queried position.
- `get_buffer_version_lineage` reports the current lineage model and its present proof boundary.
- older tools now also emit explicit truth metadata instead of summary-only results.

Current supporting seams already present in the repo:

- `src-tauri/src/mcp/tools.rs`
  - existing tool registry and execution pattern
- `src-tauri/src/buffer/registry.rs`
  - authoritative buffer access, versions, dirty state, undo/redo, snapshots
- `src-tauri/src/events.rs`
  - event types and broadcaster for content/save/open/close/workspace changes
- `src-tauri/src/syntax_handlers.rs`
  - typed syntax/document-symbol/hover/diagnostics/completion surfaces

So the needed work is not invention from nothing.
It is disciplined elevation of an existing seam.

## Governing Rules

### 1. Facts First

Every MCP tool should prefer exact facts over prose.

Return:

- exact ids
- exact ranges
- exact versions
- exact hashes
- exact timestamps
- exact source references

Do not make human-readable explanation the only payload.

### 2. Provenance Required

Every nontrivial result should say where it came from.

At minimum, the MCP result envelope should eventually support:

- `source_kind`
  - `buffer`, `file`, `syntax`, `event`, `derived`
- `path`
- `buffer_version`
- `timestamp`
- `producer`
  - e.g. `buffer_registry`, `syntax_handlers`, `event_broadcaster`

### 3. Certainty Must Be Explicit

Every nontrivial tool result should declare a certainty class.

Planned classes:

- `observed`
- `derived`
- `heuristic`
- `speculative`
- `unknown`

No heuristic result should masquerade as an exact one.

### 4. Structural Beats Textual

Whenever the runtime can operate over structure safely, prefer that to raw string surgery.

Text-based mutation is still useful, but it should no longer be the ceiling.

### 5. Watchable Truth Beats Polling

Where practical, external systems should subscribe to state changes instead of repeatedly asking "what changed?"

MCP should expose a path toward watch/change semantics, even if that begins as simple subscription registration and event snapshot retrieval.

## Target MCP v2 Envelope

The current `McpToolResult` is intentionally minimal:

- `success`
- `content`
- `version_id`
- `error`

That was fine for the first tranche.

The next envelope should move toward something like:

```json
{
  "success": true,
  "status": "ok",
  "certainty": "observed",
  "content": "...optional human-readable summary...",
  "version_id": 12,
  "provenance": {
    "source_kind": "buffer",
    "path": "/workspace/src/main.rs",
    "buffer_version": 12,
    "producer": "buffer_registry",
    "timestamp": "..."
  },
  "evidence": {
    "content_hash": "...",
    "ranges": [],
    "symbol_ids": [],
    "event_ids": []
  },
  "data": {}
}
```

This does not need to land all at once.
But it should be the contract direction.

## Planned Tool Families

### Family 1: Exact Buffer Truth

These are the highest-yield first tools because the runtime already owns the needed facts.

#### `get_buffer_metadata`

Return:

- path
- current version id
- dirty state
- can undo / can redo
- line count
- byte size
- content hash
- source of truth
  - `open_buffer`, `file_backed`, `unsaved_buffer`, etc.

Why:

- Gives external systems exact runtime posture without reading the whole file.

#### `get_buffer_snapshot_proof`

Return:

- exact content
- version id
- content hash
- dirty state
- timestamp/provenance metadata

Why:

- `read_file` is useful, but this makes the proof surface explicit.

#### `compare_buffer_versions`

Return:

- from version
- to version
- exact changed ranges
- whether comparison is exact or only partially reconstructable

Why:

- Begins turning versions into lineage, not just counters.

### Family 2: Exact Symbol Queries

These are the next most valuable because Monaco_Rust already has document symbol extraction and hover/path-position semantics.

#### `get_symbol_index`

Return:

- exact symbols for a path
- name
- kind
- full range
- selection/name range
- children
- version id

Why:

- More truthful than today’s prose-oriented `list_symbols`.

#### `get_symbol_at_position`

Return:

- the exact symbol owning a line/column position
- surrounding container symbol
- exact range metadata

Why:

- Ideal for reasoning systems and semantic overlays.

#### `query_symbols`

Query by:

- name
- kind
- prefix
- parent/container
- exact range overlap

Why:

- Turns symbol extraction into a reusable truth surface.

### Family 3: Structural Edits

This is where the seam starts becoming materially more powerful.

#### `structural_edit`

Early supported operations should be narrow and explicit:

- rename symbol
- replace function body
- insert item before/after symbol
- delete symbol

Important rule:

- if the runtime cannot prove the structural target exactly, the operation must fail clearly instead of silently degrading to fuzzy text replacement.

Why:

- Makes mutation safer and more inspectable than raw text substitution.

#### `preview_structural_edit`

Return:

- exact target symbol/range
- exact transformed spans
- version precondition
- certainty class

Why:

- Lets external callers prove what would happen before mutation.

### Family 4: Event Watches and Change Subscriptions

Monaco_Rust already has event types and broadcasting.
The MCP seam should stop ignoring that.

#### `subscribe_buffer_events`

Initial scope:

- content changed
- saved
- opened
- closed
- diagnostics changed

Return:

- subscription id
- watched path(s)
- event kinds

#### `poll_buffer_events`

Return:

- all events since a cursor/sequence id
- explicit ordering
- exact payloads

Why:

- Makes external semantic/runtime agents observers of truth instead of blind pollers.

### Family 5: Drift and Contradiction Hooks

These should begin narrow.
Do not start with a giant "analyze everything" tool.

#### `inspect_buffer_drift`

Early comparisons:

- buffer vs saved file
- symbol set vs previous version
- diagnostics vs previous diagnostics

Return:

- exact categories of change
- exact ranges/symbols affected
- certainty class

#### `inspect_contradictions`

Initial contradiction lanes could include:

- symbol declared but not structurally present after change
- diagnostics claim parse break while symbol query still assumes stability
- external semantic report references missing ranges/symbols

Why:

- Gives the runtime a first-class way to say "these truth surfaces disagree."

## Planned Implementation Order

This should be built in thin, high-yield tranches:

### Tranche 1: Shared Result Envelope

Upgrade `McpToolResult` directionally toward:

- `certainty`
- `provenance`
- `evidence`
- optional `data`

Do not overcomplicate the first pass.
Even a partial envelope is better than none.

### Tranche 2: Exact Buffer Truth

Implement:

- `get_buffer_metadata`
- `get_buffer_snapshot_proof`

These are low-risk and immediately valuable.

### Tranche 3: Exact Symbol Queries

Implement:

- `get_symbol_index`
- `get_symbol_at_position`

These can reuse existing symbol and syntax infrastructure.

### Tranche 4: Event Watches

Implement:

- `subscribe_buffer_events`
- `poll_buffer_events`

Start with in-process/runtime-local subscription tracking if needed.

### Tranche 5: Structural Edit Preview

Implement:

- `preview_structural_edit`

Do preview before mutation.
That preserves rigor.

### Tranche 6: Structural Edit Application

Implement:

- `structural_edit`

Only for the exact operations the runtime can truly support.

### Tranche 7: Drift / Contradiction Hooks

Implement:

- `inspect_buffer_drift`
- `inspect_contradictions`

Keep the first slice narrow and exact.

## What Must Be Forbidden

These are anti-goals.

- fuzzy mutation without version checks
- symbol "best guesses" presented as exact
- prose-only results with no evidence payload
- broad analysis tools that do not say whether results are observed or heuristic
- hidden side effects across tools
- structural edits that silently fall back to string search and replace

## What Success Looks Like

Monaco_Rust MCP should evolve from:

- "an agent can read and edit files through the editor runtime"

to:

- "external systems can query exact local editor truth, observe change over time, perform disciplined structural operations, and receive explicit evidence and certainty on every claim"

That is real value.

That is a seam worthy of trust.
