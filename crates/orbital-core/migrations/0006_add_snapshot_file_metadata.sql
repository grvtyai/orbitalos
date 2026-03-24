ALTER TABLE snapshots
    ADD COLUMN file_path TEXT;

ALTER TABLE snapshots
    ADD COLUMN mime_type TEXT;
