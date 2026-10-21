# ADR 0003: Validate a mock vertical slice before shipping native engines

Status: accepted.

Implement async `Extractor` and `TransferEngine` ports and deterministic mocks.
Expose mock status throughout UI and persist `isMock` on jobs. Mock jobs never
claim output paths. Do not enable `externalBin` until platform binaries are
audited, staged, hashed, licensed and signed as appropriate.

Consequence: this foundation can be built without FFmpeg or yt-dlp installed.
Developer probes verify availability and registration separately from live
extractor success. Mobile needs alternative engine adapters, not a desktop
subprocess assumption. Native binary packaging remains a later release gate.
