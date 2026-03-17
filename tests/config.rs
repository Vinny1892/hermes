use hermes::server::{
    config::{
        self, keys, AdminConfig, DatabaseConfig, HermesConfig, LocalStorageConfig,
        S3StorageConfig, ServerConfig, StorageAppConfig,
    },
    db::test_pool,
};

fn make_config() -> HermesConfig {
    HermesConfig {
        server: ServerConfig::default(),
        database: DatabaseConfig::default(),
        admin: AdminConfig::default(),
        storage: StorageAppConfig::default(),
    }
}

// ── db_get / db_set ───────────────────────────────────────────────────────────

#[tokio::test]
async fn db_get_returns_none_for_missing_key() {
    let pool = test_pool().await;
    assert!(config::db_get(&pool, "no.such.key").await.is_none());
}

#[tokio::test]
async fn db_set_then_get_round_trips() {
    let pool = test_pool().await;
    config::db_set(&pool, "x.key", "hello").await.unwrap();
    assert_eq!(config::db_get(&pool, "x.key").await.as_deref(), Some("hello"));
}

#[tokio::test]
async fn db_set_overwrites_existing_value() {
    let pool = test_pool().await;
    config::db_set(&pool, "dup", "first").await.unwrap();
    config::db_set(&pool, "dup", "second").await.unwrap();
    assert_eq!(config::db_get(&pool, "dup").await.as_deref(), Some("second"));
}

// ── sync_to_db ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn sync_writes_quota_as_human_string() {
    let pool = test_pool().await;
    make_config().sync_to_db(&pool).await.unwrap();
    assert_eq!(
        config::db_get(&pool, keys::STORAGE_DEFAULT_QUOTA).await.as_deref(),
        Some("1GB"),
    );
}

#[tokio::test]
async fn sync_writes_default_local_ratio() {
    let pool = test_pool().await;
    make_config().sync_to_db(&pool).await.unwrap();
    assert_eq!(
        config::db_get(&pool, keys::STORAGE_DEFAULT_LOCAL_RATIO).await.as_deref(),
        Some("100"),
    );
}

#[tokio::test]
async fn sync_writes_local_path_when_configured() {
    let pool = test_pool().await;
    let mut cfg = make_config();
    cfg.storage.local = Some(LocalStorageConfig { path: "/data/uploads".to_owned() });
    cfg.sync_to_db(&pool).await.unwrap();
    assert_eq!(
        config::db_get(&pool, keys::STORAGE_LOCAL_PATH).await.as_deref(),
        Some("/data/uploads"),
    );
}

#[tokio::test]
async fn sync_skips_s3_keys_when_not_configured() {
    let pool = test_pool().await;
    let mut cfg = make_config();
    cfg.storage.s3 = None;
    cfg.sync_to_db(&pool).await.unwrap();
    assert!(config::db_get(&pool, keys::STORAGE_S3_BUCKET).await.is_none());
    assert!(config::db_get(&pool, keys::STORAGE_S3_REGION).await.is_none());
}

#[tokio::test]
async fn sync_writes_s3_keys_when_configured() {
    let pool = test_pool().await;
    let mut cfg = make_config();
    cfg.storage.s3 = Some(S3StorageConfig {
        bucket: "my-bucket".to_owned(),
        region: "us-east-1".to_owned(),
        endpoint: None,
        access_key_id: "AKID".to_owned(),
        secret_access_key: "secret".to_owned(),
    });
    cfg.sync_to_db(&pool).await.unwrap();
    assert_eq!(
        config::db_get(&pool, keys::STORAGE_S3_BUCKET).await.as_deref(),
        Some("my-bucket"),
    );
    assert_eq!(
        config::db_get(&pool, keys::STORAGE_S3_REGION).await.as_deref(),
        Some("us-east-1"),
    );
    // endpoint was None — should not be written
    assert!(config::db_get(&pool, keys::STORAGE_S3_ENDPOINT).await.is_none());
}

#[tokio::test]
async fn sync_writes_s3_endpoint_when_present() {
    let pool = test_pool().await;
    let mut cfg = make_config();
    cfg.storage.s3 = Some(S3StorageConfig {
        bucket: "b".to_owned(),
        region: "r".to_owned(),
        endpoint: Some("https://minio.example.com".to_owned()),
        access_key_id: "k".to_owned(),
        secret_access_key: "s".to_owned(),
    });
    cfg.sync_to_db(&pool).await.unwrap();
    assert_eq!(
        config::db_get(&pool, keys::STORAGE_S3_ENDPOINT).await.as_deref(),
        Some("https://minio.example.com"),
    );
}

#[tokio::test]
async fn sync_overwrites_previously_edited_db_value() {
    let pool = test_pool().await;
    make_config().sync_to_db(&pool).await.unwrap();

    // Simulate frontend editing the quota between boots
    config::db_set(&pool, keys::STORAGE_DEFAULT_QUOTA, "500MB").await.unwrap();

    // Next boot: sync should overwrite with the TOML/env-resolved value
    make_config().sync_to_db(&pool).await.unwrap();
    assert_eq!(
        config::db_get(&pool, keys::STORAGE_DEFAULT_QUOTA).await.as_deref(),
        Some("1GB"),
    );
}
