# Native engines (not bundled in Phase 1)

Do not place arbitrary developer binaries here. Phase 3 will stage verified
yt-dlp, FFmpeg, FFprobe and a JS runtime using target-suffixed filenames, a SHA-256
manifest, version/source/license notices and executable permissions. Then enable
the corresponding `bundle.externalBin` entries. Homebrew binaries are developer
tools, not portable redistributable sidecars.
