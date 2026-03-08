//! File download and share-link resolution handlers.
//!
//! # Endpoints
//!
//! * `GET /f/{file_id}` — streams the file with `Content-Disposition: attachment`.
//! * `GET /share/{token}` — validates the token and redirects to `/f/{file_id}`.
//!
//! Both endpoints return `404 Not Found` when the resource has expired.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use uuid::Uuid;

use super::upload::AppState;

/// Handles `GET /f/{file_id}`.
///
/// Responds with the file bytes, a `Content-Type` matching the stored MIME type,
/// and `Content-Disposition: attachment; filename="<original name>"` so browsers
/// trigger a save-as dialog instead of trying to render the file inline.
///
/// # Errors
///
/// * `404` — file not found or already expired.
/// * `500` — database or storage backend failure.
pub async fn download_handler(
    State(state): State<AppState>,
    Path(file_id): Path<Uuid>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let now = chrono::Utc::now().to_rfc3339();
    let id_str = file_id.to_string();

    let row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT filename, mime_type, storage_key
         FROM files WHERE id = ? AND expires_at > ?",
    )
    .bind(&id_str)
    .bind(&now)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, "file not found or expired".to_owned()))?;

    let (filename, mime_type, storage_key) = row;

    let data = state
        .storage
        .get(&storage_key)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;

    let content_type = mime_type
        .parse()
        .unwrap_or_else(|_| header::HeaderValue::from_static("application/octet-stream"));

    let disposition = format!("attachment; filename=\"{filename}\"")
        .parse()
        .unwrap_or_else(|_| header::HeaderValue::from_static("attachment"));

    let len = data.len().to_string().parse().unwrap();

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, content_type);
    headers.insert(header::CONTENT_DISPOSITION, disposition);
    headers.insert(header::CONTENT_LENGTH, len);

    Ok((headers, data).into_response())
}

/// Handles `GET /share/{token}`.
///
/// Resolves the token to a file ID and issues a `307 Temporary Redirect` to
/// `/f/{file_id}`. Returns `404` if the token has expired or doesn't exist.
pub async fn share_link_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let now = chrono::Utc::now().to_rfc3339();

    let row = sqlx::query_as::<_, (String,)>(
        "SELECT file_id FROM share_links WHERE token = ? AND expires_at > ?",
    )
    .bind(&token)
    .bind(&now)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, "link not found or expired".to_owned()))?;

    Ok(Redirect::temporary(&format!("/d/{}", row.0)).into_response())
}
