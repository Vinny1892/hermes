//! Local filesystem storage backend.
//!
//! Files are written to `<base_path>/<key>`. The key is sanitised to prevent
//! path-traversal attacks before any filesystem operation is performed.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use bytes::Bytes;
use tokio::io::AsyncWriteExt;

use super::{StorageBackend, StorageError};

/// Stores files on the local filesystem under a configurable base directory.
///
/// The base directory is created (including all parents) on construction.
///
/// # Example
///
/// ```rust,no_run
/// # #[tokio::main] async fn main() {
/// use hermes::server::storage::LocalStorage;
/// let storage = LocalStorage::new("storage/uploads").await.unwrap();
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct LocalStorage {
    base_path: PathBuf,
}

impl LocalStorage {
    /// Creates a new [`LocalStorage`] rooted at `base_path`.
    ///
    /// The directory and all parents are created if they do not already exist.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Io`] if the directory cannot be created.
    pub async fn new(base_path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let base_path = base_path.as_ref().to_path_buf();
        tokio::fs::create_dir_all(&base_path).await?;
        Ok(Self { base_path })
    }

    /// Resolves `key` to an absolute path inside [`base_path`].
    ///
    /// Leading slashes are stripped and `..` segments are replaced with `__`
    /// to prevent path traversal.
    fn resolve(&self, key: &str) -> PathBuf {
        let safe = key.trim_start_matches('/').replace("..", "__");
        self.base_path.join(safe)
    }
}

#[async_trait]
impl StorageBackend for LocalStorage {
    async fn put(&self, key: &str, data: Bytes) -> Result<(), StorageError> {
        let path = self.resolve(key);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut file = tokio::fs::File::create(&path).await?;
        file.write_all(&data).await?;
        file.flush().await?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Bytes, StorageError> {
        let path = self.resolve(key);
        tokio::fs::read(&path).await.map(Bytes::from).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StorageError::NotFound(key.to_owned())
            } else {
                StorageError::Io(e)
            }
        })
    }

    async fn delete(&self, key: &str) -> Result<(), StorageError> {
        let path = self.resolve(key);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(StorageError::Io(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn put_and_get_roundtrip() {
        let dir = tempdir().unwrap();
        let s = LocalStorage::new(dir.path()).await.unwrap();

        let data = Bytes::from("hello, hermes!");
        s.put("hello.txt", data.clone()).await.unwrap();

        let retrieved = s.get("hello.txt").await.unwrap();
        assert_eq!(data, retrieved);
    }

    #[tokio::test]
    async fn get_missing_returns_not_found() {
        let dir = tempdir().unwrap();
        let s = LocalStorage::new(dir.path()).await.unwrap();

        let err = s.get("nope.bin").await.unwrap_err();
        assert!(matches!(err, StorageError::NotFound(_)));
    }

    #[tokio::test]
    async fn delete_existing_file() {
        let dir = tempdir().unwrap();
        let s = LocalStorage::new(dir.path()).await.unwrap();

        s.put("bye.txt", Bytes::from("data")).await.unwrap();
        s.delete("bye.txt").await.unwrap();

        assert!(matches!(
            s.get("bye.txt").await.unwrap_err(),
            StorageError::NotFound(_)
        ));
    }

    #[tokio::test]
    async fn delete_nonexistent_is_ok() {
        let dir = tempdir().unwrap();
        let s = LocalStorage::new(dir.path()).await.unwrap();
        s.delete("ghost.txt").await.unwrap(); // must not error
    }

    #[tokio::test]
    async fn path_traversal_stays_inside_base() {
        let dir = tempdir().unwrap();
        let s = LocalStorage::new(dir.path()).await.unwrap();

        s.put("../../etc/shadow", Bytes::from("evil")).await.unwrap();
        let resolved = s.resolve("../../etc/shadow");
        assert!(
            resolved.starts_with(dir.path()),
            "resolved path escaped base dir: {resolved:?}"
        );
    }

    #[tokio::test]
    async fn put_overwrites_existing() {
        let dir = tempdir().unwrap();
        let s = LocalStorage::new(dir.path()).await.unwrap();

        s.put("file.txt", Bytes::from("v1")).await.unwrap();
        s.put("file.txt", Bytes::from("v2")).await.unwrap();

        assert_eq!(s.get("file.txt").await.unwrap(), Bytes::from("v2"));
    }
}
