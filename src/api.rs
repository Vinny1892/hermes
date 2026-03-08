//! Dioxus server functions.
//!
//! This module is compiled on **both** the WASM client and the server target.
//! On the client, the `#[server]` macro replaces the function body with an
//! HTTP request. On the server, the function body runs directly.
//!
//! The server-side implementations access the database via the global pool
//! (see [`crate::server::db::global_pool`]), which is initialised in `main`.

use dioxus::prelude::*;

use crate::models::{CreateSessionResponse, FileInfo, ShareLinkResponse};

// ── File info ─────────────────────────────────────────────────────────────────

/// Returns lightweight metadata for a file by ID.
///
/// Used by the download page to display filename, size, and expiry before the
/// user clicks the download button.
///
/// # Errors
///
/// Returns [`ServerFnError`] if the file is not found or has already expired.
#[server]
pub async fn get_file_info(file_id: String) -> Result<FileInfo, ServerFnError> {
    use chrono::Utc;

    let pool = crate::server::db::global_pool();
    let now = Utc::now().to_rfc3339();

    let row = sqlx::query_as::<_, (String, i64, String, String)>(
        "SELECT filename, size, mime_type, expires_at
         FROM files WHERE id = ? AND expires_at > ?",
    )
    .bind(&file_id)
    .bind(&now)
    .fetch_optional(pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?
    .ok_or_else(|| ServerFnError::new("file not found or expired"))?;

    let expires_at = row
        .3
        .parse::<chrono::DateTime<Utc>>()
        .unwrap_or_else(|_| Utc::now());

    Ok(FileInfo {
        filename: row.0,
        size: row.1,
        mime_type: row.2,
        expires_at,
    })
}

// ── Share links ───────────────────────────────────────────────────────────────

/// Generates a short-lived (10-minute) public download link for `file_id`.
#[server]
pub async fn generate_share_link(file_id: String) -> Result<ShareLinkResponse, ServerFnError> {
    use chrono::Utc;
    use uuid::Uuid;

    let pool = crate::server::db::global_pool();
    let token = Uuid::new_v4().to_string();
    let now = Utc::now();
    let expires_at = now + chrono::Duration::minutes(10);

    sqlx::query(
        "INSERT INTO share_links (token, file_id, created_at, expires_at) VALUES (?, ?, ?, ?)",
    )
    .bind(&token)
    .bind(&file_id)
    .bind(now.to_rfc3339())
    .bind(expires_at.to_rfc3339())
    .execute(pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(ShareLinkResponse {
        share_url: format!("/share/{token}"),
        token,
        expires_at,
    })
}

// ── P2P sessions ──────────────────────────────────────────────────────────────

/// Creates a new P2P signaling session and returns the WebSocket URL.
#[server]
pub async fn create_p2p_session() -> Result<CreateSessionResponse, ServerFnError> {
    let pool = crate::server::db::global_pool();
    let base_url =
        std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_owned());

    crate::server::sessions::create_session(pool, &base_url)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}
