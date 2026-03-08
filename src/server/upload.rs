//! File upload handler and share-link generator.
//!
//! # HTTP endpoint
//!
//! `POST /api/upload` accepts a `multipart/form-data` body with a single
//! `file` field. The file is stored in the configured [`StorageBackend`],
//! metadata is persisted to SQLite, and a JSON [`UploadResponse`] is returned.
//!
//! Files expire **7 days** after upload and are removed by the cleanup task.
//!
//! # Share links
//!
//! After upload the client may call [`generate_share_link`] (a Dioxus server
//! function) to obtain a short-lived public URL. Share links expire in
//! **10 minutes**, independently from the file's own TTL.

use std::sync::Arc;

use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    Json,
};
use bytes::Bytes;
use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    models::UploadResponse,
    server::storage::StorageBackend,
};

/// Shared application state injected into Axum handlers.
#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub storage: Arc<dyn StorageBackend>,
}

// ── Axum handler ─────────────────────────────────────────────────────────────

/// Handles `POST /api/upload`.
///
/// Expects a `multipart/form-data` body. The first `file` field is stored;
/// additional fields are ignored. Returns [`UploadResponse`] on success.
///
/// # Errors
///
/// * `400 Bad Request` — malformed multipart body or missing `file` field.
/// * `500 Internal Server Error` — storage or database failure.
pub async fn upload_handler(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, (StatusCode, String)> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    {
        if field.name() != Some("file") {
            continue;
        }

        let filename = field
            .file_name()
            .unwrap_or("unknown")
            .to_owned();
        let content_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_owned();

        let data: Bytes = field
            .bytes()
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

        return persist_upload(&state, filename, content_type, data)
            .await
            .map(Json)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
    }

    Err((StatusCode::BAD_REQUEST, "missing `file` field".to_owned()))
}

// ── Shared helper (also used by the server function below) ───────────────────

/// Stores `data` in the backend and inserts a [`FileRecord`] into the database.
pub(crate) async fn persist_upload(
    state: &AppState,
    filename: String,
    mime_type: String,
    data: Bytes,
) -> anyhow::Result<UploadResponse> {
    let file_id = Uuid::new_v4();
    let key = file_id.to_string();
    let size = data.len() as i64;
    let now = Utc::now().to_rfc3339();
    let expires = (Utc::now() + chrono::Duration::days(7)).to_rfc3339();
    let id_str = file_id.to_string();

    state.storage.put(&key, data).await?;

    sqlx::query(
        "INSERT INTO files (id, filename, size, mime_type, backend, storage_key, created_at, expires_at)
         VALUES (?, ?, ?, ?, 'local', ?, ?, ?)",
    )
    .bind(&id_str)
    .bind(&filename)
    .bind(size)
    .bind(&mime_type)
    .bind(&key)
    .bind(&now)
    .bind(&expires)
    .execute(&state.db)
    .await?;

    Ok(UploadResponse {
        file_id,
        download_url: format!("/f/{file_id}"),
    })
}

// ── Internal helper exposed for tests ────────────────────────────────────────

/// Inserts a file record directly into `pool`.
///
/// Used by unit tests and integration tests that need a pre-existing file in
/// the database without going through the full upload handler.
#[allow(dead_code)]
pub async fn insert_test_file(
    pool: &SqlitePool,
    id: &str,
    filename: &str,
    expires_days: i64,
) {
    let now = Utc::now().to_rfc3339();
    let exp = (Utc::now() + chrono::Duration::days(expires_days)).to_rfc3339();
    sqlx::query(
        "INSERT INTO files (id, filename, size, mime_type, backend, storage_key, created_at, expires_at)
         VALUES (?, ?, 0, 'application/octet-stream', 'local', ?, ?, ?)",
    )
    .bind(id)
    .bind(filename)
    .bind(id)
    .bind(&now)
    .bind(&exp)
    .execute(pool)
    .await
    .unwrap();
}
