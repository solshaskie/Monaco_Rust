/* tslint:disable */
/* eslint-disable */

/**
 * Parse a compact binary delta or snapshot from Rust `wasm_sync`.
 *
 * The binary format is:
 * - mode 0 (noop): `[0]`
 * - mode 1 (delta): `[1][version_id:8LE][start_line:4LE][end_line:4LE][text_len:4LE][text:utf8]`
 * - mode 2 (snapshot): `[2][version_id:8LE][line_count:4LE][text_len:4LE][text:utf8]`
 *
 * Returns a JS object with `mode` and optional fields.
 */
export function apply_binary_delta(buffer: object): any;

/**
 * Clear the entire token cache.
 */
export function clear_token_cache(): void;

/**
 * Compute a line-based diff between old_text and new_text.
 * Returns a JSON array of edit objects:
 * { "start_line": 0, "start_column": 0, "end_line": 0, "end_column": 0, "text": "..." }
 */
export function compute_diff(old_text: string, new_text: string): string;

/**
 * Compute line layout information for a source string.
 * Returns a JSON array of line info: { "line": 0, "char_count": 0, "wrapped": false }
 * `line_width` is the maximum characters per line before wrapping.
 */
export function compute_layout(source: string, line_width: number): string;

/**
 * Compute viewport-visible lines given total line count, scroll offset and viewport height.
 * Returns { "start_line": 0, "end_line": 0 }
 * Callers should cache `total_lines` (e.g. from `count_lines`) to avoid
 * re-scanning the source on every scroll event.
 */
export function compute_visible_lines(total_lines: number, line_height_px: number, scroll_top_px: number, viewport_height_px: number): string;

/**
 * Return the total line count for a source string (faster than full tokenization).
 */
export function count_lines(source: string): number;

/**
 * Return the current WASM memory size in pages (64 KiB per page).
 */
export function current_memory_pages(): number;

/**
 * Remove a single resource from the token cache.
 */
export function invalidate_token_cache(resource: string): void;

/**
 * Pre-grow the WASM linear memory by `additional_pages` (64 KiB each).
 *
 * Call this before loading large buffers to avoid reallocation stalls
 * during compute operations. Returns the previous page count.
 *
 * # Example
 * - `preallocate_memory(256)` adds 16 MiB (256 × 64 KiB).
 * - The max allowed is governed by `--max-memory` at link time
 *   (currently 128 MiB = 2048 pages; see `wasm/.cargo/config.toml`).
 *
 * # Panics
 * Panics if the grow exceeds `--max-memory` or if the memory object is
 * unavailable (should never happen in practice).
 */
export function preallocate_memory(additional_pages: number): number;

/**
 * Prime the token cache for a resource. Tokenizes the full source and stores
 * it keyed by `resource`. Returns the number of tokens cached.
 */
export function prime_token_cache(resource: string, source: string, language: string): number;

/**
 * Read a UTF-8 string from a JS Uint8Array (or SharedArrayBuffer-backed view)
 * at the given offset and length. This avoids JSON serialization for large buffers.
 */
export function read_string_from_buffer(buffer: object, offset: number, length: number): string;

/**
 * Initialize the WASM module. Call once from JS before any compute functions.
 */
export function start(): void;

/**
 * Return the number of cached resources.
 */
export function token_cache_len(): number;

/**
 * Return approximate total cached bytes.
 */
export function token_cache_total_bytes(): number;

/**
 * Tokenize a source string and return a JSON array of token objects.
 * Each token: { "text": "...", "type": "...", "line": 0, "start": 0, "end": 0 }
 */
export function tokenize(source: string, language: string): string;

/**
 * Tokenize a source string, using the cache when the snapshot is unchanged.
 * Returns a JSON array of token objects.
 */
export function tokenize_cached(source: string, language: string, resource: string): string;

/**
 * Tokenize a range of lines [start_line, end_line) (0-indexed).
 * Faster than full-file tokenization for viewport-aware highlighting.
 */
export function tokenize_range(source: string, language: string, start_line: number, end_line: number): string;

/**
 * Tokenize a range of lines, using the full-file cache when possible.
 * Returns a JSON array of token objects.
 */
export function tokenize_range_cached(source: string, language: string, resource: string, start_line: number, end_line: number): string;

/**
 * Write a UTF-8 string into a JS Uint8Array (or SharedArrayBuffer-backed view)
 * at the given offset. Returns the number of bytes written.
 */
export function write_string_to_buffer(buffer: object, offset: number, text: string): number;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly apply_binary_delta: (a: any) => [number, number, number];
    readonly compute_diff: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly compute_layout: (a: number, b: number, c: number) => [number, number, number, number];
    readonly compute_visible_lines: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly count_lines: (a: number, b: number) => number;
    readonly current_memory_pages: () => number;
    readonly invalidate_token_cache: (a: number, b: number) => void;
    readonly prime_token_cache: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly read_string_from_buffer: (a: any, b: number, c: number) => [number, number, number, number];
    readonly start: () => void;
    readonly tokenize: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly tokenize_cached: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly tokenize_range: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly tokenize_range_cached: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number, number, number];
    readonly write_string_to_buffer: (a: any, b: number, c: number, d: number) => [number, number, number];
    readonly preallocate_memory: (a: number) => number;
    readonly clear_token_cache: () => void;
    readonly token_cache_len: () => number;
    readonly token_cache_total_bytes: () => number;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
