use super::*;

#[tokio::test]
async fn new_creates_data_dir_before_opening_database() {
    let cfg = Config::default();
    let data_path = cfg.data_path();

    let app = App::new(cfg).await;

    if let Err(err) = &app {
        panic!("expected App::new to create data dir: {err:#}");
    }
    assert!(data_path.exists());
}
