# ADR 0005: User-selected download folders

The user requested a selectable download location. Keep that preference in the
existing versioned settings JSON and pin the selected canonical directory in each
new job's JSON. Missing optional fields preserve older settings/jobs and the
original download root. Changing preferences affects new jobs; queued/active work,
resumable partials, completed outputs and retries retain their recorded location.

The desktop owns a native directory picker behind one main-window-only command.
It accepts a reset flag, validates the selected existing folder with reversible
write/link probes, and saves the preference through the core manager. Ordinary
preference updates cannot set an arbitrary directory. Cancellation is a no-op.
Renderer code uses the shared validated command and receives a fresh snapshot;
it has no generic filesystem or dialog permissions.

The core treats the destination as optional persisted data and serializes enqueue
with preference updates. The real engine resolves a FileStore for each job. All
transfer, recovery, publication, cleanup and file actions use that store and keep
existing filename, receipt, checksum, symlink and no-overwrite checks. A missing
or redirected selected directory fails visibly instead of silently saving to a
different location. Platform picker behavior beyond macOS remains subject to
native CI qualification.
