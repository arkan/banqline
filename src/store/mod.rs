mod accounts;
mod balances;
mod categories;
mod metadata;
mod migrations;
pub mod models;
mod transactions;

pub use models::*;

use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};

pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    pub fn open(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path).context("open database")?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=1;")
            .context("set pragmas")?;
        migrations::migrate(&conn).context("run migrations")?;
        Ok(SqliteStore {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

#[cfg(test)]
mod tests;
