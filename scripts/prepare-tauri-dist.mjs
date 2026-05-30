import { cpSync, existsSync, mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';

const root = process.cwd();
const monacoOutDir = path.join(root, 'out', 'monaco-editor');
const minDir = path.join(monacoOutDir, 'min');
const distDir = path.join(root, 'tauri-dist');
const vendorDir = path.join(distDir, 'vendor', 'monaco-editor');

if (!existsSync(minDir)) {
  console.error('Missing ./out/monaco-editor/min build output. Run npm run build-lsp && npm run build-monaco-editor before Tauri packaging.');
  process.exit(1);
}

rmSync(distDir, { recursive: true, force: true });
mkdirSync(vendorDir, { recursive: true });
cpSync(path.join(root, 'tauri', 'index.html'), path.join(distDir, 'index.html'));
cpSync(minDir, path.join(vendorDir, 'min'), { recursive: true });
console.log(`Prepared ${distDir} from ${minDir}`);
