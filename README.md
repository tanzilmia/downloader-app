# Downpour

A lightweight, local-first desktop video downloader built with Tauri 2, React and
Rust. **This is the Phase 1 foundation: metadata and downloads are simulated. No
real media files are downloaded or saved.**

The first vertical slice includes Home, media preview and format selection,
Downloads, Library and Settings, with light/dark/system appearance and smaller
window layouts. There is no remote application backend, account or telemetry.

## Run it

Prerequisites: Node 22.13+ (22.x) or 24+, pnpm 11.0.5, a current stable Rust toolchain and the
[Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/).
On macOS, Xcode Command Line Tools are sufficient for this desktop build.

```sh
pnpm install --frozen-lockfile
pnpm dev              # Browser preview: http://127.0.0.1:1420
pnpm desktop:dev      # Native Tauri development app
pnpm doctor           # Environment and optional media-tool diagnostics
```

Click **Try a sample link → Find video**, choose a format, then **Add mock
download**. Follow its progress in Downloads; cancel/retry or find the completed
simulation in Library. Preferences and queue history survive restart.

Desktop persists to `downpour.db` in Tauri's platform app-data directory. On this
Mac that is `~/Library/Application Support/app.downpour.desktop/`. SQLite and its
embedded migrations initialize automatically. Interrupted attempts requeue on
startup. Browser preview uses separate, versioned localStorage and never accesses
the desktop database. Browser preview supports a single tab at a time.

Rust was installed on this development machine without modifying shell profiles.
The desktop launcher adds `~/.cargo/bin` to its child environment. For direct Cargo
commands, first run `source ~/.cargo/env` if Cargo is not already on PATH.

## Verify and build

```sh
pnpm check                         # Format, lint, types, unit tests and web build
pnpm exec playwright install chromium
pnpm test:e2e                      # Mock flow, persistence and axe accessibility
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace             # Domain, SQLite, queue and Tauri IPC/ACL tests
pnpm desktop:build                 # Unsigned macOS .app for local validation
```

The macOS bundle is at `target/release/bundle/macos/Downpour.app`. On Windows/Linux,
use `node scripts/desktop.mjs build --no-bundle` to compile the native application;
installer packaging is a later release gate. This repository has not yet been
validated on Windows/Linux hosts. Native release builds require their respective
platform dependencies, binary audits and signing setup.

## Workspace

| Path                     | Responsibility                                                 |
| ------------------------ | -------------------------------------------------------------- |
| `apps/desktop/src`       | React screens, Zustand state, typed bridge, browser mock       |
| `apps/desktop/src-tauri` | Thin native commands, application composition, ACL/CSP         |
| `crates/download-core`   | Domain, replaceable ports, bounded queue, SQLx repository      |
| `packages/contracts`     | Zod contracts, TypeScript types, shared Rust/TS fixtures       |
| `tests/e2e`              | Browser workflow and accessibility checks                      |
| `docs`                   | Architecture, decisions, risks, plan and verification evidence |

Read [architecture](docs/architecture.md), [acceptance plan](docs/plan.md),
[risk register](docs/risks.md), [progress](docs/progress.md), and
[project rules](AGENTS.md) before extending the foundation.

## Next gates

Real yt-dlp extraction, FFmpeg processing, safe partial files/finalization, binary
bundling, live platform compatibility and mobile adapters are not implemented.
The desktop core is transport independent so those adapters can be added without
rewriting UI state or persistence. Download only media you are authorized to keep;
DRM/access-control bypass and browser session collection are outside scope.

No media sidecars are bundled. Homebrew tools are development probes only; do not
redistribute them as portable binaries. Application source licensing, third-party
notices, signing/notarization and clean-host installers must be resolved before
release. This foundation is not a production release.
