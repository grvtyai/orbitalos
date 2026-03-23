use rusqlite::{params, Connection};

use crate::error::OrbitalResult;

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "initial_schema",
    sql: include_str!("../../migrations/0001_initial.sql"),
}, Migration {
    version: 2,
    name: "add_note_body_markup",
    sql: include_str!("../../migrations/0002_add_note_body_markup.sql"),
}, Migration {
    version: 3,
    name: "add_note_body_layout",
    sql: include_str!("../../migrations/0003_add_note_body_layout.sql"),
}, Migration {
    version: 4,
    name: "add_note_display_order",
    sql: include_str!("../../migrations/0004_add_note_display_order.sql"),
}];

pub fn run_migrations(connection: &mut Connection) -> OrbitalResult<()> {
    ensure_migration_table(connection)?;

    let current_version: i64 = connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM orbital_schema_migrations",
        [],
        |row| row.get(0),
    )?;

    for migration in MIGRATIONS
        .iter()
        .filter(|migration| migration.version > current_version)
    {
        let transaction = connection.transaction()?;
        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "
            INSERT INTO orbital_schema_migrations (version, name, applied_at)
            VALUES (?1, ?2, unixepoch())
            ",
            params![migration.version, migration.name],
        )?;
        transaction.commit()?;
    }

    Ok(())
}

fn ensure_migration_table(connection: &Connection) -> OrbitalResult<()> {
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS orbital_schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at INTEGER NOT NULL
        );
        ",
    )?;

    Ok(())
}
