# Working on Downpour

Downpour is a local-first desktop downloader. Read `docs/architecture.md`,
`docs/plan.md`, and the ADRs before changing module boundaries.

## Scope and boundaries

- Follow the approved stack: Tauri 2, React/TypeScript/Vite, Tailwind/shadcn,
  React Router, Zustand, Rust/Tokio, SQLx/SQLite, Zod, pnpm, Vitest.
- `apps/desktop`: UI and thin Tauri command/event adapters only.
- `packages/contracts`: validated command/event wire contracts and fixtures.
- `crates/download-core`: domain, extractor/transfer ports, queue, persistence.
- Keep core independent of Tauri and desktop APIs for future mobile adapters.
- Phase 1 uses a clearly labelled mock engine. Never imply simulated jobs
  created real media. Real extraction, sidecars and media processing are Phase 3.
- Do not add a server, cloud storage, telemetry, or remote UI assets.

## Reliability and security

- SQLite is authoritative on desktop. Run embedded, versioned migrations on
  first launch; never silently reset corrupt storage.
- Validate URLs in both TypeScript and Rust; accept public HTTP(S) DNS names
  only. Never invoke a shell or accept arbitrary command arguments from UI.
- All state transitions go through the domain state machine. Persist changes
  before emitting events. Retain terminal jobs until explicit removal is built.
- Limit worker concurrency; cancellation must prevent later completion events.
- Never collect cookies, tokens or passwords; never bypass access controls/DRM.
- Use explicit Tauri capabilities and CSP. UI has no shell/filesystem access.
- Future real adapter must validate DNS/redirect destinations and file outputs
  before it is enabled. A hostname syntax check is not an SSRF boundary.
- Do not bundle unverified binaries; record checksums, sources and licenses.

## Workflow and checks

Inspect first, implement in dependency order, test failure paths, update progress
and document evidence. Do not mark a phase complete without its acceptance checks.

Run `pnpm check` (format, lint, types, tests, web build),
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
and `cargo test --workspace`. Use `pnpm desktop:build` for an unsigned local build.
Never commit build artifacts or credentials. Windows/Linux packaging and signing
must be verified on their native CI runners before a release.
