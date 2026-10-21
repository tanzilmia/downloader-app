CREATE TABLE jobs (
    id TEXT PRIMARY KEY NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('queued','downloading','completed','failed','cancelled')),
    created_at TEXT NOT NULL,
    payload TEXT NOT NULL CHECK (json_valid(payload))
);
CREATE INDEX jobs_status_created ON jobs(status, created_at);
CREATE TABLE settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    payload TEXT NOT NULL CHECK (json_valid(payload))
);
INSERT INTO settings (id, payload) VALUES (1, '{"concurrency":2,"theme":"dark"}');
