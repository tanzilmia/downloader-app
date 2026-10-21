# Verification and progress

Verified on 2026-10-21 (Asia/Dhaka), macOS 26.6.2 Apple Silicon. Initial foundation
has been extended with the user-approved real downloader and release preparation.

## Delivered

- Real, replaceable yt-dlp adapter; bounded metadata and safe format selection.
- Public-destination authenticated loopback proxy; all DNS answers checked and
  socket addresses pinned, including subsequent redirect/fragment requests.
- SHA-256-verified executable manifests, fixed arguments, reduced environment,
  group cancellation, bounded pipes, deadlines and working-size/free-space caps.
- Local-only FFmpeg remux, FFprobe stream/duration checks, nonzero size and hash.
- Stable resumable partials, safe names, symlink/collision defenses, durable
  receipt and atomic no-overwrite publication; safe ID-based Open/Show in folder.
- SQLite migration 2 preserves old mock history and adds real output metadata.
  Typed health/dependency UI, graceful shutdown, startup/storage reconciliation.
- Native real-engine labels; browser stays explicitly simulated. Home/preview,
  queue, Library, Settings and light/dark/system appearance remain functional.
- Official pinned yt-dlp/Deno assets and source-built minimal portable FFmpeg;
  checksums/source/config/notices retained. Local portable `.app` built.
- Native CI matrix and manual unsigned-candidate workflow; no push/publication.

## Automated checks

| Check                                                   | Result                                                                  |
| ------------------------------------------------------- | ----------------------------------------------------------------------- |
| `pnpm check`                                            | Formatting, lint, types, 47 Vitest checks and production web build pass |
| `pnpm test:e2e`                                         | 9 browser workflow/keyboard/persistence/axe checks pass                 |
| `cargo fmt --all -- --check`                            | Pass                                                                    |
| `cargo clippy --workspace --all-targets -- -D warnings` | Pass                                                                    |
| `cargo test --workspace`                                | 44 Rust checks pass                                                     |
| `pnpm audit --prod`                                     | No known vulnerabilities                                                |
| `pnpm test:native:build` / native flow                  | Actual WKWebView suite passes; evidence below                           |
| `pnpm engine:portable` / `pnpm desktop:bundle`          | Verified engine staged, unsigned macOS app built                        |

100 deterministic unit/integration/browser tests, plus opt-in native and live
integration runs. Tests include shared old/real contracts, migration/reopen,
terminal-state races, process timeout/output overflow/descendant termination,
network policy, file tampering/symlinks/collision/receipt crash recovery and 200
queued jobs limited to 4 workers with graceful resumable shutdown, plus injected
SQLite write failures that surface health errors and recover without restart. Axe scans cover
narrow Home/preview and all populated screens in both themes; manual assistive
technology testing is still needed.

## Real/native evidence

Authorized public fixture:
`https://download.blender.org/durian/trailer/sintel_trailer-480p.mp4`.
Actual yt-dlp → FFmpeg → FFprobe → SHA-256 completion succeeded with both developer
and portable toolchains. Saved output: 4,368,946 bytes, duration 52.208333 seconds,
SHA-256 `0ddfda269653c691c41a90c9d14a8fe5fa43bec79339a679427edf69c11fc2cd`.
Portable output/SQLite evidence: `output/release/portable-live/`.

Native WKWebView flow evidence:
`output/native-e2e/b52fcc5c-cda8-4975-8663-926f7344a27e/report.json`.
Actual metadata/format selection, cancellation leaving 261,120 partial bytes,
retry/completion, verified Library output, successful native folder reveal, and preferences/history across webview
reload and process restart all pass. Automation uses isolated test data, the portable engine, and a
feature excluded from release builds. The settled screenshot is `library.png`
beside the report. Debug cold startup reached engine readiness in 19.2 seconds;
the webview stays responsive during initialization. Cold-start performance needs
further release qualification. Final normal LaunchServices launch applied both
migrations successfully in the actual app-data DB. Existing private working
folders avoid redundant permission writes; new Unix folders are created as 0700.

Portable engine: `engines/aarch64-apple-darwin/`, about 139 MiB. Bundle:
`target/release/bundle/macos/Downpour.app`, approximately 154.17 MiB. All four hashes
inside the bundled resource directory match the staged manifest. FFmpeg/FFprobe
link only to system libraries; no Homebrew libraries are redistributed. Core/UI
adds about 15 MiB; bundling runtimes dominates the full package.

Tools: Rust/Cargo 1.99.1, Tauri Rust 2.12.4 / CLI 2.12.3, Node 22.17.0, pnpm
11.0.5, yt-dlp 2026.08.19, FFmpeg/FFprobe 9.0.2, Deno 2.9.7. Local developer
setup is installed without modifying shell profiles.

## Remaining qualification

The public direct-media flow is verified, not universal site support. An earlier
YouTube test returned 429; another W3C URL returned 403. No bypass attempted.
Facebook/Instagram/TikTok live URLs have not been qualified. Local H.264/AAC stream-pair merging and audio-only remux were verified using the
portable FFmpeg/FFprobe; evidence: `output/release/processing-fixtures/report.json`.
Broader real-site stream-pair coverage is still required. Controlled external redirect/DNS service testing and
OS-specific hard-kill recovery coverage should expand.

Signing/notarization credentials, other OS runners/clean hosts, complete runtime
source/notices/advisory/SBOM qualification and application licensing remain release
gates. CI definitions have not been executed remotely. Windows portable FFmpeg and
installers remain pending. Cancelled partial cleanup/retention and advanced queue
features are future work. Mobile extraction/background adapters are not delivered.
See release.md and security-review.md. This is a working unsigned local candidate,
not a qualified production release.

## YouTube error classification follow-up

The user-reported `https://youtu.be/QxWoYkrgpJE` returned an HTTP 429 warning
followed by a bot-check/sign-in error. The adapter previously suppressed warnings
and misclassified the remaining bot-check text as a content access restriction.
Keep bounded stderr warnings; classify rate limits and bot challenges as network
errors before sign-in restrictions. A generic 403 no longer proves private media;
actual private/membership/age/DRM restrictions remain unsupported. Numeric video
IDs cannot masquerade as status codes. Five regression tests cover these cases.

A metadata-only probe through the actual app proxy confirmed the new typed result:
`network`, retryable, YouTube HTTP 429. Evidence:
`output/release/youtube-link-diagnosis.json`. The upstream block remains; this
change fixes the diagnosis and does not claim a successful YouTube download. The
installed yt-dlp 2026.08.19 also matched the latest official stable release checked
on 2026-10-21. No cookie collection or access-control bypass was added.

A later user-reported Shorts link (`https://www.youtube.com/shorts/D_8JUUup5js`)
was checked once through the same real adapter, without downloading. It also
returned the typed retryable YouTube HTTP 429 result; evidence:
`output/release/youtube-shorts-diagnosis.json`. The official latest stable release
was rechecked and still matched the installed version. No application change or
new build is claimed to remove this upstream rate limit.

Reproduce without downloading:

```sh
cargo run -p media-engine --example inspect_media -- engines/aarch64-apple-darwin/engine.json 'https://youtu.be/QxWoYkrgpJE'
```

## Find button and startup recovery follow-up

Find previously stayed disabled when startup was incomplete or the cached engine
status was unavailable. It now disables only during an active lookup; URL validation
and all Rust tool/network checks still apply. Dependency failures appear beside
the link and leave Find usable. Startup verifies the manifest, platform, paths and
hashes without spawning four executable version probes. Runtime operations verify
tools immediately before execution. Failed manifest loads can be retried on
Reconnect or inspection without rebuilding the manager or losing history.

Two Rust regression tests verify startup launches no processes, changed tools
cannot execute, and repaired manifests recover in the same backend. The existing
corrupt-storage browser test verifies Find remains interactive, displays the
error and preserves stored data. All 9 browser checks pass. CLI browser proof
with an unavailable engine snapshot: `output/playwright/find-enabled-unavailable.png`.

Actual native test: `pnpm test:native:startup` with a verified engine manifest.
Evidence: `output/native-e2e/startup-0fe51744-8d55-46ba-b0e1-a882546b3730/report.json`.
Missing tools, inline failure recovery, manifest repair/reconnect, healthy process
restart and actual public Sintel metadata all pass. Native ready time was 659 ms
in this run, compared with the previous 19.2-second debug cold-start observation;
these are single local observations rather than a performance guarantee.
The settled native Home screenshot is `find-enabled.png` beside the report.

## Live download speed follow-up

Active real downloads now show their measured transfer speed below the progress
bar. The queue summary shows the combined speed of all active real downloads.
Formatting uses decimal B/s, KB/s and MB/s. Unknown samples show a dash; local
processing shows its own label. These are app transfer rates, not a connection
capacity test. Browser simulations do not invent download speed.

The engine emits selected JSON progress fields from yt-dlp, retaining actual
bytes/second independently of the percentage mapping for multiple streams. Core
validates and persists rates before events, expires silent samples after three
seconds (on its next one-second tick), and clears speed on cancellation,
completion, failure, retry and interruption recovery. Older JSON jobs remain
compatible without a new SQLite migration. Added parser/contract/format checks and
core regressions cover persistence, stale samples, invalid rates and transitions.

`pnpm check`, all 9 existing browser workflows, Rust checks and Clippy pass. CLI
browser checks also verify a two-download total, 540px layout and axe scans in
both themes. Layout fixtures: `output/playwright/download-speed-narrow.png` and
`output/playwright/download-speed-narrow-light.png`.

Actual native flow passes with isolated data and the real portable engine:
`output/native-e2e/d35a3cd0-3f2f-40e2-aed2-b51eda3b3246/report.json`.
Its `download-speed.png` captures **515.2 KB/s** on both the row and summary
(the test intentionally limits transfer to 512 KiB/s). The suite confirms live
speed, cancellation reset, resumable retry, verified file output, reveal, reload
and restart. The saved Sintel checksum matches the existing authorized fixture.
The portable unsigned app was rebuilt; log: `output/release/download-speed-bundle.log`.

## YouTube request backoff follow-up

Repeated 429 reports exposed a missing app-side request pause. A fresh Find click
could launch another lookup immediately after a previous rejection. Shared URLs
and Shorts/watch URLs also used separate metadata cache keys. The adapter now
canonicalizes recognized YouTube video IDs, rechecks cache after serialized
inspection, spaces YouTube operations by five seconds and adds a one-second
extractor request delay. Other sources keep their existing behavior.

HTTP 429 has a typed `rate_limited` code and optional `retryAfterSeconds`, with
backward-compatible deserialization for old errors. A monotonic local pause starts
at 60 seconds, doubles for repeated rejection up to 900 seconds, and resets after
success. It applies to inspection and transfer, survives webview reload, and is
in memory for the app session. Home/preview show the countdown and explanation;
another source link stays usable. No automatic retry or unblock guarantee is made.
This mitigates unnecessary requests; the user-reported upstream block is unresolved.

Five new Rust checks cover canonical aliases, expiration/backoff/reset, cancellation,
one extraction for concurrent aliases, and preventing further lookup/transfer
tool launches during a pause. These use local verified test fixtures, without
contacting YouTube. The wire-contract test preserves retry timing and rejects
invalid values. Rust evidence: `output/release/youtube-backoff-rust.log` and
`output/release/youtube-backoff-engine-tests.log`.

Native controlled-fixture test: `pnpm test:native:rate-limit`, with isolated data.
Evidence: `output/native-e2e/rate-limit-16dc5be0-863a-4f12-abf3-906b23b383d4/report.json`.
Exactly one fixture process ran; no YouTube traffic occurred. It verifies the
visible ticking countdown, simulated UI clock expiry, continued Rust enforcement,
another source remaining interactive, and reload/alias retries without another
tool launch. Its `youtube-pause.png` was visually inspected.
CLI browser checks with a controlled wire error also pass at 540px without
horizontal overflow, including axe scans of the pause UI in dark and light themes.

Real public Sintel metadata and startup/dependency repair still pass:
`output/native-e2e/startup-9e5af431-0aa4-4b1d-8689-38f504049d8d/report.json`.
The portable unsigned app was rebuilt; log: `output/release/youtube-backoff-bundle.log`.
The backoff follows the intent of yt-dlp's [request-spacing guidance](https://github.com/yt-dlp/yt-dlp/wiki/Extractors#common-youtube-errors);
the local 60–900 second pause is an app policy, not a retry time supplied by YouTube.

## Reuse inspected metadata during transfer

The new failed job was `W9-MV7HWb3g`, format `18` (360p), attempt 1, zero progress,
with a typed rate-limit error. Its preview succeeded earlier; the transfer still
launched a fresh full webpage extraction. That repeated lookup is now removed.
Inspected downloader inputs stay in a bounded RAM cache, reusable for 120 seconds,
and pass to each stream download through stdin. No signed stream URL reaches
SQLite, process arguments or a temporary info file. Cache limits are 16 MiB and 32
entries; each stdin payload is at most 8 MiB. A refreshed inspection rejects a
saved format that is no longer available.

The downloader payload retains approved stream fields and safe request headers.
It excludes cookies/authorization, metadata filenames and webpage fallback, and
resolves/validates fragment URLs. Existing proxy, tool verification, per-source
pause, cancellation and file-integrity checks remain in effect. Stdin writes and
stdout/stderr draining run concurrently under the same process deadline.

Four Rust regressions cover cache limits/expiration, payload filtering/fragments,
one inspection reused for two stream transfers (plus stale-format rejection), and
bounded stdin, concurrent output draining and a child that never reads input.
All 44 Rust, 47 Vitest and 9 browser checks pass. Evidence:
`output/release/inspection-reuse-workspace.log`, `inspection-reuse-unit.log`,
`inspection-reuse-frontend.log`, `inspection-reuse-browser.log` and
`inspection-reuse-clippy.log`. A concurrent feature build initially invalidated
the desktop doctest dependency artifacts; a sequential workspace rerun passes.

Real native public Sintel download, speed, cancellation/resume/retry, verified
checksum, reveal, history and restart all pass with stdin metadata reuse:
`output/native-e2e/5d07d623-c11f-4bf8-9f47-5ad9055ba258/report.json`.
The output is 4,368,946 bytes and matches the existing authorized fixture checksum.
Controlled 429 suppression also still passes:
`output/native-e2e/rate-limit-08a94eeb-0909-4466-8f40-079e6350ef34/report.json`.

One bounded live attempt of the user's actual YouTube URL with format `18`
returned HTTP 429 during the initial metadata inspection, before download could
start. Evidence: `output/release/inspection-reuse-youtube.log`. No successful
download of that YouTube video is claimed. Removing redundant extraction reduces
request load but does not remove the upstream block. Portable app build evidence:
`output/release/inspection-reuse-bundle.log`.

## Hide WebM from download choices

The preview now omits WebM video/audio choices and defaults to the first visible
MP4 or M4A choice. An unavailable-format message appears if nothing remains, and
enqueue is disabled with an additional selection guard. Existing saved jobs and
engine/container compatibility are preserved. No conversion is added.

CLI browser fixtures verify that an incoming WebM-first list shows the remaining
three MP4/M4A choices, selects/enqueues MP4, and that a WebM-only source has no
radio options, explains why and cannot enqueue. Screenshot:
`output/playwright/no-webm-options.png`. Existing 9 browser workflows, `pnpm check`,
Rust formatting/workspace tests and Clippy pass. Evidence: `output/release/hide-webm-*`.
The portable unsigned app is rebuilt in `hide-webm-bundle.log`.

## Desktop interface cleanup

Refined the existing green identity with a quieter neutral background, clearer
text, a compact sidebar and route-aware breadcrumbs. Downloads now has four
aligned summary cells, counted filters, a bordered file list, consistent status
and action columns, and MP4/M4A file icons instead of repeated sample thumbnails.
Live transfer speed, errors, cancellation, retry, Open and Show in folder retain
their existing behavior. Small windows group status and actions below each title
and wrap controls when needed. Home uses the full width for recent files; the
extra promotional card is removed. Library and Settings share the updated spacing
and direct page titles. WebM remains excluded from the download format picker.

`pnpm check` (47 unit checks), all 9 existing browser workflows, Rust formatting,
Clippy and all 44 Rust checks pass. CLI browser checks cover active/finished
filters, real-job-shaped visual fixtures with speed/failure/output states, long
Bengali titles and row-control placement. Axe scans at 360, 540, 800 and 1280 pixels
in both themes report no WCAG A/AA violations or horizontal overflow. Fixture
screenshots demonstrate layout only, not additional live downloads.

Evidence: `output/release/clean-ui-{check,e2e,rust,visual,accessibility}.log` and
`output/playwright/clean-ui-{downloads-dark,downloads-light,active-dark,narrow-dark,narrow-light,360-light,home-dark,home-light,library}.png`.
Portable app build log: `output/release/clean-ui-bundle.log`.
