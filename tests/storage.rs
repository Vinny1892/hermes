//! Integration tests for the local storage backend.
//!
//! Run with:
//!   cargo test --test storage
//!
//! These tests exercise the [`LocalStorage`] backend through the
//! [`StorageBackend`] trait, ensuring the abstraction holds.

use bytes::Bytes;
use hermes::server::storage::{LocalStorage, StorageBackend, StorageError};
use tempfile::tempdir;

// ── Happy-path ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn store_and_retrieve_text_file() {
    let dir = tempdir().unwrap();
    let s = LocalStorage::new(dir.path()).await.unwrap();

    s.put("readme.txt", Bytes::from("hello world")).await.unwrap();
    let got = s.get("readme.txt").await.unwrap();
    assert_eq!(got, Bytes::from("hello world"));
}

#[tokio::test]
async fn store_binary_data() {
    let dir = tempdir().unwrap();
    let s = LocalStorage::new(dir.path()).await.unwrap();

    let data: Vec<u8> = (0u8..=255).collect();
    s.put("bin.bin", Bytes::from(data.clone())).await.unwrap();
    assert_eq!(s.get("bin.bin").await.unwrap().to_vec(), data);
}

#[tokio::test]
async fn overwrite_existing_key() {
    let dir = tempdir().unwrap();
    let s = LocalStorage::new(dir.path()).await.unwrap();

    s.put("key", Bytes::from("v1")).await.unwrap();
    s.put("key", Bytes::from("v2")).await.unwrap();
    assert_eq!(s.get("key").await.unwrap(), Bytes::from("v2"));
}

// ── Delete ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_removes_file() {
    let dir = tempdir().unwrap();
    let s = LocalStorage::new(dir.path()).await.unwrap();

    s.put("to-delete", Bytes::from("data")).await.unwrap();
    s.delete("to-delete").await.unwrap();

    assert!(matches!(
        s.get("to-delete").await.unwrap_err(),
        StorageError::NotFound(_)
    ));
}

#[tokio::test]
async fn delete_nonexistent_is_idempotent() {
    let dir = tempdir().unwrap();
    let s = LocalStorage::new(dir.path()).await.unwrap();
    // Should not panic or return an error.
    s.delete("does-not-exist").await.unwrap();
}

// ── Error cases ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn get_missing_key_returns_not_found() {
    let dir = tempdir().unwrap();
    let s = LocalStorage::new(dir.path()).await.unwrap();

    match s.get("missing").await {
        Err(StorageError::NotFound(key)) => assert_eq!(key, "missing"),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

// ── Security ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn path_traversal_is_contained() {
    let dir = tempdir().unwrap();
    let s = LocalStorage::new(dir.path()).await.unwrap();

    // Attempt to write outside the base directory.
    s.put("../../evil.txt", Bytes::from("pwned")).await.unwrap();

    // The file must have been written inside the base dir.
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .collect();
    assert!(!entries.is_empty(), "file should exist inside base dir");

    // The evil.txt file must NOT be in the parent directories.
    assert!(
        !std::path::Path::new("/tmp/evil.txt").exists()
            && !std::path::Path::new("../../evil.txt").exists(),
        "path traversal succeeded!"
    );
}

// ── Concurrency ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn concurrent_puts_do_not_corrupt_data() {
    use std::sync::Arc;

    let dir = tempdir().unwrap();
    let s = Arc::new(LocalStorage::new(dir.path()).await.unwrap());

    let handles: Vec<_> = (0..20)
        .map(|i| {
            let s = s.clone();
            tokio::spawn(async move {
                let key = format!("file-{i}");
                let data = Bytes::from(format!("content-{i}"));
                s.put(&key, data.clone()).await.unwrap();
                let got = s.get(&key).await.unwrap();
                assert_eq!(got, data);
            })
        })
        .collect();

    for h in handles {
        h.await.unwrap();
    }
}
