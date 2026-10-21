# Verification and progress

Verification date: 2026-10-21 (Asia/Dhaka). Repository originally contained only a
README and one initial commit. No existing implementation or project rules needed
migration.

## Phase 0 results

| Check              | Evidence and result                                                                                                                                                                                                       |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Host               | macOS 26.6.2, Apple Silicon / `aarch64-apple-darwin`                                                                                                                                                                      |
| Native SDK         | Apple clang 21; Xcode Command Line Tools at `/Library/Developer/CommandLineTools`                                                                                                                                         |
| JavaScript         | Node 22.17.0, pnpm 11.0.5; Vite production build and dev server succeed                                                                                                                                                   |
| Rust               | Initially missing; installed stable Rust/Cargo 1.99.1 plus rustfmt/clippy with rustup, without shell-profile changes                                                                                                      |
| Tauri              | Rust 2.12.4, CLI 2.12.3; native check, release compilation, unsigned `.app` bundle and process launch succeed                                                                                                             |
| SQLite             | SQLx 0.8.6, automatic create/migrate, WAL, reopen persistence and interrupted job recovery pass integration checks                                                                                                        |
| Actual app-data DB | Launching the built app created `downpour.db`; `_sqlx_migrations` reports version 1/success 1; settings exist; `PRAGMA journal_mode` returns `wal`                                                                        |
| yt-dlp             | Initially missing; Homebrew installed 2026.08.19; `--list-extractors` includes YouTube, Facebook, Instagram and TikTok                                                                                                    |
| Live metadata      | Bounded `--simulate --no-playlist --ignore-config --js-runtimes node` probe of yt-dlp's public test video returned YouTube HTTP 429/video unavailable. Live compatibility remains unverified; no retries/bypass attempted |
| Media tools        | Homebrew FFmpeg/FFprobe 9.0.2 execute successfully; installed build reports `--enable-gpl` and is not a release sidecar                                                                                                   |
| JS challenges      | Node 22 meets the documented minimum; Homebrew also installed Deno 2.9.7. Release runtime/EJS bundling remains a gate                                                                                                     |
| Binary packaging   | Tauri `.app` works on this host; sidecar target naming documented. Portable sidecars, licenses, other OS packages and signing are unverified                                                                              |

The tool-installation gap is resolved for local development. The live-site result
is an explicit external compatibility limit. Phase 0 establishes feasibility for
the mock foundation; it does not approve shipping a real extractor.

## Phase 1 delivered

- pnpm/Cargo modular workspaces; strict TypeScript, formatting, lint, tests/build.
- Tauri-independent Rust domain, extractor and transfer interfaces.
- Versioned SQLite migration and local repositories/settings.
- FIFO bounded queue, persisted revisions/progress, cancellation, explicit retry,
  classified worker failures and startup recovery.
- Thin Tauri commands with explicit permissions, local-window event delivery and
  CSP; single-instance desktop plugin prevents competing schedulers.
- Typed command/event bridge with Zod validation and shared Rust/TS fixtures.
- Clearly labelled browser development adapter with versioned local persistence.
- All five initial views, radio format selection, loading/empty/error states,
  searchable/filterable mock history, light/dark/system theme and concurrency.
- No remote runtime assets, application backend, cookies or actual media files.

## Verification evidence

Final required checks: `pnpm check`, `pnpm test:e2e`, `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
and `pnpm desktop:build`. Results are recorded below.

- TypeScript/Vitest: 37 contract and browser-adapter checks, including URL rejection,
  stored-data corruption and failed writes without events or nondurable mutations.
- Rust: 10 checks covering domain/state and filenames; shared wire fixtures; SQLite migration,
  reopen and recovery; durable monotonic events; bounded concurrency; cancellation,
  retry and classified failure; Tauri IPC validation and denial for remote origins
  and windows outside the main capability.
- Browser E2E: 9 checks covering end-to-end format/queue/history flow, cancellation/retry, concurrency,
  settings and interrupted reload; corrupt-storage startup errors; no remote
  runtime requests; narrow-window/keyboard checks and axe scans.
- Native smoke: macOS release bundle launched; app-data database/migration/WAL
  verified. A complete automated real-WKWebView interaction suite is not available
  here; browser E2E and Tauri mock-runtime IPC tests cover the layers separately.

All listed tests, formatting/lint/type checks and the local unsigned macOS bundle
build pass. This is 56 automated checks in total. Axe scans cover Home and preview
at 540×720, and populated Home, Downloads, Library and Settings in both themes.
Automated accessibility checks do not replace assistive-technology user testing.

## Remaining work and limitations

Phase 2 includes further interaction/a11y coverage, real native webview testing,
more recovery/error UX and design refinement. Phase 3 enables a real extractor
only after network policy, process supervision, file safety/integrity, verified
sidecars and dependency notices pass. Phase 4 expands scheduler behavior and
process/partial-file recovery. Phases 5/6 include broader security/performance,
native OS matrix, installers, signing and release qualification.

Storage failures on native worker progress are logged; if persistence stays
unavailable, restart after repairing storage to recover in-flight jobs. Browser
storage failure stops its preview scheduler. Browser multi-tab scheduling is not
supported. No throughput, resume, FFprobe integrity, native real-media or mobile
support is claimed. Application source licensing is not selected; Cargo packages
are marked nonpublishable.
