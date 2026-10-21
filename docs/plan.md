# Implementation and acceptance plan

## Phase 0 — feasibility spike

- Inspect repository, OS, Xcode CLI tools, Rust, Node/pnpm and media dependencies.
- Verify Tauri/Vite development and unsigned native compilation on this host.
- Run SQLite migrations on a new database, reopen it, verify recovery.
- Check yt-dlp version, extractor registration and a bounded public test metadata
  probe; record live network/site failures separately from local tool failures.
- Verify FFmpeg/FFprobe execute, and record JS runtime and EJS needs.
- Deliver architecture, structure, risks, ADRs, rules and this acceptance plan.
- Record native sidecar feasibility; do not claim Windows/Linux/signing tested.

## Phase 1 — foundations and mock vertical slice

Dependency order and independently testable tasks:

1. Workspace/tooling: pnpm and Cargo workspaces, strict TS, formatting/lint/build.
   Accept: workspace install, typecheck and Vite build succeed.
2. Contracts/domain: Zod DTOs, Rust serializable DTOs, transition table and URL
   validation. Accept: shared fixtures and negative input/transition tests pass.
3. Persistence: embedded migrations, repositories, settings and startup recovery.
   Accept: fresh DB initializes, re-open preserves data, recovery resets attempts
   safely, invalid status is rejected and settings survive restart.
4. Mock ports and queue: replaceable extractor and engine, bounded worker set,
   events, progress, cancellation, classified errors and retry.
   Accept: integration tests exercise success/cancel/retry/failure/recovery and
   concurrency, with no media downloads or files claimed.
5. Desktop bridge: thin commands, runtime validation, event subscriptions,
   revisions and explicit ACL. Accept: Rust builds; browser and desktop adapters
   pass the same contract checks; no global shell/filesystem capability.
6. UI vertical slice: Home → preview/format → queue → simulated Library; Settings
   persists concurrency/theme. Accessible loading/empty/error states and responsive
   layout. Accept: browser E2E including reload, cancellation, retry and settings;
   keyboard and narrow-window checks; native launch smoke test if available.

Phase 1 deliberately builds only enough of the five required screens to exercise
the foundation. Rich library organization, real files and advanced features wait.

## Later phase gates

2. Refine all screen states, accessibility, design system and frontend E2E coverage.
3. Add audited yt-dlp adapter + supervised FFmpeg, DNS/redirect enforcement,
   sidecar manifest/checksums/licenses, file integrity and safe finalization.
   Accept real authorized downloads across a maintained compatibility matrix.
4. Expand recovery/process supervision, queue controls and advanced features.
5. Run comprehensive security, performance, native E2E and packaging regressions.
6. Build native release matrix, sign/notarize and verify installers on clean hosts.

Do not describe the app as production-ready until all required release gates pass.
