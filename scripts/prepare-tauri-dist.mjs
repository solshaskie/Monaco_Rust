import { cpSync, existsSync, mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';

const root = process.cwd();
const monacoNpmDir = path.join(root, 'node_modules', 'monaco-editor');
const minDir = path.join(monacoNpmDir, 'min');
const distDir = path.join(root, 'tauri-dist');
const vendorDir = path.join(distDir, 'vendor', 'monaco-editor');

if (!existsSync(minDir)) {
  console.error('Missing node_modules/monaco-editor/min. Run npm install before Tauri packaging.');
  process.exit(1);
}

rmSync(distDir, { recursive: true, force: true });
mkdirSync(vendorDir, { recursive: true });
cpSync(path.join(root, 'tauri', 'index.html'), path.join(distDir, 'index.html'));
cpSync(minDir, path.join(vendorDir, 'min'), { recursive: true });
console.log(`Prepared ${distDir} from ${minDir}`);
