use anyhow::{Context, Result};
use rusqlite::Connection;

const MIGRATIONS: &[(&str, &str)] = &[
    ("001_init", include_str!("../../migrations/001_init.sql")),
    (
        "002_add_category",
        include_str!("../../migrations/002_add_category.sql"),
    ),
    (
        "003_alert_last_checked",
        include_str!("../../migrations/003_alert_last_checked.sql"),
    ),
    (
        "004_add_account_alias",
        include_str!("../../migrations/004_add_account_alias.sql"),
    ),
];

pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)",
        [],
    )
    .context("create schema_version table")?;

    let current: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    for (version_idx, (name, sql)) in MIGRATIONS.iter().enumerate() {
        let version = (version_idx + 1) as i32;
        if version <= current {
            continue;
        }

        let tx = conn
            .unchecked_transaction()
            .with_context(|| format!("begin migration {name}"))?;

        tx.execute_batch(sql)
            .with_context(|| format!("execute migration {name}"))?;

        tx.execute(
            "INSERT INTO schema_version (version) VALUES (?1)",
            [version],
        )
        .with_context(|| format!("update schema version for {name}"))?;

        tx.commit()
            .with_context(|| format!("commit migration {name}"))?;
    }

    Ok(())
}
