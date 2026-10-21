import { spawn } from 'node:child_process';
import { homedir } from 'node:os';
import { delimiter, join } from 'node:path';
import { fileURLToPath } from 'node:url';

// rustup was installed without changing the user's shell profile.
const desktopDirectory = fileURLToPath(
  new URL('../apps/desktop/', import.meta.url),
);
const cli = join(
  desktopDirectory,
  'node_modules',
  '@tauri-apps',
  'cli',
  'tauri.js',
);
const child = spawn(process.execPath, [cli, ...process.argv.slice(2)], {
  cwd: desktopDirectory,
  stdio: 'inherit',
  env: {
    ...process.env,
    PATH: `${join(homedir(), '.cargo', 'bin')}${delimiter}${process.env.PATH ?? ''}`,
  },
  shell: false,
});
child.on('error', (error) => {
  console.error(error.message);
  process.exitCode = 1;
});
child.on('exit', (code) => {
  process.exitCode = code ?? 1;
});
