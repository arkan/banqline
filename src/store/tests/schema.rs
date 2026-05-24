use super::*;

#[test]
fn test_open_creates_schema() {
    let store = open_test_store();
    let conn = store.conn.lock().unwrap();

    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    assert!(tables.contains(&"schema_version".to_string()));
    assert!(tables.contains(&"accounts".to_string()));
    assert!(tables.contains(&"balances".to_string()));
    assert!(tables.contains(&"transactions".to_string()));
    assert!(tables.contains(&"sync_meta".to_string()));
    assert!(tables.contains(&"app_metadata".to_string()));
}

#[test]
fn test_open_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let path_str = path.to_str().unwrap();

    let _s1 = SqliteStore::open(path_str).unwrap();
    let _s2 = SqliteStore::open(path_str).unwrap();
}
