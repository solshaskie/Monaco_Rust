import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { basename, dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '..');
const packageJson = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
const defaultOutDir = join(root, 'artifacts');

const args = process.argv.slice(2);
let outDir = defaultOutDir;

for (let i = 0; i < args.length; i += 1) {
  const arg = args[i];
  if (arg === '--out-dir') {
    const next = args[i + 1];
    if (!next) {
      console.error('Missing value for --out-dir');
      process.exit(1);
    }
    outDir = resolve(next);
    i += 1;
    continue;
  }

  console.error(`Unknown argument: ${arg}`);
  process.exit(1);
}

const version = packageJson.version;
const artifactBaseName = `monaco-tauri-artifact-v${version}`;
const stageDir = join(outDir, artifactBaseName);
const tarballPath = join(outDir, `${artifactBaseName}.tar.gz`);

const requiredCopies = [
  ['tauri-dist', 'tauri-dist'],
  ['out/monaco-editor/min', 'out/monaco-editor/min'],
  ['proto', 'proto'],
];

for (const [fromRel] of requiredCopies) {
  const source = join(root, fromRel);
  if (!existsSync(source)) {
    console.error(`Missing required artifact input: ${source}`);
    process.exit(1);
  }
}

rmSync(stageDir, { recursive: true, force: true });
mkdirSync(stageDir, { recursive: true });

for (const [fromRel, toRel] of requiredCopies) {
  cpSync(join(root, fromRel), join(stageDir, toRel), { recursive: true });
}

const manifest = {
  artifactVersion: 1,
  packageName: packageJson.name,
  version,
  vscodeRef: packageJson.vscodeRef ?? null,
  createdAt: new Date().toISOString(),
  contract: {
    monacoOwns: [
      'out/monaco-editor/min',
      'tauri-dist',
      'proto',
    ],
    vscOwns: [
      'out/monaco-editor.html',
      'desktop/dist',
      'scripts/prepare-tauri-dist.mjs',
    ],
  },
  contents: requiredCopies.map(([, toRel]) => toRel),
  upgradeHints: [
    'Validate tauri-dist locally before refreshing downstream vendored assets.',
    'Do not replace downstream shell HTML with tauri-dist/index.html.',
    'Refresh only the Monaco-owned editor asset payload in downstream consumers.',
  ],
};

writeFileSync(
  join(stageDir, 'artifact-manifest.json'),
  `${JSON.stringify(manifest, null, 2)}\n`,
  'utf8'
);

mkdirSync(outDir, { recursive: true });
rmSync(tarballPath, { force: true });

const tarResult = spawnSync(
  'tar',
  ['-czf', tarballPath, '-C', outDir, basename(stageDir)],
  { stdio: 'inherit' }
);

if (tarResult.status !== 0) {
  process.exit(tarResult.status ?? 1);
}

console.log(`Packaged Monaco Tauri artifact: ${tarballPath}`);
