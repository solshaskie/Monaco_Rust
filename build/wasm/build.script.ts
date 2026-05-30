/**
 * Build script: compiles the WASM compute module and generates JS bindings.
 *
 * Usage: npx ts-node build/wasm/build.script.ts
 *
 * This script:
 * 1. Compiles the `wasm/` crate to wasm32-unknown-unknown (release)
 * 2. Runs wasm-bindgen to generate JS glue code
 * 3. Copies the .wasm and .js files to tauri/wasm/ for the frontend
 */

import { execSync } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';

const ROOT = path.resolve(__dirname, '..', '..');
const WASM_CRATE = path.join(ROOT, 'wasm');
const OUTPUT_DIR = path.join(ROOT, 'tauri', 'wasm');
const GLUE_DIR = path.join(ROOT, 'tauri');

function ensureDir(dir: string) {
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
}

function run(cmd: string, cwd: string) {
  console.log(`> ${cmd}`);
  execSync(cmd, { cwd, stdio: 'inherit' });
}

function main() {
  console.log('=== WASM Compute Module Build ===');

  // 1. Build the Rust WASM crate
  ensureDir(OUTPUT_DIR);
  run(
    'cargo build --target wasm32-unknown-unknown --release',
    WASM_CRATE
  );

  const wasmInput = path.join(
    WASM_CRATE,
    'target',
    'wasm32-unknown-unknown',
    'release',
    'monaco_wasm.wasm'
  );

  if (!fs.existsSync(wasmInput)) {
    console.error(`WASM output not found: ${wasmInput}`);
    process.exit(1);
  }

  // 2. Generate wasm-bindgen JS bindings
  run(
    `wasm-bindgen "${wasmInput}" --out-dir "${OUTPUT_DIR}" --target web --no-typescript`,
    ROOT
  );

  const jsOut = path.join(OUTPUT_DIR, 'monaco_wasm.js');
  const wasmOut = path.join(OUTPUT_DIR, 'monaco_wasm_bg.wasm');

  if (!fs.existsSync(jsOut) || !fs.existsSync(wasmOut)) {
    console.error('wasm-bindgen output missing');
    process.exit(1);
  }

  const jsStats = fs.statSync(jsOut);
  const wasmStats = fs.statSync(wasmOut);

  console.log(`\n✓ WASM module built successfully`);
  console.log(`  JS glue:  ${(jsStats.size / 1024).toFixed(1)} KB  →  ${jsOut}`);
  console.log(`  WASM:     ${(wasmStats.size / 1024).toFixed(1)} KB  →  ${wasmOut}`);
}

main();
