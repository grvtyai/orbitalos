CREATE TABLE IF NOT EXISTS snapshots (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    kind TEXT NOT NULL,
    source TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    archived_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_snapshots_updated_at
    ON snapshots (updated_at DESC);
