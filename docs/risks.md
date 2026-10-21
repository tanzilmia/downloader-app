# Risk register

| Risk                                      | Impact                                       | Mitigation / release gate                                                                                     |
| ----------------------------------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| Site extractors change or require sign-in | Some URLs fail                               | Replaceable adapter, classified errors, maintained live compatibility matrix; never promise universal support |
| YouTube requires JS/EJS                   | Extraction fails despite yt-dlp installation | Test pinned yt-dlp/EJS/runtime together, bundle verified runtime, no implicit remote script update            |
| Desktop binaries on mobile                | Process/sandbox restrictions                 | Keep platform-neutral core; develop separate mobile extraction and lifecycle adapters                         |
| SSRF via DNS/redirects                    | Local network exposure                       | Mock does no networking; real adapter blocked until resolved IP/redirect policy is enforced and tested        |
| Cancellation races                        | Cancelled job later completes                | Serialize mutations; check state before progress/terminal write; count worker until exit                      |
| Crash mid-transfer                        | Lost/stuck queue                             | Persist first, emit second, recover downloading jobs to queue at startup, reconcile partial files in Phase 3  |
| Low disk, corrupt DB, migration failure   | Lost history                                 | Report storage errors, no automatic database deletion, transactional migrations and persistence tests         |
| Unsafe filename or existing output        | Traversal/data overwrite                     | Sanitize names, app-owned job directories, exclusive reservation and integrity checks in real file manager    |
| FFmpeg binary size/license                | Lightweight goal or redistribution failure   | Audit configure flags and licensing; separate app and engine footprint; signed manifest and SBOM              |
| Unsigned/native cross-platform packages   | Install or launch failure                    | Native CI on macOS/Windows/Linux; clean-machine tests and signing gates                                       |
| UI/Rust DTO drift                         | Runtime invocation/event failures            | Strict command map, Zod responses/events and shared Rust/TS wire fixture tests                                |
| Web preview storage differs from SQLite   | Browser checks miss native bugs              | Core repository integration tests and native launch smoke; identify preview mode in UI                        |
| External tooling installation             | Environment differences                      | Record versions and commands; pin dependency lockfiles; no reliance on PATH for release sidecars              |
