# ADR 0001: Local Rust core and thin Tauri transport

Status: accepted for Phase 1 from the approved brief.

Use an independent Rust crate for domain, repositories, queue and extractor/engine
ports. Compose it in Tauri; the UI only calls a validated command bridge. This
keeps a single local authority and makes future desktop/mobile transports
replaceable. We reject a cloud backend and local HTTP daemon because neither is
needed. Browser preview is a separate development adapter, explicitly labelled.

Consequence: maintain Rust and TypeScript wire models with shared contract fixtures
and runtime response/event validation until code generation becomes worthwhile.
