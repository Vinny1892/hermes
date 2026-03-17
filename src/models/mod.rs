//! Data models shared between the client (WASM) and the server.
//!
//! All types in this module implement [`serde::Serialize`] and
//! [`serde::Deserialize`] so they can be sent over the network as JSON or
//! via Dioxus server-function serialisation.

pub mod auth;
pub mod file;
pub mod session;

pub use auth::{LoginResponse, UserInfo};
pub use file::{FileInfo, ShareLinkResponse, UploadResponse};
pub use session::CreateSessionResponse;
#[cfg(feature = "server")]
pub use session::{P2pSession, SessionState, SignalMessage};

/// A single row from the `server_config` table.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct AppConfigEntry {
    pub key: String,
    pub value: String,
}
