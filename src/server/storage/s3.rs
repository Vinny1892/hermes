//! S3-compatible storage backend backed by the `object_store` crate.
//!
//! Any S3-compatible service (AWS S3, MinIO, Cloudflare R2, …) is supported
//! by setting the optional `endpoint` field in [`S3StorageConfig`].

use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use object_store::{aws::AmazonS3Builder, path::Path as OsPath, ObjectStore};

use crate::server::config::S3StorageConfig;

use super::{StorageBackend, StorageError};

/// Storage backend that reads and writes objects to an S3-compatible bucket.
pub struct S3Storage {
    store: Arc<dyn ObjectStore>,
}

impl S3Storage {
    /// Construct an [`S3Storage`] from the given configuration.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Other`] when the client cannot be initialised
    /// (e.g. invalid credentials or unknown region).
    pub fn new(cfg: &S3StorageConfig) -> Result<Self, StorageError> {
        let mut builder = AmazonS3Builder::new()
            .with_bucket_name(&cfg.bucket)
            .with_region(&cfg.region)
            .with_access_key_id(&cfg.access_key_id)
            .with_secret_access_key(&cfg.secret_access_key);

        if let Some(endpoint) = &cfg.endpoint {
            builder = builder.with_endpoint(endpoint);
        }

        let store = builder
            .build()
            .map_err(|e| StorageError::Other(e.to_string()))?;

        Ok(Self {
            store: Arc::new(store),
        })
    }
}

#[async_trait]
impl StorageBackend for S3Storage {
    async fn put(&self, key: &str, data: Bytes) -> Result<(), StorageError> {
        let path = OsPath::from(key);
        self.store
            .put(&path, data.into())
            .await
            .map(|_| ())
            .map_err(|e| StorageError::Other(e.to_string()))
    }

    async fn get(&self, key: &str) -> Result<Bytes, StorageError> {
        let path = OsPath::from(key);
        match self.store.get(&path).await {
            Ok(result) => result
                .bytes()
                .await
                .map_err(|e| StorageError::Other(e.to_string())),
            Err(object_store::Error::NotFound { .. }) => {
                Err(StorageError::NotFound(key.to_owned()))
            }
            Err(e) => Err(StorageError::Other(e.to_string())),
        }
    }

    async fn delete(&self, key: &str) -> Result<(), StorageError> {
        let path = OsPath::from(key);
        match self.store.delete(&path).await {
            Ok(()) => Ok(()),
            Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(e) => Err(StorageError::Other(e.to_string())),
        }
    }
}
