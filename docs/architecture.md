# Downpour architecture

Approved stack: Tauri 2, React/TypeScript/Vite, Tailwind/shadcn, React Router,
Zustand, Rust/Tokio, SQLx/SQLite, Zod and pnpm/Cargo. See ADR 0004 and progress.

## Ownership and flow

Startup initialization runs on Tokio while the native event loop stays responsive.
Commands share one initialization and can retry a repaired startup storage error.
React owns presentation/theme; thin Tauri commands delegate to `download-core`,
which has no Tauri dependency. `media-engine` implements replaceable Extractor and
TransferEngine ports. SQLite is authoritative. Shared Rust/TS fixtures cover old
mock jobs and real artifacts. Native composes the real adapter; browser stays mock.

1. Validate public HTTP(S) URLs in TS/Rust. Verified yt-dlp inspects via a temporary
   authenticated loopback proxy. Require all DNS answers to be public and connect
   to a validated socket. Redirects and fragments use the same proxy policy.
2. Filter non-DRM HTTP formats; pair compatible video/audio streams. Enqueue
   revalidates the format, using a bounded short-lived metadata cache. Keep only
   allowlisted downloader inputs in RAM (120-second usable lifetime, 16 MiB cache,
   at most 32 entries). Never persist signed stream URLs or load remote thumbnails.
3. Insert queued work; claim indexed FIFO rows with 1–4 workers (default 2).
   Bounded progress channels feed serialized, persisted revisions before events.
4. Pipe the inspected stream metadata to yt-dlp through stdin (`--load-info-json -`)
   and download individual streams to UUID-owned stable partial paths. Avoid
   another webpage extraction per stream. Metadata excludes cookies/auth headers,
   output filenames, webpage fallback and unselected/unsupported formats; fragment
   addresses still require public HTTP(S) destinations. Expired inspections refresh
   once and validate the saved format before transfer. FFmpeg only
   receives local inputs with restricted protocols/demuxers, copies codecs and
   removes metadata. FFprobe verifies requested streams and positive duration.
5. Hash the nonempty staged file; sync its recovery receipt; publish on the same
   volume using a no-overwrite hard link. Commit completed output to SQLite before
   cleaning partial files. Library actions accept job IDs and recheck path/hash.

The loopback proxy is an internal network policy, not an external application
backend or OS sandbox. HTTPS remains end-to-end TLS. Vetted tools are trusted.

## State and recovery

States: queued → downloading/cancelled; downloading → completed/failed/cancelled/
queued (interruption); failed/cancelled → queued on explicit retry. Completed is
terminal. Progress is monotonic per attempt. Cancel persists before signalling
process groups, and slots remain occupied until work exits. Late output cannot
complete a cancelled attempt.

SQLx opens SQLite with WAL, foreign keys, busy timeout and embedded migrations 1/2.
Existing mock provenance remains. Startup requeues interrupted work before workers
start. Normal exit cancels and drains workers. Receipts reconcile process crashes
during publication. Typed `engine:health` reports storage failures; the scheduler
retries and reconciles stranded rows once storage works. Corrupt DBs are preserved.
Browser preview has separate validated, versioned localStorage and a single tab.

Real transfer progress includes an optional measured `speedBytesPerSecond` from
yt-dlp's JSON progress template. Percent maps across streams; speed retains its
actual bytes/second value. The manager validates and persists rates before events,
expires samples after three seconds without updates, and clears them on terminal
states, retry and startup recovery. Optional fields preserve existing job payloads.
Rows show decimal B/s, KB/s or MB/s; Downloads sums active real transfer rates.
Processing has no network rate, and simulated transfers never report real speed.

## Process, file and desktop boundaries

Find remains interactive during startup/dependency failures and shows lookup
errors beside the link. Failed tool-manifest loads can retry on Reconnect; startup
does not spawn executable version probes. Per-use verification still applies.

YouTube Shorts/share/watch aliases with a valid video ID use one canonical cache
key. YouTube tool operations run serially with a five-second gap and a one-second
extractor request delay. HTTP 429 becomes a typed `rate_limited` error with an
optional `retryAfterSeconds`. The adapter enforces a local session pause starting
at 60 seconds, doubling on repeated rejections up to 15 minutes. Webview reload
does not reset it; process restart does. Successful requests reset the backoff.
Cached metadata remains usable and other sources are unaffected. Home and preview
show an explicit countdown without sending an automatic retry. This policy reduces
repeat requests; it does not guarantee that YouTube removes its upstream block.

The new-download preview hides WebM choices and selects the first remaining MP4
or M4A format. A source with no remaining choices explains the limitation and
cannot enqueue a hidden format. Existing WebM history, contracts and engine
compatibility remain intact; this is a presentation preference.

Audio selection retains the best stream per container so higher-bitrate WebM
cannot displace M4A. If no separate M4A exists, a compatible combined MP4 may
provide an `audio:<source-id>` choice. The reserved prefix distinguishes its job
from the video choice and is removed only by the strict source-selector parser;
cached downloader metadata still contains inspected source IDs. FFmpeg copies
only the local audio track into M4A, and FFprobe rejects any video stream in an
audio-only result. Known silent/incompatible sources are not offered; unknown
direct-file codecs must pass actual processing and verification. The preview
groups audio before video and explains when a full source video transfer is
needed to save audio.

Tool manifests bind target/version/path/SHA-256. Portable paths stay within their
package; developer paths are absolute. Verify startup and per execution. Fixed
argument arrays, reduced environment, disabled cookies/plugins/config/remote
components, bounded streams, deadlines, group cancellation and disk caps constrain
execution. File names combine a sanitized bounded title, UUID and allowlisted
extension. Symlink files/directories and existing destinations are rejected.
Output volumes must support hard links. Cancelled partials remain for retry.
Process stdin is bounded to 8 MiB, written while both output pipes drain, and
closed on completion, cancellation or timeout. No temporary info JSON is created.

React escapes text. Production CSP permits local assets and IPC. Capabilities grant
only application commands and event subscriptions to the local main window. No
frontend shell/filesystem/network plugin access, sign-in sessions or telemetry.
Same-user concurrent path replacement and hostile executables require stronger OS
sandboxing; these checks do not claim that protection.

## Packaging and mobile

`engines.lock.json` pins official assets and FFmpeg source. Native preparation
builds local-only FFmpeg without GPL/nonfree/external libraries and retains exact
source/config/notices. Tauri bundles separate executables as resources. Signing
nested tools changes bytes: regenerate hashes before signing the containing app.
See release.md for outstanding qualification. Mobile can reuse core policies but
needs separate extraction, storage and lifecycle adapters.

Primary references: [Tauri capabilities](https://v2.tauri.app/security/capabilities/),
[resources](https://v2.tauri.app/reference/config/#resources),
[SQLx SQLite](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html),
[yt-dlp/EJS](https://github.com/yt-dlp/yt-dlp/wiki/EJS).
