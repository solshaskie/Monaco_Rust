import fs from 'fs';
import os from 'os';
import path from 'path';
import process from 'process';
import { spawn, spawnSync } from 'child_process';

const repoRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..', '..');
const appBinary = path.resolve(repoRoot, 'src-tauri/target/release/monaco-tauri');
const driverHost = '127.0.0.1';
const driverPort = Number(process.env.TAURI_DRIVER_PORT || 4444);
const nativePort = Number(process.env.TAURI_DRIVER_NATIVE_PORT || 4445);
const baseUrl = `http://${driverHost}:${driverPort}`;

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitFor(fn, description, timeoutMs = 30000, intervalMs = 250) {
  const startedAt = Date.now();
  // eslint-disable-next-line no-constant-condition
  while (true) {
    try {
      const value = await fn();
      if (value) {
        return value;
      }
    } catch (error) {
      if (Date.now() - startedAt >= timeoutMs) {
        throw new Error(`${description} timed out: ${error instanceof Error ? error.message : String(error)}`);
      }
    }

    if (Date.now() - startedAt >= timeoutMs) {
      throw new Error(`${description} timed out after ${timeoutMs}ms`);
    }
    await sleep(intervalMs);
  }
}

async function webdriverRequest(method, route, body) {
  const response = await fetch(`${baseUrl}${route}`, {
    method,
    headers: body ? { 'content-type': 'application/json' } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  });

  const text = await response.text();
  let json = null;
  if (text) {
    try {
      json = JSON.parse(text);
    } catch (error) {
      throw new Error(`failed to parse WebDriver response from ${route}: ${text}`);
    }
  }

  if (!response.ok) {
    throw new Error(`WebDriver ${method} ${route} failed: ${response.status} ${text}`);
  }

  if (json?.value?.error) {
    throw new Error(`WebDriver ${method} ${route} error: ${json.value.error} ${json.value.message || ''}`.trim());
  }

  return json;
}

function buildAsyncScript(expression) {
  return `
    const done = arguments[arguments.length - 1];
    Promise.resolve()
      .then(() => (${expression}))
      .then((value) => done({ ok: true, value }))
      .catch((error) => done({
        ok: false,
        message: error && error.message ? error.message : String(error)
      }));
  `;
}

async function executeAsync(sessionId, expression, args = []) {
  const result = await webdriverRequest(
    'POST',
    `/session/${sessionId}/execute/async`,
    {
      script: buildAsyncScript(expression),
      args,
    }
  );

  if (!result?.value?.ok) {
    throw new Error(result?.value?.message || 'unknown async script failure');
  }

  return result.value.value;
}

async function createSession() {
  const payload = {
    capabilities: {
      alwaysMatch: {
        browserName: 'webkit',
        'tauri:options': {
          application: appBinary,
        },
      },
    },
  };

  const response = await webdriverRequest('POST', '/session', payload);
  const sessionId = response?.sessionId || response?.value?.sessionId;
  if (!sessionId) {
    throw new Error(`session id missing from response: ${JSON.stringify(response)}`);
  }
  return sessionId;
}

function spawnTauriDriver() {
  const args = ['--port', String(driverPort), '--native-port', String(nativePort)];
  if (process.env.TAURI_NATIVE_DRIVER) {
    args.push('--native-driver', process.env.TAURI_NATIVE_DRIVER);
  }

  const child = spawn('tauri-driver', args, {
    cwd: repoRoot,
    env: {
      ...process.env,
      DISPLAY: process.env.DISPLAY || ':0',
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });

  child.stdout.on('data', (chunk) => {
    process.stdout.write(`[tauri-driver] ${chunk}`);
  });
  child.stderr.on('data', (chunk) => {
    process.stderr.write(`[tauri-driver] ${chunk}`);
  });

  return child;
}

async function main() {
  if (!fs.existsSync(appBinary)) {
    throw new Error(`missing Tauri binary at ${appBinary}; build it with 'cd src-tauri && cargo build --release' first`);
  }
  if (!process.env.TAURI_NATIVE_DRIVER) {
    const which = spawnSync('bash', ['-lc', 'command -v WebKitWebDriver'], {
      cwd: repoRoot,
      encoding: 'utf8',
    });
    if (which.status !== 0 || !which.stdout.trim()) {
      throw new Error(
        "missing native WebKit driver. Install 'webkit2gtk-driver' or set TAURI_NATIVE_DRIVER to a WebKitWebDriver binary path"
      );
    }
  }

  const tempFile = path.join(os.tmpdir(), `monaco-sparse-proof-${Date.now()}.log`);
  const content = Array.from({ length: 15000 }, (_, i) => `log-line-${i}\n`).join('');
  fs.writeFileSync(tempFile, content, 'utf8');

  const driver = spawnTauriDriver();
  let sessionId;

  try {
    await waitFor(async () => {
      const status = await webdriverRequest('GET', '/status');
      return status?.value?.ready ?? true;
    }, 'tauri-driver status');

    sessionId = await createSession();

    await webdriverRequest('POST', `/session/${sessionId}/timeouts`, {
      script: 30000,
      pageLoad: 30000,
      implicit: 0,
    });

    await waitFor(async () => {
      return executeAsync(
        sessionId,
        'Boolean(window.__monacoRustTest && document.querySelector(".monaco-editor"))'
      );
    }, 'Monaco sparse test hooks');

    const opened = await executeAsync(
      sessionId,
      'window.__monacoRustTest.openSparseLargeDocument(arguments[0])',
      [tempFile]
    );

    if (opened?.line_count !== 15000) {
      throw new Error(`expected 15000 lines, got ${opened?.line_count}`);
    }

    const viewport = await executeAsync(
      sessionId,
      'window.__monacoRustTest.readSparseViewport(arguments[0], arguments[1], arguments[2])',
      [opened.session_id, 12048, 3]
    );

    if (viewport?.start_line !== 12048 || viewport?.end_line !== 12051) {
      throw new Error(`unexpected viewport range ${JSON.stringify(viewport)}`);
    }
    if (!viewport?.content?.includes('log-line-12048') || !viewport?.content?.includes('log-line-12050')) {
      throw new Error(`viewport content missing expected lines: ${JSON.stringify(viewport)}`);
    }

    const closed = await executeAsync(
      sessionId,
      'window.__monacoRustTest.closeSparseLargeDocument(arguments[0])',
      [opened.session_id]
    );

    if (!closed?.closed) {
      throw new Error(`sparse session did not close cleanly: ${JSON.stringify(closed)}`);
    }

    console.log('Sparse large-document WebDriver proof passed.');
  } finally {
    fs.rmSync(tempFile, { force: true });

    if (sessionId) {
      try {
        await webdriverRequest('DELETE', `/session/${sessionId}`);
      } catch (error) {
        console.error(`failed to delete session ${sessionId}: ${error instanceof Error ? error.message : String(error)}`);
      }
    }

    driver.kill('SIGTERM');
    await new Promise((resolve) => {
      driver.once('exit', resolve);
      setTimeout(resolve, 2000);
    });
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.stack || error.message : String(error));
  process.exitCode = 1;
});
