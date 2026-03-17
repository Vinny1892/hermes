//! Integration tests for the Axum HTTP handlers.
//!
//! Covers `download_handler`, `share_link_handler`, and `upload_handler` using
//! an in-memory SQLite database and a temporary directory for local storage.
//!
//! Run with:
//!   cargo test --features server --test handlers

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::{get, post},
    Router,
};
use bytes::Bytes;
use chrono::Utc;
use tempfile::tempdir;
use tower::ServiceExt;

use hermes::server::{
    config::StorageAppConfig,
    db::test_pool,
    download::{download_handler, share_link_handler},
    storage::{BackendKind, LocalStorage, StorageRouter},
    upload::{insert_test_file, upload_handler, AppState},
};

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn setup() -> (Router, sqlx::SqlitePool, Arc<StorageRouter>, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let local = Arc::new(LocalStorage::new(dir.path()).await.unwrap());
    let router = Arc::new(StorageRouter::new(
        StorageAppConfig::default(),
        Some(local),
        None,
    ));
    let db = test_pool().await;
    let state = AppState {
        db: db.clone(),
        storage: router.clone(),
    };
    let app = Router::new()
        .route("/f/{file_id}", get(download_handler))
        .route("/share/{token}", get(share_link_handler))
        .route("/api/upload", post(upload_handler))
        .with_state(state);
    (app, db, router, dir)
}

/// Builds a minimal `multipart/form-data` body with a single `file` field.
fn make_multipart(filename: &str, content_type: &str, data: &[u8]) -> (String, Bytes) {
    let boundary = "----TestBoundaryXYZ";
    let header = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
         Content-Type: {content_type}\r\n\r\n"
    );
    let footer = format!("\r\n--{boundary}--\r\n");
    let mut body = header.into_bytes();
    body.extend_from_slice(data);
    body.extend_from_slice(footer.as_bytes());
    (
        format!("multipart/form-data; boundary={boundary}"),
        Bytes::from(body),
    )
}

// ── Download handler ──────────────────────────────────────────────────────────

#[tokio::test]
async fn download_existing_file_returns_200_with_headers() {
    let (app, db, router, _dir) = setup().await;

    let id = "aaaaaaaa-bb00-0000-0000-000000000001";
    insert_test_file(&db, id, "report.pdf", 7).await;
    router
        .backend_for(BackendKind::Local)
        .unwrap()
        .put(id, Bytes::from("PDF content here"))
        .await
        .unwrap();

    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/f/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let headers = resp.headers();
    assert!(
        headers
            .get("content-disposition")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("report.pdf"),
        "Content-Disposition must contain the filename"
    );
    assert_eq!(
        headers.get("content-type").unwrap(),
        "application/octet-stream"
    );
}

#[tokio::test]
async fn download_expired_file_returns_404() {
    let (app, db, router, _dir) = setup().await;

    let id = "aaaaaaaa-bb00-0000-0000-000000000002";
    insert_test_file(&db, id, "stale.txt", -1).await; // expired yesterday
    router
        .backend_for(BackendKind::Local)
        .unwrap()
        .put(id, Bytes::from("old data"))
        .await
        .unwrap();

    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/f/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn download_unknown_id_returns_404() {
    let (app, _db, _router, _dir) = setup().await;

    let id = uuid::Uuid::new_v4();
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/f/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Share-link handler ────────────────────────────────────────────────────────

#[tokio::test]
async fn share_link_valid_token_redirects_to_download_page() {
    let (app, db, _router, _dir) = setup().await;

    let file_id = "aaaaaaaa-bb00-0000-0000-000000000003";
    insert_test_file(&db, file_id, "shared.zip", 7).await;

    let token = "valid-token-abc";
    let now = Utc::now().to_rfc3339();
    let expires = (Utc::now() + chrono::Duration::minutes(10)).to_rfc3339();
    sqlx::query(
        "INSERT INTO share_links (token, file_id, created_at, expires_at) VALUES (?, ?, ?, ?)",
    )
    .bind(token)
    .bind(file_id)
    .bind(&now)
    .bind(&expires)
    .execute(&db)
    .await
    .unwrap();

    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/share/{token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // share_link_handler issues a 307 Temporary Redirect to /d/{file_id}.
    assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);
    let location = resp
        .headers()
        .get("location")
        .expect("location header must be present")
        .to_str()
        .unwrap();
    assert!(
        location.contains(file_id),
        "redirect location must contain the file_id; got: {location}"
    );
}

#[tokio::test]
async fn share_link_expired_token_returns_404() {
    let (app, db, _router, _dir) = setup().await;

    let file_id = "aaaaaaaa-bb00-0000-0000-000000000004";
    insert_test_file(&db, file_id, "old.zip", 7).await;

    let token = "expired-token-xyz";
    let past = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
    sqlx::query(
        "INSERT INTO share_links (token, file_id, created_at, expires_at) VALUES (?, ?, ?, ?)",
    )
    .bind(token)
    .bind(file_id)
    .bind(&past)
    .bind(&past)
    .execute(&db)
    .await
    .unwrap();

    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/share/{token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn share_link_unknown_token_returns_404() {
    let (app, _db, _router, _dir) = setup().await;

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/share/no-such-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Upload handler ────────────────────────────────────────────────────────────

#[tokio::test]
async fn upload_stores_file_and_returns_ok() {
    let (app, _db, _router, _dir) = setup().await;

    let (ct, body) = make_multipart("hello.txt", "text/plain", b"hello world");

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/upload")
                .header("content-type", ct)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn upload_without_file_field_returns_400() {
    let (app, _db, _router, _dir) = setup().await;

    // Send a multipart body with no `file` field — only an unrelated field.
    let boundary = "----TestBoundaryXYZ";
    let body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"other\"\r\n\r\n\
         value\r\n\
         --{boundary}--\r\n"
    );

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/upload")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn upload_zero_byte_file_returns_ok() {
    let (app, _db, _router, _dir) = setup().await;

    let (ct, body) = make_multipart("empty.bin", "application/octet-stream", b"");

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/upload")
                .header("content-type", ct)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}
