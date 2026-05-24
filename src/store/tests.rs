use super::*;
use chrono::TimeZone;

fn open_test_store() -> SqliteStore {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    // Keep tempdir alive by leaking it — tests are short-lived anyway.
    std::mem::forget(dir);
    SqliteStore::open(path.to_str().unwrap()).unwrap()
}

mod accounts;
mod balances;
mod categories;
mod metadata;
mod schema;
mod transactions;
