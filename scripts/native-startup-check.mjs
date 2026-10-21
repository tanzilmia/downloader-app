import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { join, resolve, dirname, isAbsolute } from 'node:path';

// The debug-only driver uses isolated data; user files are never modified.
const source = process.env.DOWNPOUR_ENGINE_MANIFEST;
if (!source)
  throw new Error(
    'Set DOWNPOUR_ENGINE_MANIFEST to a verified real engine manifest.',
  );
const root = resolve('output/native-e2e', `startup-${randomUUID()}`);
await mkdir(root, { recursive: true });
const manifest = join(root, 'engine.json');
const binary =
  process.env.DOWNPOUR_TEST_BINARY ??
  resolve(
    'target/debug/downpour-desktop' +
      (process.platform === 'win32' ? '.exe' : ''),
  );
const server = createServer();
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const port = server.address().port;
await new Promise((resolve) => server.close(resolve));
let logs = '';
let child;
function start() {
  child = spawn(binary, [], {
    env: {
      ...process.env,
      TAURI_WEBDRIVER_PORT: String(port),
      DOWNPOUR_ENGINE_MANIFEST: manifest,
      DOWNPOUR_TEST_DATA: join(root, 'data'),
      DOWNPOUR_TEST_OUTPUT: join(root, 'downloads'),
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  child.stdout.on('data', (bytes) => {
    logs = (logs + bytes).slice(-64000);
  });
  child.stderr.on('data', (bytes) => {
    logs = (logs + bytes).slice(-64000);
  });
}
async function request(path, data, method = 'POST') {
  const response = await fetch(`http://127.0.0.1:${port}${path}`, {
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
  const deadline = Date.now() + timeout;
  let error;
  while (Date.now() < deadline) {
    if (child.exitCode !== null || child.signalCode !== null)
      throw new Error(`App exited: ${logs}`);
    try {
      const value = await task();
      if (value) return value;
    } catch (cause) {
      error = cause;
    }
    await new Promise((resolve) => setTimeout(resolve, 120));
  }
  throw new Error(
    `Native startup check timed out: ${error?.message ?? 'condition not met'}`,
  );
}
let session;
const command = (path, data, method) =>
  request(`/session/${session}${path}`, data, method);
const execute = (script, args = []) =>
  command('/execute/sync', { script, args });
async function click(selector) {
  const element = await command('/element', {
    using: 'css selector',
    value: selector,
  });
  await command(
    `/element/${element['element-6066-11e4-a52e-4f735466cecf']}/click`,
    {},
  );
}
async function connect() {
  await until(async () => {
    await request('/status', undefined, 'GET');
    return true;
  }, 30000);
  session = (await request('/session', { capabilities: { alwaysMatch: {} } }))
    .sessionId;
  await until(() =>
    execute(
      'return !!document.querySelector("#video-url") && document.querySelector(".local-card").textContent.includes("SQLite connected")',
    ),
  );
}
async function stop() {
  if (session) {
    await request(`/session/${session}`, undefined, 'DELETE');
    session = undefined;
  }
  if (child.exitCode === null && child.signalCode === null) {
    const exited = new Promise((resolve) => child.once('exit', resolve));
    child.kill('SIGTERM');
    await exited;
  }
}
start();
try {
  await connect();
  assert.equal(
    await execute(
      'return document.querySelector("button[type=submit]").disabled',
    ),
    false,
  );
  await click('.text-button');
  await click('button[type=submit]');
  await until(() =>
    execute(
      'return !!document.querySelector("#url-error") && !document.querySelector("button[type=submit]").disabled',
    ),
  );
  assert.match(
    await execute('return document.querySelector("#url-error").textContent'),
    /engine|installed|manifest/i,
  );

  // Repair only the isolated manifest, referencing verified existing tools.
  const repaired = JSON.parse(await readFile(source, 'utf8'));
  for (const tool of Object.values(repaired.tools))
    if (!isAbsolute(tool.path)) tool.path = resolve(dirname(source), tool.path);
  repaired.distribution = 'development';
  await writeFile(manifest, JSON.stringify(repaired, null, 2));
  const origin = await execute('return performance.timeOrigin');
  await click('.app-error button');
  await until(() =>
    execute(
      'return performance.timeOrigin !== arguments[0] && document.querySelector(".local-card")?.textContent.includes("SQLite connected") && !document.querySelector(".app-error") && !document.querySelector("button[type=submit]").disabled',
      [origin],
    ),
  );
  await stop();
  const began = Date.now();
  start();
  await connect();
  const startupReadyMs = Date.now() - began;
  assert.equal(
    await execute(
      'return document.querySelector("button[type=submit]").disabled',
    ),
    false,
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
  assert.match(title, /sintel/i);
  await click('button[aria-label="Close preview"]');
  await until(() =>
    execute(
      'return !document.querySelector("[role=dialog]") && !document.querySelector("button[type=submit]").disabled && Number(getComputedStyle(document.querySelector(".page")).opacity) > .99',
    ),
  );
  const screenshot = await command('/screenshot', undefined, 'GET');
  await writeFile(
    join(root, 'find-enabled.png'),
    Buffer.from(screenshot, 'base64'),
  );
  const report = {
    passed: true,
    startupReadyMs,
    title,
    checks: [
      'Find remains clickable when tools are unavailable',
      'dependency failure appears beside the link and button recovers',
      'Reconnect repairs tools without restarting the manager',
      'healthy restart enables Find',
      'Find retrieves actual public metadata',
    ],
  };
  await writeFile(join(root, 'report.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify({ ...report, evidence: root }, null, 2));
} finally {
  try {
    await stop();
  } finally {
    await writeFile(join(root, 'native.log'), logs);
  }
}
