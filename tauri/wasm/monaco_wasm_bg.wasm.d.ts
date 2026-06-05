/* tslint:disable */
/* eslint-disable */
export const memory: WebAssembly.Memory;
export const apply_binary_delta: (a: any) => [number, number, number];
export const compute_diff: (a: number, b: number, c: number, d: number) => [number, number, number, number];
export const compute_layout: (a: number, b: number, c: number) => [number, number, number, number];
export const compute_visible_lines: (a: number, b: number, c: number, d: number) => [number, number, number, number];
export const count_lines: (a: number, b: number) => number;
export const current_memory_pages: () => number;
export const invalidate_token_cache: (a: number, b: number) => void;
export const prime_token_cache: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
export const read_string_from_buffer: (a: any, b: number, c: number) => [number, number, number, number];
export const start: () => void;
export const tokenize: (a: number, b: number, c: number, d: number) => [number, number, number, number];
export const tokenize_cached: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
export const tokenize_range: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
export const tokenize_range_cached: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number, number, number];
export const write_string_to_buffer: (a: any, b: number, c: number, d: number) => [number, number, number];
export const preallocate_memory: (a: number) => number;
export const clear_token_cache: () => void;
export const token_cache_len: () => number;
export const token_cache_total_bytes: () => number;
export const __wbindgen_malloc: (a: number, b: number) => number;
export const __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
export const __wbindgen_externrefs: WebAssembly.Table;
export const __externref_table_dealloc: (a: number) => void;
export const __wbindgen_free: (a: number, b: number, c: number) => void;
export const __wbindgen_start: () => void;
