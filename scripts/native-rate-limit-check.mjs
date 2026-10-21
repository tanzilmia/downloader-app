import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { chmod, mkdir, readFile, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { dirname, isAbsolute, join, resolve } from 'node:path';

// Controlled local 429 fixture. No YouTube traffic or production data is used.
if (process.platform === 'win32')
  throw new Error('This fixture requires a POSIX host.');
const source = process.env.DOWNPOUR_ENGINE_MANIFEST;
if (!source)
  throw new Error(
    'Set DOWNPOUR_ENGINE_MANIFEST to a verified engine manifest.',
  );
const root = resolve('output/native-e2e', `rate-limit-${randomUUID()}`);
await mkdir(root, { recursive: true });
const manifest = JSON.parse(await readFile(source, 'utf8'));
for (const tool of Object.values(manifest.tools))
  if (!isAbsolute(tool.path)) tool.path = resolve(dirname(source), tool.path);
const toolPath = join(root, 'yt-dlp-fixture');
const script =
  "#!/bin/sh\nprintf x >> calls.txt\nprintf '%s\\n' 'ERROR: [youtube] fixture: HTTP Error 429: Too Many Requests' >&2\nexit 1\n";
await writeFile(toolPath, script);
await chmod(toolPath, 0o700);
manifest.distribution = 'development';
manifest.tools['yt-dlp'] = {
  path: toolPath,
  version: 'controlled-429-fixture',
  sha256: createHash('sha256').update(script).digest('hex'),
};
const manifestPath = join(root, 'engine.json');
await writeFile(manifestPath, JSON.stringify(manifest, null, 2));
const server = createServer();
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const port = server.address().port;
await new Promise((resolve) => server.close(resolve));
let logs = '';
const child = spawn(
  process.env.DOWNPOUR_TEST_BINARY ?? resolve('target/debug/downpour-desktop'),
  [],
  {
    env: {
      ...process.env,
      TAURI_WEBDRIVER_PORT: String(port),
      DOWNPOUR_ENGINE_MANIFEST: manifestPath,
      DOWNPOUR_TEST_DATA: join(root, 'data'),
      DOWNPOUR_TEST_OUTPUT: join(root, 'downloads'),
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  },
);
for (const pipe of [child.stdout, child.stderr])
  pipe.on('data', (bytes) => {
    logs = (logs + bytes).slice(-64000);
  });
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
async function until(task, timeout = 30000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    if (child.exitCode !== null || child.signalCode !== null)
      throw new Error(`App exited: ${logs}`);
    try {
      const result = await task();
      if (result) return result;
    } catch (error) {
      last = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 120));
  }
  throw new Error(
    `Native rate-limit wait timed out: ${last?.message ?? 'condition not met'}`,
  );
}
let session;
const command = (path, data, method) =>
  request(`/session/${session}${path}`, data, method);
const execute = (script, args = []) =>
  command('/execute/sync', { script, args });
async function element(selector) {
  const result = await command('/element', {
    using: 'css selector',
    value: selector,
  });
  return result['element-6066-11e4-a52e-4f735466cecf'];
}
async function click(selector) {
  await command(`/element/${await element(selector)}/click`, {});
}
async function fill(url) {
  const input = await element('#video-url');
  await command(`/element/${input}/clear`, {});
  await command(`/element/${input}/value`, { text: url });
  await until(() =>
    execute(
      'return document.querySelector("#video-url").value === arguments[0]',
      [url],
    ),
  );
}
try {
  await until(async () => {
    await request('/status', undefined, 'GET');
    return true;
  });
  session = (await request('/session', { capabilities: { alwaysMatch: {} } }))
    .sessionId;
  await until(() =>
    execute(
      'return !!document.querySelector("#video-url") && document.querySelector(".local-card").textContent.includes("SQLite connected")',
    ),
  );
  await fill('https://youtube.com/shorts/D_8JUUup5js');
  await click('button[type=submit]');
  await until(() =>
    execute(
      'return document.querySelector("button[type=submit]").disabled && !!document.querySelector("#youtube-pause") && document.querySelector("#url-error").textContent.includes("HTTP 429")',
    ),
  );
  const firstLabel = await execute(
    'return document.querySelector("button[type=submit]").textContent',
  );
  assert.match(firstLabel, /Retry in (1:00|0:\d{2})/);
  await until(() =>
    execute(
      'return document.querySelector("button[type=submit]").textContent !== arguments[0]',
      [firstLabel],
    ),
  );
  // Advance only this test webview's wall clock to exercise timer expiry.
  // Rust uses a monotonic clock and must continue to enforce its real pause.
  await execute(
    'const clock = Date.now.bind(Date); Date.now = () => clock() + 61000; return true',
  );
  await until(() =>
    execute(
      'return !document.querySelector("button[type=submit]").disabled && !document.querySelector("#youtube-pause")',
    ),
  );
  await click('button[type=submit]');
  await until(() =>
    execute(
      'return !!document.querySelector("#youtube-pause") && document.querySelector("button[type=submit]").disabled',
    ),
  );
  const screenshot = await command('/screenshot', undefined, 'GET');
  await writeFile(
    join(root, 'youtube-pause.png'),
    Buffer.from(screenshot, 'base64'),
  );
  assert.equal(await readFile(join(root, 'downloads/calls.txt'), 'utf8'), 'x');
  await click('.text-button');
  assert.equal(
    await execute(
      'return document.querySelector("button[type=submit]").disabled',
    ),
    false,
    'other sources remain usable',
  );
  assert.equal(
    await execute('return !!document.querySelector("#youtube-pause")'),
    false,
  );

  // Reload removes the UI timer; Rust still blocks a different YouTube alias.
  const origin = await execute('return performance.timeOrigin');
  await command('/refresh', {});
  await until(() =>
    execute(
      'return performance.timeOrigin !== arguments[0] && !!document.querySelector("#video-url") && document.querySelector(".local-card").textContent.includes("SQLite connected")',
      [origin],
    ),
  );
  await fill('https://youtu.be/D_8JUUup5js?si=shared');
  await click('button[type=submit]');
  await until(() =>
    execute(
      'return !!document.querySelector("#youtube-pause") && document.querySelector("button[type=submit]").disabled',
    ),
  );
  assert.equal(
    await readFile(join(root, 'downloads/calls.txt'), 'utf8'),
    'x',
    'reload and alias retry did not spawn another tool',
  );
  const report = {
    passed: true,
    controlledFixture: true,
    youtubeRequests: 0,
    extractorLaunches: 1,
    checks: [
      'typed rate-limit error reaches native UI',
      'explicit pause countdown ticks',
      'UI timer expires; backend still prevents an early retry',
      'other source link remains interactive',
      'backend retains pause across webview reload and YouTube aliases',
    ],
    evidence: root,
  };
  await writeFile(join(root, 'report.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify(report, null, 2));
} finally {
  if (session) {
    try {
      await request(`/session/${session}`, undefined, 'DELETE');
    } catch {
      /* already closing */
    }
  }
  child.kill('SIGTERM');
  await writeFile(join(root, 'native.log'), logs);
}
