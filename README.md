# Downpour

A local-first desktop downloader using Tauri 2, React/TypeScript and Rust.
**Native desktop now downloads real media** with yt-dlp, local FFmpeg processing,
FFprobe verification and SHA-256 checks. Browser preview stays explicitly simulated.
No account, cloud application backend, telemetry or remote UI assets.

## Run

Use Node 22.13+ (22.x) or 24+, pnpm 11.0.5, stable Rust and the
[Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/).

```sh
pnpm install --frozen-lockfile
pnpm dev                 # Browser preview, simulated files
pnpm engine:setup        # Record installed yt-dlp/ffmpeg/ffprobe/deno hashes
pnpm desktop:dev         # Native app, real downloads
```

Developer setup requires all four tools on PATH and writes a machine-specific
manifest. Tools are verified again before execution; updating one requires setup
again. Missing/changed tools produce a dependency error. Set
`DOWNPOUR_ENGINE_MANIFEST` before startup to use another verified manifest.

Choose **Try a sample link → Find video → Download video**. The desktop sample is
Blender's public Sintel trailer. Preview shows actual formats; Downloads supports
progress/cancel/retry; Library offers Open/Show in folder for verified output.
Old simulations keep their labels. Only download media you are authorized to keep.
Sign-in/session collection, DRM bypass, live streams and playlists are outside scope.

Files go to the OS Downloads directory under `Downpour`; resumable partials remain
in `.partial/<job-id>`. No automatic partial-retention policy yet. SQLite history
and preferences use the platform app-data directory, on this Mac at
`~/Library/Application Support/app.downpour.desktop/`. Interrupted work requeues
on restart. Browser localStorage is separate and supports one preview tab.

## Verify and package

```sh
pnpm check
pnpm exec playwright install chromium
pnpm test:e2e
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm test:native:build
pnpm test:native          # Opt-in real native flow; tools/network required
pnpm test:native:startup  # Missing tools, reconnect, clickable Find and real preview
pnpm test:native:rate-limit # Controlled 429 fixture; countdown and request suppression
pnpm engine:portable     # Native macOS/Linux, official pinned assets/source
pnpm desktop:bundle      # Portable engine included, unsigned local candidate
```

The macOS candidate is `target/release/bundle/macos/Downpour.app`, approximately
154 MiB including its engine/source notices. `pnpm desktop:build` builds a smaller
developer app using the installed manifest. The launcher adds `~/.cargo/bin`
without modifying profiles; direct Cargo commands may need `source ~/.cargo/env`.
Native automation is a debug-only feature that cannot compile into release mode.

| Path                   | Responsibility                                       |
| ---------------------- | ---------------------------------------------------- |
| `apps/desktop`         | React UI, typed bridge, thin Tauri commands, ACL/CSP |
| `crates/download-core` | Independent domain, ports, queue, SQLite             |
| `crates/media-engine`  | Real adapter, network policy, processes/files/tools  |
| `packages/contracts`   | Zod contracts and shared mock/real fixtures          |

See [evidence](docs/progress.md), [architecture](docs/architecture.md),
[security review](docs/security-review.md), [release gates](docs/release.md),
[risks](docs/risks.md) and [project rules](AGENTS.md). CI is configured for three
OSes; only this macOS host has actually been verified. This is an unsigned local
candidate, with signing, complete redistribution qualification and other OS
installers still pending. Real progressive HTTP is verified; an earlier YouTube
probe returned 429. Facebook/Instagram/TikTok live compatibility is unqualified.
Mobile needs separate extraction/lifecycle adapters around the reusable core.
