import { ChildProcess, execSync, spawn } from 'child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync, readFileSync } from 'fs';
import * as http from 'http';
import * as net from 'net';
import * as os from 'os';
import * as path from 'path';

const STATE_FILE = path.join(os.tmpdir(), 'hermes-e2e-state.json');

function findFreePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const srv = net.createServer();
    srv.listen(0, '127.0.0.1', () => {
      const addr = srv.address() as net.AddressInfo;
      const port = addr.port;
      srv.close((err) => {
        if (err) reject(err);
        else resolve(port);
      });
    });
  });
}

function waitForServer(url: string, timeoutMs = 30_000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  return new Promise((resolve, reject) => {
    function tryOnce() {
      http
        .get(url, (res) => {
          res.resume();
          resolve();
        })
        .on('error', () => {
          if (Date.now() > deadline) {
            reject(new Error(`Server at ${url} did not start within ${timeoutMs}ms`));
          } else {
            setTimeout(tryOnce, 500);
          }
        });
    }
    tryOnce();
  });
}

/** globalSetup — starts the Hermes binary with a temporary DB */
export default async function setup() {
  const port = await findFreePort();

  const tmpDir = mkdtempSync(path.join(os.tmpdir(), 'hermes-e2e-'));
  const dbPath = path.join(tmpDir, 'hermes.db');
  const storageDir = path.join(tmpDir, 'uploads');
  mkdirSync(storageDir, { recursive: true });

  const repoRoot = path.resolve(__dirname, '../..');

  if (!process.env.SKIP_BUILD) {
    console.log('\n[e2e] Building hermes (this may take a while)…');
    execSync('dx build --platform web --release', {
      cwd: repoRoot,
      stdio: 'inherit',
    });
  }

  // dx build outputs the binary + public/ together under target/dx/hermes/release/web/
  const bundleDir = path.join(repoRoot, 'target', 'dx', 'hermes', 'release', 'web');
  const binaryPath = path.join(bundleDir, 'hermes');
  const baseUrl = `http://127.0.0.1:${port}`;

  const child: ChildProcess = spawn(binaryPath, [], {
    // Run from bundleDir so dioxus-server finds ./public next to the binary
    cwd: bundleDir,
    env: {
      ...process.env,
      PORT: String(port),
      HOST: '127.0.0.1',
      DATABASE_URL: `sqlite://${dbPath}?mode=rwc`,
      STORAGE_DIR: storageDir,
      BASE_URL: baseUrl,
      RUST_LOG: 'hermes=info',
    },
    detached: false,
    stdio: ['ignore', 'inherit', 'inherit'],
  });

  child.on('error', (err) => {
    console.error('[e2e] Server process error:', err);
  });

  // Persist PID and dirs so teardown.ts can clean up
  writeFileSync(
    STATE_FILE,
    JSON.stringify({ pid: child.pid, tmpDir, baseUrl }),
  );

  process.env.E2E_BASE_URL = baseUrl;

  console.log(`\n[e2e] Waiting for server at ${baseUrl}…`);
  await waitForServer(baseUrl);
  console.log('[e2e] Server ready.\n');
}

/** globalTeardown — kills the server and removes temp files */
export async function teardown() {
  let state: { pid: number; tmpDir: string } | null = null;
  try {
    state = JSON.parse(readFileSync(STATE_FILE, 'utf8'));
  } catch {
    return;
  }

  if (state?.pid) {
    try {
      process.kill(state.pid, 'SIGTERM');
      // Give it up to 3 s to exit
      await new Promise<void>((resolve) => setTimeout(resolve, 3000));
    } catch {
      // process may have already exited
    }
  }

  if (state?.tmpDir) {
    rmSync(state.tmpDir, { recursive: true, force: true });
  }

  rmSync(STATE_FILE, { force: true });
}
