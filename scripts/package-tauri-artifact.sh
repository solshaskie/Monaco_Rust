#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="$ROOT_DIR/artifacts"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --out-dir)
      if [[ $# -lt 2 ]]; then
        echo "Missing value for --out-dir" >&2
        exit 1
      fi
      OUT_DIR="$2"
      shift 2
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

PACKAGE_VERSION="$(sed -n 's/^[[:space:]]*version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$ROOT_DIR/src-tauri/Cargo.toml" | head -n1)"

if [[ -z "$PACKAGE_VERSION" ]]; then
  echo "Failed to determine package version from src-tauri/Cargo.toml" >&2
  exit 1
fi

ARTIFACT_BASE_NAME="monaco-tauri-artifact-v${PACKAGE_VERSION}"
STAGE_DIR="$OUT_DIR/$ARTIFACT_BASE_NAME"
TARBALL_PATH="$OUT_DIR/$ARTIFACT_BASE_NAME.tar.gz"

REQUIRED_PATHS=(
  "tauri-dist"
  "out/monaco-editor/min"
  "proto"
)

for rel_path in "${REQUIRED_PATHS[@]}"; do
  if [[ ! -e "$ROOT_DIR/$rel_path" ]]; then
    echo "Missing required artifact input: $ROOT_DIR/$rel_path" >&2
    exit 1
  fi
done

rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR"

cp -R "$ROOT_DIR/tauri-dist" "$STAGE_DIR/tauri-dist"
mkdir -p "$STAGE_DIR/out/monaco-editor"
cp -R "$ROOT_DIR/out/monaco-editor/min" "$STAGE_DIR/out/monaco-editor/min"
cp -R "$ROOT_DIR/proto" "$STAGE_DIR/proto"

cat > "$STAGE_DIR/artifact-manifest.json" <<EOF
{
  "artifactVersion": 1,
  "packageName": "monaco-tauri",
  "version": "$PACKAGE_VERSION",
  "vscodeRef": null,
  "createdAt": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
  "contract": {
    "monacoOwns": [
      "out/monaco-editor/min",
      "tauri-dist",
      "proto"
    ],
    "vscOwns": [
      "out/monaco-editor.html",
      "desktop/dist",
      "scripts/prepare-tauri-dist.sh"
    ]
  },
  "contents": [
    "tauri-dist",
    "out/monaco-editor/min",
    "proto"
  ],
  "upgradeHints": [
    "Validate tauri-dist locally before refreshing downstream vendored assets.",
    "Do not replace downstream shell HTML with tauri-dist/index.html.",
    "Refresh only the Monaco-owned editor asset payload in downstream consumers."
  ]
}
EOF

mkdir -p "$OUT_DIR"
rm -f "$TARBALL_PATH"
tar -czf "$TARBALL_PATH" -C "$OUT_DIR" "$ARTIFACT_BASE_NAME"

echo "Packaged Monaco Tauri artifact: $TARBALL_PATH"
