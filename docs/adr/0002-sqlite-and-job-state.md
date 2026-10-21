# ADR 0002: SQLx SQLite and durable validated state transitions

Status: accepted.

SQLite WAL in Tauri app-data with embedded forward migrations is authoritative.
Persist snapshots with indexed status and monotonic revision; emit events only
after commit. A manager lock serializes job mutations and a bounded worker set
owns cancellation tokens. Recovery requeues interrupted downloading jobs before
scheduling. Explicit retry resets progress but preserves attempt count.

Consequence: one application process owns this database. Multi-instance locking
is enforced with the desktop single-instance plugin. Mock recovery restarts an
attempt; resumable real partial files need a later reconciliation design.
