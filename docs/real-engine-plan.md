# Real engine implementation and acceptance

1. Real engine boundaries and migration: optional artifacts, typed dependency /
   storage health, preserve mock history. Accept shared old/new wire fixtures.
2. Network policy and process supervision: public DNS/redirect proxy, bounded
   streams, process group cancellation, deadlines, isolated executable manifest.
   Accept adversarial destination/proxy/process tests.
3. Files and processing: stable partial paths, capacity checks, nonempty files,
   FFprobe stream checks, SHA-256, receipts and atomic no-overwrite publication.
   Accept traversal/symlink/collision/corruption/recovery tests.
4. yt-dlp metadata/formats/download integration and local FFmpeg remux. Accept a
   real authorized public media download through the actual adapter plus local
   deterministic fixture tests. Record site/network failures honestly.
5. Desktop/UI: real engine status, truthful progress/history, open/reveal by job
   ID, clear storage/dependency failures, native flow and accessibility checks.
6. Reliability: safe cancel/retry/resume, interruption reconciliation, disk errors
   and bounded load checks. Run all existing regression checks.
7. Release preparation: native CI matrix, unsigned local package, engine manifest
   validator/stager, notices and signing/release instructions. Verify what this
   host supports; record external credentials/OS runners/portable binaries as
   outstanding gates when unavailable.

## Current acceptance

Items 1–6 are implemented with shared fixtures, adversarial file/process/network
tests, actual public downloads, native UI cancel/retry/restart and a 200-job load
check. Progressive HTTP is verified; per-site live qualification remains open.
Item 7 has pinned source/asset staging, an unsigned portable macOS candidate,
native CI definitions and documented release gates. Signing, full runtime
redistribution qualification and other OS native execution remain outstanding.
See progress.md and release.md for evidence and limits.
