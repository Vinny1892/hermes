//! File metadata models shared between client and server.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Metadata for a file stored on the server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileRecord {
    /// Unique identifier for the file.
    pub id: Uuid,
    /// Original filename as provided by the uploader.
    pub filename: String,
    /// File size in bytes.
    pub size: i64,
    /// MIME type detected from the upload.
    pub mime_type: String,
    /// Storage backend: `"local"` or `"s3"`.
    pub backend: String,
    /// Key used to locate the file in the storage backend.
    pub storage_key: String,
    /// When the file was uploaded.
    pub created_at: DateTime<Utc>,
    /// When the file will be automatically deleted (7 days after upload).
    pub expires_at: DateTime<Utc>,
}

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
