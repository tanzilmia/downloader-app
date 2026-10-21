import { createHash } from 'node:crypto';
import { createReadStream, existsSync } from 'node:fs';
import { mkdir, realpath, writeFile } from 'node:fs/promises';
import { homedir, platform, arch } from 'node:os';
import { delimiter, dirname, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const target = `${arch() === 'arm64' ? 'aarch64' : 'x86_64'}-${platform() === 'darwin' ? 'apple-darwin' : platform() === 'win32' ? 'pc-windows-msvc' : 'unknown-linux-gnu'}`;
const appData =
  platform() === 'darwin'
    ? join(homedir(), 'Library', 'Application Support', 'app.downpour.desktop')
    : platform() === 'win32'
      ? join(
          process.env.APPDATA ?? join(homedir(), 'AppData', 'Roaming'),
          'app.downpour.desktop',
        )
      : join(
          process.env.XDG_DATA_HOME ?? join(homedir(), '.local', 'share'),
          'app.downpour.desktop',
        );
const manifestPath = process.argv[2] ?? join(appData, 'engine.json');
const tools = {};
for (const name of ['yt-dlp', 'ffmpeg', 'ffprobe', 'deno']) {
  let found;
  for (const directory of (process.env.PATH ?? '').split(delimiter)) {
    const path = join(directory, name + (platform() === 'win32' ? '.exe' : ''));
    if (existsSync(path)) {
      found = await realpath(path);
      break;
    }
  }
  if (!found)
    throw new Error(
      `${name} is missing. Install the required local media tools, then run this setup again.`,
    );
  const check = spawnSync(
    found,
    [name.startsWith('ff') ? '-version' : '--version'],
    { encoding: 'utf8', timeout: 15_000, shell: false },
  );
  if (check.error || check.status !== 0)
    throw new Error(`${name} failed its version check.`);
  const hash = createHash('sha256');
  for await (const bytes of createReadStream(found)) hash.update(bytes);
  tools[name] = {
    path: found,
    sha256: hash.digest('hex'),
    version: check.stdout.split('\n')[0].trim(),
  };
  console.log(`Verified ${name}: ${tools[name].version}`);
}
await mkdir(dirname(resolve(manifestPath)), { recursive: true, mode: 0o700 });
await writeFile(
  manifestPath,
  JSON.stringify(
    { schemaVersion: 1, target, distribution: 'development', tools },
    null,
    2,
  ) + '\n',
  { mode: 0o600 },
);
console.log(`Developer engine manifest saved: ${manifestPath}`);
console.log(
  'This records installed tools on this machine. It is not a portable or redistributable engine package.',
);
