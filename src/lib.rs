//! Hermes — file-sharing library crate.
//!
//! Re-exports the public modules so integration tests in `tests/` can import
//! them as `hermes::server::…` and `hermes::models::…`.

pub mod api;
pub mod app;
pub mod components;
pub mod models;
pub mod pages;

#[cfg(not(target_arch = "wasm32"))]
pub mod server;
