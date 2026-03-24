use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::snapshot::{
    NewSnapshot, SnapshotId, SnapshotKind, SnapshotSummary, SNAPSHOT_ENTITY_TYPE,
};
use crate::error::{OrbitalError, OrbitalResult};

pub struct SnapshotRepository<'connection> {
    connection: &'connection Connection,
}

impl<'connection> SnapshotRepository<'connection> {
    pub fn new(connection: &'connection Connection) -> Self {
        Self { connection }
    }

    pub fn create(&self, snapshot: NewSnapshot) -> OrbitalResult<SnapshotSummary> {
        let now = current_unix_timestamp()?;

        self.connection.execute(
            "
            INSERT INTO snapshots (id, title, kind, source, file_path, mime_type, created_at, updated_at, archived_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, NULL)
            ",
            params![
                snapshot.id.as_str(),
                snapshot.title,
                snapshot.kind.as_str(),
                snapshot.source,
                snapshot.file_path,
                snapshot.mime_type,
                now
            ],
        )?;

        self.replace_tags(&snapshot.id, &snapshot.tags)?;
        self.record_change(&snapshot.id, "created", now)?;

        self.get(&snapshot.id)?
            .ok_or(OrbitalError::DataInvariant("created snapshot could not be reloaded"))
    }

    pub fn get(&self, id: &SnapshotId) -> OrbitalResult<Option<SnapshotSummary>> {
        let mut statement = self.connection.prepare(
            "
            SELECT id, title, kind, source, file_path, mime_type, created_at, updated_at, archived_at
            FROM snapshots
            WHERE id = ?1
            ",
        )?;

        let snapshot = statement
            .query_row(params![id.as_str()], |row| {
                Ok((
                    SnapshotId::from(row.get::<_, String>(0)?),
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                ))
            })
            .optional()?;

        snapshot
            .map(
                |(
                    id,
                    title,
                    stored_kind,
                    source,
                    file_path,
                    mime_type,
                    created_at,
                    updated_at,
                    archived_at,
                )| {
                    Ok(SnapshotSummary {
                        tags: self.load_tags(&id)?,
                        id,
                        title,
                        kind: SnapshotKind::from_stored(&stored_kind)?,
                        source,
                        file_path,
                        mime_type,
                        created_at,
                        updated_at,
                        archived_at,
                    })
                },
            )
            .transpose()
    }

    pub fn list_active(&self) -> OrbitalResult<Vec<SnapshotSummary>> {
        let mut statement = self.connection.prepare(
            "
            SELECT id, title, kind, source, file_path, mime_type, created_at, updated_at, archived_at
            FROM snapshots
            WHERE archived_at IS NULL
            ORDER BY updated_at DESC, created_at DESC, title ASC
            ",
        )?;

        let snapshot_rows = statement.query_map([], |row| {
            Ok((
                SnapshotId::from(row.get::<_, String>(0)?),
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, Option<i64>>(8)?,
            ))
        })?;

        let mut snapshots = Vec::new();

        for snapshot_row in snapshot_rows {
            let (
                id,
                title,
                stored_kind,
                source,
                file_path,
                mime_type,
                created_at,
                updated_at,
                archived_at,
            ) = snapshot_row?;

            snapshots.push(SnapshotSummary {
                tags: self.load_tags(&id)?,
                id,
                title,
                kind: SnapshotKind::from_stored(&stored_kind)?,
                source,
                file_path,
                mime_type,
                created_at,
                updated_at,
                archived_at,
            });
        }

        Ok(snapshots)
    }

    pub fn archive(&self, id: &SnapshotId) -> OrbitalResult<()> {
        let now = current_unix_timestamp()?;

        let updated_rows = self.connection.execute(
            "
            UPDATE snapshots
            SET archived_at = ?1, updated_at = ?1
            WHERE id = ?2 AND archived_at IS NULL
            ",
            params![now, id.as_str()],
        )?;

        if updated_rows == 0 {
            return Err(OrbitalError::NotFound {
                entity: SNAPSHOT_ENTITY_TYPE,
                id: id.to_string(),
            });
        }

        self.record_change(id, "archived", now)?;
        Ok(())
    }

    fn load_tags(&self, id: &SnapshotId) -> OrbitalResult<Vec<String>> {
        let mut statement = self.connection.prepare(
            "
            SELECT tags.name
            FROM tags
            INNER JOIN entity_tags ON entity_tags.tag_id = tags.id
            WHERE entity_tags.entity_type = ?1 AND entity_tags.entity_id = ?2
            ORDER BY tags.name ASC
            ",
        )?;

        let tag_rows =
            statement.query_map(params![SNAPSHOT_ENTITY_TYPE, id.as_str()], |row| row.get(0))?;
        let mut tags = Vec::new();

        for tag in tag_rows {
            tags.push(tag?);
        }

        Ok(tags)
    }

    fn replace_tags(&self, id: &SnapshotId, tags: &[String]) -> OrbitalResult<()> {
        self.connection.execute(
            "
            DELETE FROM entity_tags
            WHERE entity_type = ?1 AND entity_id = ?2
            ",
            params![SNAPSHOT_ENTITY_TYPE, id.as_str()],
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
                params![SNAPSHOT_ENTITY_TYPE, id.as_str(), tag.as_str()],
            )?;
        }

        Ok(())
    }

    fn record_change(
        &self,
        id: &SnapshotId,
        change_kind: &str,
        changed_at: i64,
    ) -> OrbitalResult<()> {
        self.connection.execute(
            "
            INSERT INTO change_log (entity_type, entity_id, change_kind, changed_at)
            VALUES (?1, ?2, ?3, ?4)
            ",
            params![SNAPSHOT_ENTITY_TYPE, id.as_str(), change_kind, changed_at],
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
