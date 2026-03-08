//! Background cleanup task.
//!
//! Runs once per hour and removes:
//! * Expired files from the database **and** the storage backend.
//! * Expired share links.
//! * Expired P2P sessions.
//!
//! Start this task once at server startup:
//!
//! ```rust,no_run
//! # #[tokio::main] async fn main() {
//! # use std::sync::Arc;
//! # use hermes::server::{cleanup, db, storage::LocalStorage};
//! let pool = db::init_db().await.unwrap();
//! let storage: Arc<dyn hermes::server::storage::StorageBackend> =
//!     Arc::new(LocalStorage::new("storage/uploads").await.unwrap());
//! tokio::spawn(cleanup::run(pool.clone(), storage.clone()));
//! # }
//! ```

use std::sync::Arc;

use sqlx::SqlitePool;

use super::{sessions::purge_expired_sessions, storage::StorageBackend};

/// Deletes files whose `expires_at` is in the past.
///
/// For each expired record the file is removed from the storage backend first,
/// then the database row is deleted. Expired share links are cleaned up in the
/// same pass.
///
/// Returns the number of file records deleted.
pub async fn purge_expired_files(
    db: &SqlitePool,
    storage: &dyn StorageBackend,
) -> anyhow::Result<u64> {
    let now = chrono::Utc::now().to_rfc3339();

    let expired = sqlx::query_as::<_, (String,)>(
        "SELECT storage_key FROM files WHERE expires_at < ?",
    )
    .bind(&now)
    .fetch_all(db)
    .await?;

    let count = expired.len() as u64;

    for (key,) in &expired {
        // Best-effort: log but don't abort if a single delete fails.
        if let Err(e) = storage.delete(key).await {
            tracing::warn!("failed to delete storage key {key}: {e}");
        }
    }

    sqlx::query("DELETE FROM files WHERE expires_at < ?")
        .bind(&now)
        .execute(db)
        .await?;

    sqlx::query("DELETE FROM share_links WHERE expires_at < ?")
        .bind(&now)
        .execute(db)
        .await?;

    Ok(count)
}

/// Runs the cleanup loop indefinitely, waking every hour.
///
/// This function never returns — run it inside `tokio::spawn`.
pub async fn run(db: SqlitePool, storage: Arc<dyn StorageBackend>) {
    let interval = std::time::Duration::from_secs(3600);
    loop {
        tokio::time::sleep(interval).await;

        match purge_expired_files(&db, storage.as_ref()).await {
            Ok(n) => tracing::info!("cleanup: removed {n} expired files"),
            Err(e) => tracing::error!("cleanup: file purge failed: {e}"),
        }

        match purge_expired_sessions(&db).await {
            Ok(n) => tracing::info!("cleanup: removed {n} expired sessions"),
            Err(e) => tracing::error!("cleanup: session purge failed: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{db::test_pool, upload::insert_test_file};
    use bytes::Bytes;
    use crate::server::storage::LocalStorage;
    use tempfile::tempdir;

    #[tokio::test]
    async fn purge_removes_expired_files_and_storage() {
        let dir = tempdir().unwrap();
        let storage = LocalStorage::new(dir.path()).await.unwrap();
        let db = test_pool().await;

        let id = "aaaaaaaa-0000-0000-0000-000000000001";
        insert_test_file(&db, id, "old.txt", -1).await; // expires yesterday
        storage.put(id, Bytes::from("stale data")).await.unwrap();

        let count = purge_expired_files(&db, &storage).await.unwrap();
        assert_eq!(count, 1);

        // File should be gone from storage.
        let result = storage.get(id).await;
        assert!(result.is_err());

        // Row should be gone from DB.
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM files WHERE id = ?")
                .bind(id)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(row.0, 0);
    }

    #[tokio::test]
    async fn purge_does_not_remove_valid_files() {
        let dir = tempdir().unwrap();
        let storage = LocalStorage::new(dir.path()).await.unwrap();
        let db = test_pool().await;

        let id = "aaaaaaaa-0000-0000-0000-000000000002";
        insert_test_file(&db, id, "fresh.txt", 7).await; // expires in 7 days

        let count = purge_expired_files(&db, &storage).await.unwrap();
        assert_eq!(count, 0);
    }
}
