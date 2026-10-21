# ADR 0004: Supervised real media engine

Accepted scope: the user approved the remaining native verification, real
yt-dlp/FFmpeg integration, safe files, reliability, security and release work.

Add a `media-engine` crate implementing the existing core ports. Desktop composes
the real adapter; browser preview keeps its explicit mock. Preserve old mock
history and migrate optional output metadata without resetting the database.

All extractor HTTP traffic uses an ephemeral authenticated loopback proxy. It
resolves every destination, rejects nonpublic/special-use IPs (including mixed
DNS answers), restricts ports, and connects to the validated socket address.
HTTPS CONNECT remains end-to-end TLS; HTTP redirects reconnect through the same
policy. There is no externally reachable application backend. Disable yt-dlp
config, browser cookies, plugins, remote components and external downloaders.
Only HTTP-based, non-DRM formats are exposed. This proxy is a network policy for
the vetted adapter, not a sandbox for arbitrary hostile executables.

Spawn fixed, SHA-256-verified executables with argument arrays and a reduced
environment. Supervise their process groups, drain both pipes with bounds, and
enforce cancellation/timeouts. Download streams separately using yt-dlp's native
downloader; FFmpeg receives only local files with a restricted protocol list.
FFprobe, nonzero size and SHA-256 checks precede atomic no-overwrite finalization.

Use UUID-owned partial directories and stable stream filenames for retry/resume.
Persist an output receipt before finalization so startup can reconcile a crash
between filesystem completion and database commit. Cancellation cannot publish a
late completion; discarded outputs are only removed through the owned file store.

Expose engine availability, output location and health errors through typed IPC.
Open/reveal operations accept job IDs and revalidate stored output paths; the UI
never gets arbitrary shell/filesystem access. Test native WKWebView interactions
with a feature-gated driver that is absent from release builds.

Release tooling separates local development tools from portable engine bundles.
Local installed tools may be snapshotted into a verified developer manifest.
Portable bundles require relative paths, hashes, platform triples, provenance and
license/source notices. Do not claim portable installers, signing or notarization
until native runners and credentials actually verify them.
