use super::*;

#[tokio::test]
async fn test_metadata() {
    let store = open_test_store();

    let val = store.get_metadata("alert_last_checked").await.unwrap();
    assert_eq!(val, None);

    store
        .set_metadata("alert_last_checked", "2026-03-27T12:00:00Z")
        .await
        .unwrap();
    let val = store.get_metadata("alert_last_checked").await.unwrap();
    assert_eq!(val, Some("2026-03-27T12:00:00Z".into()));

    store
        .set_metadata("alert_last_checked", "2026-03-28T08:00:00Z")
        .await
        .unwrap();
    let val = store.get_metadata("alert_last_checked").await.unwrap();
    assert_eq!(val, Some("2026-03-28T08:00:00Z".into()));
}
