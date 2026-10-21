import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { mkdir, readFile, realpath, writeFile } from 'node:fs/promises';
import { join, resolve, relative, isAbsolute } from 'node:path';
import { spawn } from 'node:child_process';

const target = `${process.arch === 'arm64' ? 'aarch64' : 'x86_64'}-${process.platform === 'darwin' ? 'apple-darwin' : process.platform === 'win32' ? 'pc-windows-msvc' : 'unknown-linux-gnu'}`;
const directory = await realpath(resolve('engines', target));
const manifest = JSON.parse(
  await readFile(join(directory, 'engine.json'), 'utf8'),
);
if (
  manifest.target !== target ||
  manifest.distribution !== 'portable' ||
  manifest.schemaVersion !== 1
)
  throw new Error('A matching portable engine package is required.');
for (const name of ['yt-dlp', 'ffmpeg', 'ffprobe', 'deno']) {
  const tool = manifest.tools[name];
  if (
    !tool ||
    isAbsolute(tool.path) ||
    tool.path.split(/[\\/]/).some((part) => ['..', '.', ''].includes(part))
  )
    throw new Error(`Invalid packaged path: ${name}`);
  const path = await realpath(join(directory, tool.path));
  if (
    relative(directory, path).startsWith('..') ||
    isAbsolute(relative(directory, path))
  )
    throw new Error('Packaged tool escaped its directory.');
  const hash = createHash('sha256');
  for await (const bytes of createReadStream(path)) hash.update(bytes);
  if (hash.digest('hex') !== tool.sha256)
    throw new Error(`Packaged tool checksum mismatch: ${name}`);
}
await readFile(join(directory, 'THIRD_PARTY_NOTICES.md'));
await readFile(join(directory, 'notices', 'engines.lock.json'));
const config = resolve('output/release/tauri.portable.conf.json');
await mkdir(resolve('output/release'), { recursive: true });
await writeFile(
  config,
  JSON.stringify(
    { bundle: { resources: { [directory + '/']: `engines/${target}/` } } },
    null,
    2,
  ),
);
const bundles =
  process.platform === 'darwin'
    ? 'app'
    : process.platform === 'win32'
      ? 'nsis'
      : 'deb';
const child = spawn(
  process.execPath,
  ['scripts/desktop.mjs', 'build', '--bundles', bundles, '--config', config],
  { shell: false, stdio: 'inherit' },
);
child.on('error', (error) => {
  console.error(error.message);
  process.exitCode = 1;
});
child.on('exit', (code) => {
  process.exitCode = code ?? 1;
});
