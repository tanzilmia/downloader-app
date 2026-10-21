# Engine staging

Portable staging now uses `engines/<target>/` at the workspace root via
`pnpm engine:portable`. `pnpm desktop:bundle` verifies relative paths and hashes,
then maps tools/source/notices into bundled resources. This legacy sidecar folder
stays empty; see `docs/release.md` and ADR 0004. Homebrew binaries are development
tools and must not be copied into portable packages.
