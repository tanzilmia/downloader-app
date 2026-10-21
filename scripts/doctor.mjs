import { spawnSync } from 'node:child_process';
import { homedir, platform, arch } from 'node:os';
import { delimiter, join } from 'node:path';

const env = {
  ...process.env,
  PATH: `${join(homedir(), '.cargo', 'bin')}${delimiter}${process.env.PATH ?? ''}`,
};
const checks = [
  ['node', ['--version'], true],
  ['pnpm', ['--version'], true],
  ['rustc', ['--version'], true],
  ['cargo', ['--version'], true],
  ['yt-dlp', ['--version'], false],
  ['ffmpeg', ['-version'], false],
  ['ffprobe', ['-version'], false],
  ['deno', ['--version'], false],
];
if (platform() === 'darwin') checks.push(['xcode-select', ['-p'], true]);
console.log(`Downpour environment / ${platform()} ${arch()}`);
for (const [command, args, required] of checks) {
  const windowsPnpm = platform() === 'win32' && command === 'pnpm';
  const result = spawnSync(
    windowsPnpm ? 'cmd.exe' : command,
    windowsPnpm ? ['/d', '/s', '/c', 'pnpm --version'] : args,
    {
      env,
      encoding: 'utf8',
      timeout: 15_000,
      shell: false,
    },
  );
  const ok = !result.error && result.status === 0;
  console.log(
    `${ok ? 'OK' : required ? 'MISSING' : 'OPTIONAL'} ${command}: ${ok ? result.stdout.split('\n')[0] : (result.error?.message ?? result.stderr.trim())}`,
  );
  if (!ok && required) process.exitCode = 1;
}
console.log(
  'Media tools are optional for the mock build. Live site compatibility and portable packaging require separate verification.',
);
