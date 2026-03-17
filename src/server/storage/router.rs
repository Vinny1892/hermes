//! Storage router that distributes uploads across local and S3 backends.
//!
//! When both backends are configured the router uses each user's quota and
//! local/S3 ratio to decide where a file lands. When only one backend is
//! configured all traffic goes there.

use std::sync::Arc;

use sqlx::SqlitePool;

use crate::server::config::{self, QuotaBytes, StorageAppConfig};

use super::{LocalStorage, S3Storage, StorageBackend, StorageError};

// ── BackendKind ───────────────────────────────────────────────────────────────

/// Identifies which physical backend stores a file.
///
/// The string representation (`"local"` / `"s3"`) is the value stored in the
/// `files.backend` database column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Local,
    S3,
}

impl BackendKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BackendKind::Local => "local",
            BackendKind::S3 => "s3",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "local" => Some(BackendKind::Local),
            "s3" => Some(BackendKind::S3),
            _ => None,
        }
    }
}

// ── StorageRouter ─────────────────────────────────────────────────────────────

/// Routes file operations to the appropriate storage backend.
#[derive(Clone)]
pub struct StorageRouter {
    config: StorageAppConfig,
    local: Option<Arc<LocalStorage>>,
    s3: Option<Arc<S3Storage>>,
}

impl StorageRouter {
    /// Build a router from optional backend instances.
    ///
    /// At least one of `local` / `s3` must be `Some`, otherwise uploads will
    /// always fail.
    pub fn new(
        config: StorageAppConfig,
        local: Option<Arc<LocalStorage>>,
        s3: Option<Arc<S3Storage>>,
    ) -> Self {
        Self { config, local, s3 }
    }

    /// Decide which backend to use for a new upload and return it together with
    /// the [`BackendKind`] token that must be saved in the database.
    ///
    /// Routing logic:
    /// 1. Resolve this user's quota and `local_ratio` from the DB (or fall back
    ///    to the global defaults in `config`).
    /// 2. If a `backend_override` is set for the user, honour it (subject to
    ///    quota).
    /// 3. Otherwise run the automatic ratio-based selection:
    ///    * If only one backend is configured, use it.
    ///    * Otherwise pick based on which side still has headroom according to
    ///      the ratio; spill to the other side if the primary side is full.
    ///
    /// Returns `StorageError::Other` with a descriptive message when all
    /// backends are full or unavailable.
    pub async fn route_upload(
        &self,
        db: &SqlitePool,
        user_id: Option<&str>,
        size: u64,
    ) -> Result<(BackendKind, Arc<dyn StorageBackend>), StorageError> {
        let (quota, local_ratio, backend_override) =
            self.resolve_user_config(db, user_id).await;

        // Honour explicit backend override
        if let Some(ov) = backend_override.as_deref() {
            return match ov {
                "local" => self
                    .pick_local(db, user_id, &quota, size)
                    .await
                    .map(|b| (BackendKind::Local, b)),
                "s3" => self
                    .pick_s3(db, user_id, &quota, size)
                    .await
                    .map(|b| (BackendKind::S3, b)),
                other => Err(StorageError::Other(format!(
                    "unknown backend_override: {other}"
                ))),
            };
        }

        // Automatic routing based on backend availability and ratio
        match (self.local.is_some(), self.s3.is_some()) {
            (true, false) => self
                .pick_local(db, user_id, &quota, size)
                .await
                .map(|b| (BackendKind::Local, b)),
            (false, true) => self
                .pick_s3(db, user_id, &quota, size)
                .await
                .map(|b| (BackendKind::S3, b)),
            (true, true) => {
                let used_local = self.used_bytes(db, user_id, "local").await;
                let used_s3 = self.used_bytes(db, user_id, "s3").await;
                let ratio = local_ratio as u64;

                let local_quota = match &quota {
                    QuotaBytes::Unlimited => QuotaBytes::Unlimited,
                    QuotaBytes::Bytes(total) => QuotaBytes::Bytes(total * ratio / 100),
                };
                let s3_quota = match &quota {
                    QuotaBytes::Unlimited => QuotaBytes::Unlimited,
                    QuotaBytes::Bytes(total) => QuotaBytes::Bytes(total * (100 - ratio) / 100),
                };

                let local_fits = local_quota.has_space(used_local, size);
                let s3_fits = s3_quota.has_space(used_s3, size);

                if ratio > 0 && local_fits {
                    self.pick_local(db, user_id, &quota, size)
                        .await
                        .map(|b| (BackendKind::Local, b))
                } else if s3_fits {
                    self.pick_s3(db, user_id, &quota, size)
                        .await
                        .map(|b| (BackendKind::S3, b))
                } else if local_fits {
                    self.pick_local(db, user_id, &quota, size)
                        .await
                        .map(|b| (BackendKind::Local, b))
                } else {
                    Err(StorageError::Other(
                        "storage quota exceeded on all backends".to_owned(),
                    ))
                }
            }
            (false, false) => Err(StorageError::Other(
                "no storage backend configured".to_owned(),
            )),
        }
    }

    /// Return the backend that stores objects of `kind`, or `None` if that
    /// backend is not configured.
    pub fn backend_for(&self, kind: BackendKind) -> Option<Arc<dyn StorageBackend>> {
        match kind {
            BackendKind::Local => self
                .local
                .as_ref()
                .map(|b| b.clone() as Arc<dyn StorageBackend>),
            BackendKind::S3 => self
                .s3
                .as_ref()
                .map(|b| b.clone() as Arc<dyn StorageBackend>),
        }
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    async fn resolve_user_config(
        &self,
        db: &SqlitePool,
        user_id: Option<&str>,
    ) -> (QuotaBytes, u8, Option<String>) {
        // Global defaults come from DB (seeded from config on boot).
        let global_quota = self.global_quota(db).await;
        let global_ratio = self.global_ratio(db).await;

        let Some(uid) = user_id else {
            return (global_quota, global_ratio, None);
        };

        let row = sqlx::query_as::<_, (Option<i64>, Option<i64>, Option<String>)>(
            "SELECT quota_bytes, local_ratio, backend_override
             FROM user_storage_config WHERE user_id = ?",
        )
        .bind(uid)
        .fetch_optional(db)
        .await
        .unwrap_or(None);

        match row {
            Some((quota_bytes, local_ratio, backend_override)) => {
                let quota = match quota_bytes {
                    Some(b) if b > 0 => QuotaBytes::Bytes(b as u64),
                    Some(_) => QuotaBytes::Unlimited,
                    None => global_quota,
                };
                let ratio = local_ratio
                    .map(|r| r.clamp(0, 100) as u8)
                    .unwrap_or(global_ratio);
                (quota, ratio, backend_override)
            }
            None => (global_quota, global_ratio, None),
        }
    }

    async fn global_quota(&self, db: &SqlitePool) -> QuotaBytes {
        config::db_get(db, config::keys::STORAGE_DEFAULT_QUOTA)
            .await
            .and_then(|v| config::parse_quota_str(&v).ok())
            .unwrap_or_else(|| self.config.default_quota.clone())
    }

    async fn global_ratio(&self, db: &SqlitePool) -> u8 {
        config::db_get(db, config::keys::STORAGE_DEFAULT_LOCAL_RATIO)
            .await
            .and_then(|v| v.parse::<u8>().ok())
            .filter(|&r| r <= 100)
            .unwrap_or(self.config.default_local_ratio)
    }

    async fn used_bytes(&self, db: &SqlitePool, user_id: Option<&str>, backend: &str) -> u64 {
        let now = chrono::Utc::now().to_rfc3339();

        let result: Option<i64> = if let Some(uid) = user_id {
            sqlx::query_scalar(
                "SELECT SUM(size) FROM files
                 WHERE user_id = ? AND backend = ? AND expires_at > ?",
            )
            .bind(uid)
            .bind(backend)
            .bind(&now)
            .fetch_one(db)
            .await
            .unwrap_or(None)
        } else {
            None
        };

        result.unwrap_or(0) as u64
    }

    async fn pick_local(
        &self,
        db: &SqlitePool,
        user_id: Option<&str>,
        quota: &QuotaBytes,
        size: u64,
    ) -> Result<Arc<dyn StorageBackend>, StorageError> {
        let backend = self
            .local
            .as_ref()
            .ok_or_else(|| StorageError::Other("local backend not configured".to_owned()))?;

        if user_id.is_some() {
            let used = self.used_bytes(db, user_id, "local").await
                + self.used_bytes(db, user_id, "s3").await;
            if !quota.has_space(used, size) {
                return Err(StorageError::Other("storage quota exceeded".to_owned()));
            }
        }

        Ok(backend.clone() as Arc<dyn StorageBackend>)
    }

    async fn pick_s3(
        &self,
        db: &SqlitePool,
        user_id: Option<&str>,
        quota: &QuotaBytes,
        size: u64,
    ) -> Result<Arc<dyn StorageBackend>, StorageError> {
        let backend = self
            .s3
            .as_ref()
            .ok_or_else(|| StorageError::Other("S3 backend not configured".to_owned()))?;

        if user_id.is_some() {
            let used = self.used_bytes(db, user_id, "local").await
                + self.used_bytes(db, user_id, "s3").await;
            if !quota.has_space(used, size) {
                return Err(StorageError::Other("storage quota exceeded".to_owned()));
            }
        }

        Ok(backend.clone() as Arc<dyn StorageBackend>)
    }
}
