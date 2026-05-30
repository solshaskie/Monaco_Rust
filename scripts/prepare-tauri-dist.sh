#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_HTML="$ROOT_DIR/tauri/index.html"
SOURCE_MONACO_DIR="$ROOT_DIR/out/monaco-editor/min"
DIST_DIR="$ROOT_DIR/tauri-dist"
VENDOR_DIR="$DIST_DIR/vendor/monaco-editor"

if [[ ! -f "$SOURCE_HTML" ]]; then
  echo "Missing frontend shell: $SOURCE_HTML" >&2
  exit 1
fi

if [[ ! -d "$SOURCE_MONACO_DIR" ]]; then
  echo "Missing vendored Monaco assets: $SOURCE_MONACO_DIR" >&2
  echo "Expected checked-in Monaco assets under out/monaco-editor/min." >&2
  exit 1
fi

rm -rf "$DIST_DIR"
mkdir -p "$VENDOR_DIR"
cp "$SOURCE_HTML" "$DIST_DIR/index.html"
cp -R "$SOURCE_MONACO_DIR" "$VENDOR_DIR/min"

echo "Prepared $DIST_DIR from vendored Monaco assets in $SOURCE_MONACO_DIR"
