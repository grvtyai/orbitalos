use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::note::{note_excerpt, NewNote, NoteDocument, NoteId, NoteSummary, NOTE_ENTITY_TYPE};
use crate::error::{OrbitalError, OrbitalResult};

pub struct NoteRepository<'connection> {
    connection: &'connection Connection,
}

impl<'connection> NoteRepository<'connection> {
    pub fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }

    pub fn create(&self, note: NewNote) -> OrbitalResult<NoteDocument> {
        let now = current_unix_timestamp()?;

        self.connection.execute(
            "
            INSERT INTO notes (id, title, body, created_at, updated_at, archived_at)
            VALUES (?1, ?2, ?3, ?4, ?4, NULL)
            ",
            params![note.id.as_str(), note.title, note.body, now],
        )?;

        self.replace_tags(&note.id, &note.tags)?;
        self.record_change(&note.id, "created", now)?;

        self.get(&note.id)?
            .ok_or(OrbitalError::DataInvariant("created note could not be reloaded"))
    }

    pub fn get(&self, id: &NoteId) -> OrbitalResult<Option<NoteDocument>> {
        let mut statement = self.connection.prepare(
            "
            SELECT id, title, body, created_at, updated_at, archived_at
            FROM notes
            WHERE id = ?1
            ",
        )?;

        let note = statement
            .query_row(params![id.as_str()], |row| {
                Ok((
                    NoteId::from(row.get::<_, String>(0)?),
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                ))
            })
            .optional()?;

        note.map(|(id, title, body, created_at, updated_at, archived_at)| {
            Ok(NoteDocument {
                summary: NoteSummary {
                    excerpt: note_excerpt(&body),
                    tags: self.load_tags(&id)?,
                    id,
                    title,
                    created_at,
                    updated_at,
                    archived_at,
                },
                body,
            })
        })
        .transpose()
    }

    pub fn list_active(&self) -> OrbitalResult<Vec<NoteSummary>> {
        let mut statement = self.connection.prepare(
            "
            SELECT id, title, body, created_at, updated_at, archived_at
            FROM notes
            WHERE archived_at IS NULL
            ORDER BY updated_at DESC, created_at DESC
            ",
        )?;

        let note_rows = statement.query_map([], |row| {
            Ok((
                NoteId::from(row.get::<_, String>(0)?),
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<i64>>(5)?,
            ))
        })?;

        let mut notes = Vec::new();

        for note_row in note_rows {
            let (id, title, body, created_at, updated_at, archived_at) = note_row?;

            notes.push(NoteSummary {
                excerpt: note_excerpt(&body),
                tags: self.load_tags(&id)?,
                id,
                title,
                created_at,
                updated_at,
                archived_at,
            });
        }

        Ok(notes)
    }

    pub fn save(&self, note: &NoteDocument) -> OrbitalResult<NoteDocument> {
        let now = current_unix_timestamp()?;

        let updated_rows = self.connection.execute(
            "
            UPDATE notes
            SET title = ?1, body = ?2, updated_at = ?3, archived_at = ?4
            WHERE id = ?5
            ",
            params![
                note.summary.title.as_str(),
                note.body.as_str(),
                now,
                note.summary.archived_at,
                note.summary.id.as_str()
            ],
        )?;

        if updated_rows == 0 {
            return Err(OrbitalError::NotFound {
                entity: NOTE_ENTITY_TYPE,
                id: note.summary.id.to_string(),
            });
        }

        self.replace_tags(&note.summary.id, &note.summary.tags)?;
        self.record_change(&note.summary.id, "updated", now)?;

        self.get(&note.summary.id)?
            .ok_or(OrbitalError::DataInvariant("saved note could not be reloaded"))
    }

    pub fn archive(&self, id: &NoteId) -> OrbitalResult<()> {
        let now = current_unix_timestamp()?;

        let updated_rows = self.connection.execute(
            "
            UPDATE notes
            SET archived_at = ?1, updated_at = ?1
            WHERE id = ?2 AND archived_at IS NULL
            ",
            params![now, id.as_str()],
        )?;

        if updated_rows == 0 {
            return Err(OrbitalError::NotFound {
                entity: NOTE_ENTITY_TYPE,
                id: id.to_string(),
            });
        }

        self.record_change(id, "archived", now)?;

        Ok(())
    }

    fn load_tags(&self, id: &NoteId) -> OrbitalResult<Vec<String>> {
        let mut statement = self.connection.prepare(
            "
            SELECT tags.name
            FROM tags
            INNER JOIN entity_tags ON entity_tags.tag_id = tags.id
            WHERE entity_tags.entity_type = ?1 AND entity_tags.entity_id = ?2
            ORDER BY tags.name ASC
            ",
        )?;

        let tag_rows = statement.query_map(params![NOTE_ENTITY_TYPE, id.as_str()], |row| row.get(0))?;
        let mut tags = Vec::new();

        for tag in tag_rows {
            tags.push(tag?);
        }

        Ok(tags)
    }

    fn replace_tags(&self, id: &NoteId, tags: &[String]) -> OrbitalResult<()> {
        self.connection.execute(
            "
            DELETE FROM entity_tags
            WHERE entity_type = ?1 AND entity_id = ?2
            ",
            params![NOTE_ENTITY_TYPE, id.as_str()],
        )?;

        for tag in normalized_tags(tags) {
            self.connection.execute(
                "
                INSERT INTO tags (name)
                VALUES (?1)
                ON CONFLICT(name) DO NOTHING
                ",
                params![tag.as_str()],
            )?;

            self.connection.execute(
                "
                INSERT INTO entity_tags (entity_type, entity_id, tag_id, created_at)
                SELECT ?1, ?2, id, unixepoch()
                FROM tags
                WHERE name = ?3
                ",
                params![NOTE_ENTITY_TYPE, id.as_str(), tag.as_str()],
            )?;
        }

        Ok(())
    }

    fn record_change(&self, id: &NoteId, change_kind: &str, changed_at: i64) -> OrbitalResult<()> {
        self.connection.execute(
            "
            INSERT INTO change_log (entity_type, entity_id, change_kind, changed_at)
            VALUES (?1, ?2, ?3, ?4)
            ",
            params![NOTE_ENTITY_TYPE, id.as_str(), change_kind, changed_at],
        )?;

        Ok(())
    }
}

fn current_unix_timestamp() -> OrbitalResult<i64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64)
}

fn normalized_tags(tags: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();

    for tag in tags {
        let trimmed = tag.trim();

        if trimmed.is_empty() {
            continue;
        }

        if normalized.iter().any(|existing| existing == trimmed) {
            continue;
        }

        normalized.push(trimmed.to_string());
    }

    normalized
}
