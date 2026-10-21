import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createReadStream, createWriteStream, existsSync } from 'node:fs';
import {
  chmod,
  copyFile,
  mkdir,
  readFile,
  rename,
  rm,
  writeFile,
} from 'node:fs/promises';
import { cpus } from 'node:os';
import { join, resolve } from 'node:path';
import { Readable, Transform } from 'node:stream';
import { pipeline } from 'node:stream/promises';

// Native builds only. Never copy executables out of Homebrew or an unpinned mirror.
const target = `${process.arch === 'arm64' ? 'aarch64' : 'x86_64'}-${process.platform === 'darwin' ? 'apple-darwin' : process.platform === 'win32' ? 'pc-windows-msvc' : 'unknown-linux-gnu'}`;
const lock = JSON.parse(await readFile('engines.lock.json', 'utf8'));
const assets = lock.targets[target];
if (!assets || !['darwin', 'linux'].includes(process.platform))
  throw new Error(
    'Portable source builds currently support native macOS/Linux. Windows packaging remains gated on a verified native FFmpeg build.',
  );
const root = resolve('output/release');
const cache = join(root, 'cache');
const destination = resolve('engines', target);
const bin = join(destination, 'bin');
await mkdir(cache, { recursive: true });
await mkdir(bin, { recursive: true });
function run(file, args, options = {}) {
  const result = spawnSync(file, args, {
    stdio: 'inherit',
    shell: false,
    ...options,
  });
  if (result.error || result.status !== 0)
    throw new Error(
      `${file} failed (${result.status}): ${result.error?.message ?? ''}`,
    );
  return result;
}
async function checksum(path) {
  const hash = createHash('sha256');
  for await (const bytes of createReadStream(path)) hash.update(bytes);
  return hash.digest('hex');
}
async function download(url, name, sha256) {
  const path = join(cache, name);
  if (existsSync(path) && (!sha256 || (await checksum(path)) === sha256))
    return path;
  console.log(`Downloading official source: ${name}`);
  const response = await fetch(url, { signal: AbortSignal.timeout(600000) });
  if (!response.ok || !response.body)
    throw new Error(`Download failed: ${name} (${response.status})`);
  let count = 0;
  const limit = new Transform({
    transform(chunk, _, callback) {
      count += chunk.length;
      callback(
        count > 256 * 1024 * 1024
          ? new Error('Download exceeds size limit')
          : null,
        chunk,
      );
    },
  });
  const temporary = path + '.tmp';
  try {
    await pipeline(
      Readable.fromWeb(response.body),
      limit,
      createWriteStream(temporary),
    );
    if (sha256 && (await checksum(temporary)) !== sha256)
      throw new Error(`Checksum mismatch: ${name}`);
    await rename(temporary, path);
    return path;
  } catch (error) {
    await rm(temporary, { force: true });
    throw error;
  }
}
const yt = await download(
  `https://github.com/yt-dlp/yt-dlp/releases/download/${lock.ytDlpVersion}/${assets.ytDlp.asset}`,
  assets.ytDlp.asset,
  assets.ytDlp.sha256,
);
await copyFile(yt, join(bin, 'yt-dlp'));
await chmod(join(bin, 'yt-dlp'), 0o755);
const deno = await download(
  `https://github.com/denoland/deno/releases/download/v${lock.denoVersion}/${assets.deno.asset}`,
  assets.deno.asset,
  assets.deno.sha256,
);
run('unzip', ['-o', deno, 'deno', '-d', bin]);
await chmod(join(bin, 'deno'), 0o755);
let archive = process.env.DOWNPOUR_FFMPEG_SOURCE_ARCHIVE;
if (archive) {
  if ((await checksum(archive)) !== lock.ffmpeg.sha256)
    throw new Error('FFmpeg source checksum mismatch');
} else
  archive = await download(
    lock.ffmpeg.url,
    `ffmpeg-${lock.ffmpeg.version}.tar.xz`,
    lock.ffmpeg.sha256,
  );
const work = join(root, 'work');
await mkdir(work, { recursive: true });
const source = join(work, `ffmpeg-${lock.ffmpeg.version}`);
const prefix = join(root, 'ffmpeg');
const configureFlags = [
  ...lock.ffmpeg.configure,
  ...(process.platform === 'darwin'
    ? [
        `--extra-cflags=-mmacosx-version-min=${lock.ffmpeg.macosMinimumVersion}`,
        `--extra-ldflags=-mmacosx-version-min=${lock.ffmpeg.macosMinimumVersion}`,
      ]
    : []),
];
const existing = existsSync(join(prefix, 'bin', 'ffmpeg'))
  ? spawnSync(join(prefix, 'bin', 'ffmpeg'), ['-version'], {
      encoding: 'utf8',
      timeout: 15000,
      shell: false,
    })
  : undefined;
const expectedConfiguration = `configuration: --prefix=${prefix} ${configureFlags.join(' ')}`;
if (
  existing?.status !== 0 ||
  !existing.stdout.includes(`ffmpeg version ${lock.ffmpeg.version} `) ||
  !existing.stdout.replaceAll("'", '').includes(expectedConfiguration)
) {
  run('tar', ['-xf', archive, '-C', work]);
  run('./configure', [`--prefix=${prefix}`, ...configureFlags], {
    cwd: source,
  });
  run('make', ['clean'], { cwd: source });
  run('make', ['-j', String(Math.min(cpus().length, 6))], { cwd: source });
  run('make', ['install'], { cwd: source });
}
for (const name of ['ffmpeg', 'ffprobe']) {
  await copyFile(join(prefix, 'bin', name), join(bin, name));
  await chmod(join(bin, name), 0o755);
  if (process.platform === 'darwin') {
    const dependencies = run('otool', ['-L', join(bin, name)], {
      encoding: 'utf8',
      stdio: 'pipe',
    }).stdout;
    for (const line of dependencies.split('\n').slice(1).filter(Boolean)) {
      if (!/^\s+\/(usr\/lib|System\/Library)\//.test(line))
        throw new Error(`Non-system FFmpeg dependency: ${line}`);
    }
  }
}
const notices = join(destination, 'notices');
await mkdir(notices, { recursive: true });
await copyFile(
  archive,
  join(notices, `ffmpeg-${lock.ffmpeg.version}-source.tar.xz`),
);
await copyFile(
  join(source, 'COPYING.LGPLv2.1'),
  join(notices, 'FFmpeg-LGPL-2.1.txt'),
);
await copyFile(join(source, 'LICENSE.md'), join(notices, 'FFmpeg-LICENSE.md'));
await copyFile(
  join(source, 'ffbuild', 'config.log'),
  join(notices, 'FFmpeg-config.log'),
);
await writeFile(
  join(notices, 'FFmpeg-build.json'),
  JSON.stringify({ source: lock.ffmpeg, modified: false, target }, null, 2),
);
for (const [name, url] of [
  [
    'yt-dlp-LICENSE.txt',
    `https://raw.githubusercontent.com/yt-dlp/yt-dlp/${lock.ytDlpVersion}/LICENSE`,
  ],
  [
    'yt-dlp-THIRD_PARTY_LICENSES.txt',
    `https://raw.githubusercontent.com/yt-dlp/yt-dlp/${lock.ytDlpVersion}/THIRD_PARTY_LICENSES.txt`,
  ],
  [
    'Deno-LICENSE.md',
    `https://raw.githubusercontent.com/denoland/deno/v${lock.denoVersion}/LICENSE.md`,
  ],
  [
    'yt-dlp-README.md',
    `https://raw.githubusercontent.com/yt-dlp/yt-dlp/${lock.ytDlpVersion}/README.md`,
  ],
])
  await copyFile(
    await download(
      url,
      `${name.startsWith('Deno') ? lock.denoVersion : lock.ytDlpVersion}-${name}`,
    ),
    join(notices, name),
  );
await writeFile(
  join(destination, 'THIRD_PARTY_NOTICES.md'),
  `Downpour engine for ${target}\n\nFFmpeg ${lock.ffmpeg.version}: LGPL 2.1 or later, separate executable; source, exact configuration and license are in notices/. No source modifications. Downpour does not link to libav*.\n\nyt-dlp ${lock.ytDlpVersion}: source Unlicense; this PyInstaller executable is GPLv3+ as a combined work. See its complete upstream third-party notices and README in notices/. Deno ${lock.denoVersion}: MIT plus bundled dependency licenses.\n\nProvenance and pinned asset digests: engines.lock.json. This is a local unsigned candidate. Public redistribution requires the complete corresponding sources/notices for packaged runtimes, application licensing, signing and native platform acceptance.\n`,
);
await copyFile('engines.lock.json', join(notices, 'engines.lock.json'));
const tools = {};
for (const name of ['yt-dlp', 'ffmpeg', 'ffprobe', 'deno']) {
  const result = run(
    join(bin, name),
    [name.startsWith('ff') ? '-version' : '--version'],
    {
      encoding: 'utf8',
      stdio: 'pipe',
      timeout: name === 'yt-dlp' ? 45000 : 15000,
    },
  );
  tools[name] = {
    path: `bin/${name}`,
    sha256: await checksum(join(bin, name)),
    version: result.stdout.split('\n')[0].trim(),
  };
}
await writeFile(
  join(destination, 'engine.json'),
  JSON.stringify(
    { schemaVersion: 1, target, distribution: 'portable', tools },
    null,
    2,
  ) + '\n',
);
console.log(`Verified portable engine staged: ${destination}`);
