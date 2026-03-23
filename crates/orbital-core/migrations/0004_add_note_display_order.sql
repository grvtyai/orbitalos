ALTER TABLE notes
ADD COLUMN display_order INTEGER NOT NULL DEFAULT 0;

WITH ordered AS (
    SELECT
        id,
        ROW_NUMBER() OVER (ORDER BY updated_at DESC, created_at DESC) AS sort_index
    FROM notes
)
UPDATE notes
SET display_order = (
    SELECT sort_index
    FROM ordered
    WHERE ordered.id = notes.id
);

CREATE INDEX IF NOT EXISTS idx_notes_display_order
    ON notes (display_order ASC, updated_at DESC);
