//! Data models shared between the client (WASM) and the server.
//!
//! All types in this module implement [`serde::Serialize`] and
//! [`serde::Deserialize`] so they can be sent over the network as JSON or
//! via Dioxus server-function serialisation.

pub mod file;
pub mod session;

pub use file::{FileInfo, FileRecord, ShareLinkResponse, UploadResponse};
pub use session::{CreateSessionResponse, P2pSession, SessionState, SignalMessage};
