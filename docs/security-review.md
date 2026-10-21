# Foundation security review

Scope: Phase 1 React/TypeScript UI, wire validation, Rust command boundary, native
permissions and mock queue. This is a focused implementation review, not a release
security certification. Rust-specific guidance was reviewed against code/tests;
the security-best-practices skill supplies React and general frontend guidance.

## Verified controls

- UI renders untrusted strings with React's normal escaping. No raw HTML, eval,
  dynamic scripts, arbitrary navigation, remote thumbnails or remote fonts.
- Public HTTP(S) URLs are validated in Zod and Rust. Shared negative fixtures
  reject file/script schemes, credentials, IP literals and local/reserved names.
- Native commands revalidate URL/format/settings inputs and use bound SQL
  parameters. No shell commands or media executables run in the application.
- `tauri.conf.json` restricts production scripts to bundled assets and connections
  to IPC. Inline styles support numeric progress widths and the dialog library;
  script `unsafe-inline` is limited to Vite development, never the release policy.
- `capabilities/main.json` grants only listed commands and event listen/unlisten
  to the local main window. IPC tests demonstrate denial for a remote origin and
  another window, plus rejection of invalid payloads.
- No shell/filesystem/HTTP plugin capabilities, cookie access, credentials,
  analytics or cloud services. Browser storage holds only validated mock jobs and
  preferences; it is separate from native SQLite.
- Job cancellation/terminal states are serialized and committed before events.
  Corrupt browser data is preserved; failed writes do not publish success events.
- `pnpm audit --prod` returned no known vulnerabilities during this session.

No exploitable high/critical issue was identified within this mock scope.

## Gates before real networking or release

1. URL hostname validation is not DNS/redirect enforcement. Keep real extraction
   disabled until public destination validation, rebinding/redirect behavior and
   yt-dlp network paths are threat modeled and tested.
2. `safe_filename_stem` only sanitizes a stem. It does not implement a safe file
   manager. Exclusive output reservation, symlink/overwrite defenses, partial-file
   handling, integrity verification and atomic finalization remain Phase 3.
3. Vet sidecars and JavaScript challenge components, pin versions/checksums,
   constrain process arguments/lifecycle and retain license/build provenance.
4. Run Rust advisory/supply-chain review, OS-native security tests, resource-limit
   tests and signed clean-machine installation checks before release.
5. Native disk failures currently surface through startup/command errors and
   worker logs. Add a typed engine-health event and recovery UX before real
   transfers so prolonged storage failure is immediately visible.
