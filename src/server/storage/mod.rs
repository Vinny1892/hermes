//! Pluggable storage backend abstraction.
//!
//! All file I/O goes through the [`StorageBackend`] trait. The application
//! currently ships [`LocalStorage`]; an S3-compatible backend can be added
//! without touching any upload/download logic.

pub mod local;

pub use local::LocalStorage;

use async_trait::async_trait;
use bytes::Bytes;
use thiserror::Error;

/// Errors that can occur during storage operations.
#[derive(Debug, Error)]
pub enum StorageError {
    /// The requested object does not exist in the backend.
    #[error("object not found: {0}")]
    NotFound(String),

    /// An I/O error occurred (e.g. permission denied, disk full).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A backend-specific error not covered by the variants above.
    #[error("storage error: {0}")]
    Other(String),
}

/// Trait for pluggable file storage backends.
///
/// All methods take `&self` so a shared `Arc<dyn StorageBackend>` can be used
/// across Tokio tasks without a mutex.
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Store `data` under `key`, overwriting any existing object.
    async fn put(&self, key: &str, data: Bytes) -> Result<(), StorageError>;

    /// Retrieve the object stored under `key`.
    ///
    /// Returns [`StorageError::NotFound`] when the key does not exist.
    async fn get(&self, key: &str) -> Result<Bytes, StorageError>;

    /// Delete the object stored under `key`.
    ///
    /// This is a no-op (returns `Ok`) if the key does not exist.
    async fn delete(&self, key: &str) -> Result<(), StorageError>;
}
