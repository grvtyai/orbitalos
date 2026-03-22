CREATE TABLE IF NOT EXISTS notes (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    archived_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_notes_updated_at
    ON notes (updated_at DESC);

CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS entity_tags (
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (entity_type, entity_id, tag_id)
);

CREATE INDEX IF NOT EXISTS idx_entity_tags_entity
    ON entity_tags (entity_type, entity_id);

CREATE TABLE IF NOT EXISTS entity_links (
    from_entity_type TEXT NOT NULL,
    from_entity_id TEXT NOT NULL,
    to_entity_type TEXT NOT NULL,
    to_entity_id TEXT NOT NULL,
    link_kind TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (
        from_entity_type,
        from_entity_id,
        to_entity_type,
        to_entity_id,
        link_kind
    )
);

CREATE INDEX IF NOT EXISTS idx_entity_links_from
    ON entity_links (from_entity_type, from_entity_id);

CREATE INDEX IF NOT EXISTS idx_entity_links_to
    ON entity_links (to_entity_type, to_entity_id);

CREATE TABLE IF NOT EXISTS attachments (
    id TEXT PRIMARY KEY,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    file_path TEXT NOT NULL UNIQUE,
    mime_type TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX IF NOT EXISTS idx_attachments_entity
    ON attachments (entity_type, entity_id);

CREATE TABLE IF NOT EXISTS change_log (
    id INTEGER PRIMARY KEY,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    change_kind TEXT NOT NULL,
    changed_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_change_log_entity
    ON change_log (entity_type, entity_id, changed_at DESC);
