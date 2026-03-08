//! File metadata models shared between client and server.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Response returned after a successful upload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UploadResponse {
    /// Unique ID of the stored file.
    pub file_id: Uuid,
    /// Direct download path: `/f/{file_id}`.
    pub download_url: String,
}

/// Response returned after generating a shareable link.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShareLinkResponse {
    /// Short-lived token embedded in the share URL.
    pub token: String,
    /// Full shareable path: `/share/{token}`.
    pub share_url: String,
    /// When this link becomes invalid (10 minutes from creation).
    pub expires_at: DateTime<Utc>,
}

/// Lightweight file info used by the download page.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileInfo {
    pub filename: String,
    pub size: i64,
    pub mime_type: String,
    pub expires_at: DateTime<Utc>,
}
