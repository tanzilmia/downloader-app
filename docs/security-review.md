# Implementation security review

Scope: UI, native IPC, Rust queue and real adapter. Implementation evidence,
not release certification. React/general TypeScript guidance from the
security-best-practices skill informed the frontend review.

## Controls and evidence

- Escaped React text, local assets, no raw HTML/eval/dynamic scripts. Production
  CSP restricts assets and connections to bundled content/IPC.
- Explicit main-window capabilities. IPC tests reject remote origins, another
  window, unsafe URLs and invalid settings. File actions accept job IDs, recheck
  completed provenance and verify owned paths/size/hash.
- Public HTTP(S) validation in TS/Rust plus authenticated loopback proxy. Every
  destination resolves to public addresses; connect to validated sockets. Tests
  cover mixed/excessive DNS answers, private/special addresses, proxy auth and
  private tunnels. Redirect requests use the same policy.
- Fixed tool paths, startup/per-use SHA-256, literal argument arrays without shell,
  reduced environment, no cookies/plugins/config/remote components. Bounded pipes,
  deadlines, free-space/working-size caps and actual descendant-kill tests.
- Native downloader only; local FFmpeg/FFprobe protocols/demuxers restricted.
  Portable FFmpeg has network support disabled. No DRM/sign-in bypass.
- UUID-owned partials, bounded sanitized names, symlink/collision checks,
  capacity checks, FFprobe streams/duration, nonzero size, SHA-256 receipts and
  no-overwrite publication. Tests reject tampered files and recover crashes.
- Persist before events, count workers until exit, reject late cancelled success.
  Typed storage health and recovery; 200-job/shutdown bounds tests.
- Native automation has isolated identity/data and a debug-only feature, with
  compile-time rejection in release builds.
- `pnpm audit --prod`: no known vulnerabilities on 2026-10-21.

## Limits before release

The proxy is policy for trusted tools, not an OS sandbox. Same-user executable or
path replacement between checking and use is outside these controls. Media parser
vulnerabilities need ongoing advisories and platform sandboxing. Controlled public
redirect/DNS service integration coverage is incomplete, although every connection
uses the tested proxy checks. Windows job-object behavior and other OS install
checks need native runners. Rust/bundled-runtime advisory and complete licensing
qualification, signing/notarization and clean-host checks remain release gates.
