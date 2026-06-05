#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WASM_CRATE_DIR="$ROOT_DIR/wasm"
OUTPUT_DIR="$ROOT_DIR/tauri/wasm"
WASM_INPUT="$WASM_CRATE_DIR/target/wasm32-unknown-unknown/release/monaco_wasm.wasm"

mkdir -p "$OUTPUT_DIR"

echo "=== WASM Compute Module Build ==="
echo "> cargo build --target wasm32-unknown-unknown --release"
cargo build --target wasm32-unknown-unknown --release --manifest-path "$WASM_CRATE_DIR/Cargo.toml"

if [[ ! -f "$WASM_INPUT" ]]; then
  echo "WASM output not found: $WASM_INPUT" >&2
  exit 1
fi

echo "> wasm-bindgen \"$WASM_INPUT\" --out-dir \"$OUTPUT_DIR\" --target web"
wasm-bindgen "$WASM_INPUT" --out-dir "$OUTPUT_DIR" --target web

JS_OUT="$OUTPUT_DIR/monaco_wasm.js"
WASM_OUT="$OUTPUT_DIR/monaco_wasm_bg.wasm"
DTS_OUT="$OUTPUT_DIR/monaco_wasm.d.ts"

if [[ ! -f "$JS_OUT" || ! -f "$WASM_OUT" || ! -f "$DTS_OUT" ]]; then
  echo "wasm-bindgen output missing" >&2
  exit 1
fi

echo "WASM module built successfully"
echo "  JS glue:  $JS_OUT"
echo "  WASM:     $WASM_OUT"
