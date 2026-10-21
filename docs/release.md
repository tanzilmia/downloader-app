# Local candidate and release gates

## Build locally

```sh
pnpm engine:portable
pnpm desktop:bundle
```

The lock file pins official yt-dlp/Deno assets using upstream GitHub release
digests, and FFmpeg 9.0.2 source from its official HTTPS archive. SHA-256 checks
bind downloads to the reviewed lock; upstream GPG verification is not claimed.
Cache: `output/release/cache`. Optional reuse:
`DOWNPOUR_FFMPEG_SOURCE_ARCHIVE=/absolute/path/ffmpeg-9.0.2.tar.xz`.

Native source builds need compiler/make/tar/unzip. Exact FFmpeg flags are pinned:
no autodetection, external libraries, network, GPL or nonfree options; only needed
containers/parsers/decoders. Remuxing copies codecs. macOS checks reject non-system
dynamic dependencies. FFmpeg/FFprobe explicitly target macOS 12.0 rather than
inheriting the build host version. Exact source/config/license accompany the separate
executables; Downpour does not link libav libraries. Executable paths are relative
inside `engines/<target>`. The bundler verifies hashes before packaging resources.

Local macOS package: `target/release/bundle/macos/Downpour.app`, about 154 MiB,
including a 139 MiB engine/source-notices directory. Developer manifests and the
native test driver must never ship.

| Target              | Evidence                                                         | Remaining qualification                     |
| ------------------- | ---------------------------------------------------------------- | ------------------------------------------- |
| Apple Silicon macOS | Native checks, real transfer, WKWebView flow, portable app built | Signing and clean-host acceptance           |
| Intel macOS         | Assets pinned; native source build path                          | Not run                                     |
| Windows x64         | Core/UI CI configured; yt-dlp/Deno assets pinned                 | Native FFmpeg, installers and process tests |
| Linux x64/arm64     | Source build path; x64 CI configured                             | Not run; establish glibc/WebKit baseline    |

CI checks three native OSes. The manual candidate workflow prepares an unsigned
macOS artifact. Workflows have not been pushed/run in this session. No automatic
publication or updater is configured.

## Before public distribution

1. Select application licensing and qualify all packaged runtime source/notices.
   yt-dlp's PyInstaller combined executable is **GPLv3+**; source Unlicense alone
   is insufficient. Deno is MIT plus dependency licenses. Preserve FFmpeg exact
   source/config/license. Current candidate is local, with complete runtime
   redistribution qualification still pending.
2. Run CI/clean-host installers and qualify authorized URLs per advertised site.
   Registration is not compatibility proof. Do not circumvent 403/429/sign-in/DRM.
3. Audit Rust/Python/PyInstaller/Deno/FFmpeg advisories and record an SBOM. The
   clean pnpm production audit covers only the JavaScript dependency scope.
4. Sign nested macOS executables, regenerate their hashes, then sign/notarize the
   containing app; verify stapling/Gatekeeper on a clean host. Signing credentials
   were not supplied. Windows needs a verified native engine and signed installer;
   Linux needs tested native package/dependency baselines.

Primary sources: [yt-dlp licensing](https://github.com/yt-dlp/yt-dlp#licensing),
[Deno license](https://github.com/denoland/deno/blob/v2.9.7/LICENSE.md),
[FFmpeg build/license guidance](https://ffmpeg.org/legal.html),
[Tauri signing](https://v2.tauri.app/distribute/sign/macos/).
