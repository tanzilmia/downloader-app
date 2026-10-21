# Downpour architecture

Status: Phase 0 design accepted from the supplied project brief; Phase 1 vertical
slice in development. Evidence and remaining limits live in `docs/progress.md`.

## Decisions and alternatives

Use a Rust workspace inside a pnpm monorepo. The React application owns views,
transient presentation state and theme. Thin Tauri commands validate inputs and
delegate to a platform-independent Rust core. SQLite is the desktop source of
truth. A typed event carries committed job snapshots; UI reconciles events by
revision and re-fetches the queue after subscribing.

A frontend-only queue would be quicker but loses ownership when a webview closes.
A local HTTP service adds authentication, ports and lifecycle management without
benefit here. The chosen in-process core avoids both and keeps transport replaceable.

## Structure

```text
apps/desktop/
  src/{components,lib,pages,state}/  React screens and typed bridge
  src-tauri/{src,capabilities}/     Native composition and commands
packages/contracts/                Zod schemas, TS types, shared wire fixture
crates/download-core/
  src/{domain,extractor,engine,repository,manager,files}.rs
  migrations/                      Embedded SQLx migrations
docs/{adr,architecture.md,plan.md,risks.md,progress.md}
scripts/                           Reproducible environment/contract checks
```

## Flow and ownership

1. User submits an authorized public URL. Zod and Rust reject malformed URLs,
   credentials, IP literals and local/reserved DNS names. Phase 1 makes no request.
2. `Extractor` inspects media and returns format IDs. The mock adapter supplies
   deterministic metadata. Enqueue re-inspects and validates the selected format;
   frontend titles and filenames are never trusted as authority.
3. Manager inserts a queued job. Scheduler claims FIFO jobs within the configured
   concurrency bound (1–4, default 2). Each worker gets a cancellation token.
4. `TransferEngine` reports progress through a bounded channel. Manager persists
   updates and increments revision before publishing `job:updated` events.
5. Terminal jobs appear in Library. Mock jobs have `isMock: true` and no output
   path. Completion represents a simulated transfer, never a real saved file.

Core state transitions: queued → downloading/cancelled; downloading →
completed/failed/cancelled/queued (startup recovery only); failed/cancelled →
queued (explicit retry). Completed is terminal. Progress is monotonic during an
attempt. A manager mutex serializes transitions; worker handles remain counted
until the worker exits, including cancelled workers.

## Persistence and recovery

Create an app-data directory through Tauri's path resolver. SQLx opens SQLite with
`create_if_missing`, WAL, foreign keys and busy timeout, then runs embedded
migrations. Job and settings payloads are versioned by database migrations. Job
status is indexed and constrained; revisions allow the UI to ignore stale events.
Recovery requeues interrupted downloads at startup before starting scheduling.
Errors opening/migrating the database fail startup with an actionable error;
storage must never be silently replaced.

The browser development adapter uses the same validated contracts and a separate
versioned localStorage namespace. It is a preview convenience, not the desktop
persistence implementation. Failed storage writes are reported, not disguised as
durable success. It also requeues interrupted jobs and enforces concurrency.

## Security boundaries

No remote backend, analytics, remote fonts, remote thumbnails, browser cookie
access or arbitrary filesystem/shell APIs. CSP permits bundled assets and Tauri
IPC only; dev-server WebSockets are a development exception. Only local main-window
event listening is granted. Application commands have explicit ACL permissions.
Future process execution uses fixed executable paths and argument arrays.

URL syntax filtering does not prevent DNS rebinding or redirects. The real adapter
is gated on DNS/IP/redirect policy and adversarial tests; it remains disabled now.
Future output management reserves unique job directories, sanitizes filenames,
uses partial files, checks size/hash and FFprobe output, then finalizes atomically
on the same volume where possible. No output file is ever executed.

## Packaging and mobile

Desktop release sidecars require one vetted binary set per target triple:
`aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`,
`x86_64-unknown-linux-gnu` (arm64 Linux is a later supported target).
Use Tauri `externalBin` suffixed paths only when verified binaries are staged.
Do not enable them for mock builds. Sign macOS nested binaries before app signing
and notarization; Windows uses signed installers; Linux needs a documented glibc
baseline. Audit FFmpeg build configuration and all redistribution licenses.

Core models/repositories/queue policies can be reused on Android/iOS. Desktop
yt-dlp processes cannot be assumed to work on mobile: mobile extraction, sandbox
storage, background scheduling and platform policy require new adapters. Mobile
is an architectural direction, not a support promise.

## Primary references checked for Phase 0

- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
- [Vite integration](https://v2.tauri.app/start/frontend/vite/)
- [Tauri commands](https://v2.tauri.app/develop/calling-rust/)
- [Capabilities](https://v2.tauri.app/security/capabilities/)
- [Sidecar target naming](https://tauri.app/develop/sidecar/)
- [SQLx SQLite options](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html)
- [yt-dlp JavaScript runtime/EJS](https://github.com/yt-dlp/yt-dlp/wiki/EJS)

yt-dlp's documented Node minimum is 22; pass `--js-runtimes node` explicitly.
Prefer a vetted bundled runtime for release rather than relying on developer PATH.
Extractor registration is not proof that a live site permits or supports every URL.
