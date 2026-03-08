//! Server-only modules.
//!
//! Everything in this module is compiled only for the server target
//! (not WASM). The modules are:
//!
//! | Module | Responsibility |
//! |--------|---------------|
//! | [`db`] | SQLite pool initialisation and migrations |
//! | [`storage`] | Pluggable file storage backends |
//! | [`upload`] | Multipart upload handler + share-link generator |
//! | [`download`] | File download + share-link resolution |
//! | [`sessions`] | P2P session CRUD |
//! | [`signaling`] | WebRTC signaling relay over WebSocket |
//! | [`cleanup`] | Background task that deletes expired files/sessions |

pub mod cleanup;
pub mod db;
pub mod download;
pub mod sessions;
pub mod signaling;
pub mod storage;
pub mod upload;
