import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { mkdir, writeFile, readdir, stat } from 'node:fs/promises';
import { createServer } from 'node:net';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';

const root = resolve('output/native-e2e', randomUUID());
await mkdir(root, { recursive: true });
const server = createServer();
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const port = server.address().port;
await new Promise((resolve) => server.close(resolve));
const manifest =
  process.env.DOWNPOUR_ENGINE_MANIFEST ??
  join(
    homedir(),
    'Library',
    'Application Support',
    'app.downpour.desktop',
    'engine.json',
  );
const binary =
  process.env.DOWNPOUR_TEST_BINARY ??
  resolve(
    'target/debug/downpour-desktop' +
      (process.platform === 'win32' ? '.exe' : ''),
  );
let logs = '';
function startApp() {
  const child = spawn(binary, [], {
    env: {
      ...process.env,
      TAURI_WEBDRIVER_PORT: String(port),
      DOWNPOUR_ENGINE_MANIFEST: manifest,
      DOWNPOUR_TEST_OUTPUT: join(root, 'downloads'),
      DOWNPOUR_TEST_DATA: join(root, 'data'),
      DOWNPOUR_TEST_RATE: '512K',
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  child.stdout.on('data', (bytes) => {
    logs = (logs + bytes).slice(-64000);
  });
  child.stderr.on('data', (bytes) => {
    logs = (logs + bytes).slice(-64000);
  });
  return child;
}
const startedAt = Date.now();
let child = startApp();
const base = `http://127.0.0.1:${port}`;
async function request(path, method = 'GET', data) {
  const response = await fetch(base + path, {
    method,
    headers: { 'Content-Type': 'application/json' },
    body: data === undefined ? undefined : JSON.stringify(data),
    signal: AbortSignal.timeout(15000),
  });
  const result = await response.json();
  if (!response.ok || result.value?.error)
    throw new Error(JSON.stringify(result.value));
  return result.value;
}
async function until(task, timeout = 15000) {
  const start = Date.now();
  let error;
  while (Date.now() - start < timeout) {
    if (child.exitCode !== null)
      throw new Error(`Native test app exited: ${logs}`);
    try {
      const result = await task();
      if (result) return result;
    } catch (cause) {
      error = cause;
    }
    await new Promise((resolve) => setTimeout(resolve, 150));
  }
  throw new Error(
    `Native UI wait timed out: ${error?.message ?? 'condition not met'}`,
  );
}
let session;
try {
  await until(async () => {
    await request('/status');
    return true;
  }, 30000);
  const created = await request('/session', 'POST', {
    capabilities: { alwaysMatch: {} },
  });
  session = created.sessionId;
  assert.ok(session, 'driver returned a session');
  const command = (path, data, method = 'POST') =>
    request(`/session/${session}${path}`, method, data);
  const execute = (script, args = []) =>
    command('/execute/sync', { script, args });
  const element = async (selector) => {
    const value = await command('/element', {
      using: 'css selector',
      value: selector,
    });
    return value['element-6066-11e4-a52e-4f735466cecf'];
  };
  const click = async (selector) =>
    command(`/element/${await element(selector)}/click`, {});
  await until(
    () =>
      execute(
        'return !!document.querySelector("#video-url") && !document.querySelector("button[type=submit]").disabled && document.querySelector(".preview-pill").textContent.trim() === "LOCAL DOWNLOADS"',
      ),
    60000,
  );
  const startupReadyMs = Date.now() - startedAt;
  assert.equal(
    await execute(
      'return document.querySelector(".preview-pill").textContent.trim()',
    ),
    'LOCAL DOWNLOADS',
  );
  await click('.text-button');
  await click('button[type=submit]');
  await until(
    () => execute('return !!document.querySelector("[role=dialog]")'),
    90000,
  );
  const title = await execute(
    'return document.querySelector(".preview-title").textContent',
  );
  assert.ok(title.includes('sintel'), 'real metadata is displayed');
  assert.equal(
    await execute(
      'return document.querySelector("[data-testid=enqueue]").textContent.trim()',
    ),
    'Download video',
  );
  await click('[data-testid=enqueue]');
  await until(
    () =>
      execute(
        'return [...document.querySelectorAll(".job-status")].some(node => /[1-9][0-9]*%/.test(node.textContent))',
      ),
    90000,
  );
  await until(
    () =>
      execute(
        'return [...document.querySelectorAll(".job-transfer strong")].some(node => /[0-9].* (KB|MB|B)\\/s/.test(node.textContent))',
      ),
    30000,
  );
  const { liveSpeed, totalSpeed } = await execute(
    'return {liveSpeed: document.querySelector(".job-transfer strong").textContent, totalSpeed: document.querySelector(".queue-speed strong").textContent}',
  );
  assert.equal(
    totalSpeed,
    liveSpeed,
    'total speed matches the single active real download',
  );
  const speedScreenshot = await command('/screenshot', undefined, 'GET');
  await writeFile(
    join(root, 'download-speed.png'),
    Buffer.from(speedScreenshot, 'base64'),
  );
  await click('button[aria-label^="Cancel "]');
  await until(() =>
    execute(
      'return [...document.querySelectorAll(".job-status")].some(node => node.textContent.trim() === "Cancelled")',
    ),
  );
  assert.equal(
    await execute('return !!document.querySelector(".job-transfer")'),
    false,
    'cancel clears the live rate',
  );
  assert.equal(
    await execute(
      'return document.querySelector(".queue-speed strong").textContent',
    ),
    '0 B/s',
  );
  const partialDirectories = await readdir(join(root, 'downloads', '.partial'));
  const partial = join(
    root,
    'downloads',
    '.partial',
    partialDirectories[0],
    'stream-0.download.part',
  );
  const partialBytes = (await stat(partial)).size;
  assert.ok(partialBytes > 0, 'cancel preserves resumable partial data');
  await click('button[aria-label^="Retry "]');
  await until(
    () =>
      execute(
        'return [...document.querySelectorAll(".job-status")].some(node => node.textContent.trim() === "Saved")',
      ),
    180000,
  );
  await click('a[aria-label=Library]');
  await until(() =>
    execute('return !!document.querySelector(".library-card .output-actions")'),
  );
  await click('button[aria-label="Show saved file in folder"]');
  await until(() =>
    execute(
      'return !document.querySelector("button[aria-label=\\"Show saved file in folder\\"]").disabled',
    ),
  );
  assert.equal(
    await execute('return !!document.querySelector(".app-error")'),
    false,
    'native reveal command succeeded',
  );
  const files = await import('node:fs/promises').then((fs) =>
    fs.readdir(join(root, 'downloads')),
  );
  const saved = files.find((name) => name.endsWith('.mp4'));
  assert.ok(saved);
  const hash = createHash('sha256');
  for await (const bytes of createReadStream(join(root, 'downloads', saved)))
    hash.update(bytes);
  const checksum = hash.digest('hex');
  await until(() =>
    execute(
      'return Number(getComputedStyle(document.querySelector(".page")).opacity) > 0.99',
    ),
  );
  const screenshot = await command('/screenshot', undefined, 'GET');
  await writeFile(join(root, 'library.png'), Buffer.from(screenshot, 'base64'));
  await click('a[aria-label=Settings]');
  await until(() =>
    execute('return !!document.querySelector(".settings-page")'),
  );
  await execute(
    'document.querySelector("input[value=light]").click(); return true',
  );
  await click('.settings-actions > button');
  await until(() =>
    execute('return document.documentElement.dataset.theme === "light"'),
  );
  const origin = await execute('return performance.timeOrigin');
  await command('/refresh', {});
  await until(() =>
    execute(
      'return performance.timeOrigin !== arguments[0] && document.documentElement.dataset.theme === "light" && !!document.querySelector(".settings-page")',
      [origin],
    ),
  );
  await click('a[aria-label=Library]');
  await until(() =>
    execute('return !!document.querySelector(".library-card .output-actions")'),
  );
  await request(`/session/${session}`, 'DELETE');
  session = undefined;
  const stopped = new Promise((resolve) => child.once('exit', resolve));
  child.kill('SIGTERM');
  await stopped;
  child = startApp();
  await until(async () => {
    await request('/status');
    return true;
  }, 30000);
  const restarted = await request('/session', 'POST', {
    capabilities: { alwaysMatch: {} },
  });
  session = restarted.sessionId;
  await until(
    () =>
      execute(
        'return document.documentElement.dataset.theme === "light" && !!document.querySelector("#video-url")',
      ),
    60000,
  );
  await click('a[aria-label=Library]');
  await until(() =>
    execute('return !!document.querySelector(".library-card .output-actions")'),
  );
  const report = {
    passed: true,
    startupReadyMs,
    title,
    liveSpeed,
    output: saved,
    sha256: checksum,
    partialBytesBeforeRetry: partialBytes,
    checks: [
      'real native metadata',
      'format selection',
      'real cancellation/partial resume/retry',
      'queue/progress/completion',
      'live engine download speed, total speed and cancellation reset',
      'verified saved file',
      'native reveal action with verified job ownership',
      'preferences and history across reload and application restart',
    ],
    appData: join(root, 'data'),
  };
  await writeFile(join(root, 'report.json'), JSON.stringify(report, null, 2));
  console.log(`Native WKWebView flow passed. Evidence: ${root}`);
  console.log(JSON.stringify(report, null, 2));
} finally {
  if (session) {
    try {
      await request(`/session/${session}`, 'DELETE');
    } catch {
      /* app may already be closing */
    }
  }
  child.kill('SIGTERM');
  await writeFile(join(root, 'native.log'), logs);
}
