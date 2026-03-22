use rusqlite::Connection;

use crate::db::migrations::run_migrations;
use crate::error::OrbitalResult;
use crate::paths::OrbitalPaths;

#[derive(Debug)]
pub struct OrbitalDatabase {
    connection: Connection,
}

impl OrbitalDatabase {
    pub fn open(paths: &OrbitalPaths) -> OrbitalResult<Self> {
        paths.create_missing()?;

        let database_path = paths.database_path();
        let mut connection = Connection::open(database_path)?;

        configure_connection(&connection)?;
        run_migrations(&mut connection)?;

        Ok(Self { connection })
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}

fn configure_connection(connection: &Connection) -> OrbitalResult<()> {
    connection.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        ",
    )?;

    Ok(())
}
