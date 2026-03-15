//! Application entry point.
//!
//! # Server mode
//!
//! Starts an Axum HTTP server on `0.0.0.0:8080` (or `$PORT`). The router
//! exposes:
//!
//! * `POST /api/upload`               — multipart file upload
//! * `GET  /f/:file_id`               — streaming file download
//! * `GET  /share/:token`             — share-link redirect
//! * `GET  /ws/signal/:session_id`    — WebRTC signaling WebSocket
//!
//! Dioxus server functions (`get_file_info`, `generate_share_link`,
//! `create_p2p_session`) are available via the fullstack runtime on the
//! standard server-function endpoint.
//!
//! A background task runs hourly to purge expired files and sessions.
//!
//! # Client mode (`feature = "web"`)
//!
//! Calls `dioxus::launch(App)` which mounts the WASM application.

mod api;
mod app;
mod components;
mod models;
mod pages;

#[cfg(all(not(target_arch = "wasm32"), feature = "server"))]
mod server;

use app::App;

fn main() {
    #[cfg(feature = "server")]
    {
        tokio::runtime::Runtime::new()
            .expect("failed to create tokio runtime")
            .block_on(run_server());
    }

    #[cfg(not(feature = "server"))]
    {
        dioxus::launch(App);
    }
}

// ── Server entry point ────────────────────────────────────────────────────────

#[cfg(feature = "server")]
async fn run_server() {
    use std::sync::Arc;

    use axum::{extract::DefaultBodyLimit, routing, Router};
    use dioxus::prelude::{DioxusRouterExt, ServeConfig};
    use server::{
        auth::seed_admin_if_empty,
        cleanup,
        db::{init_db, set_global_pool},
        download::{download_handler, share_link_handler},
        signaling::{signaling_ws_handler, SignalingRegistry},
        storage::LocalStorage,
        upload::{upload_handler, AppState},
    };
    use tower_http::cors::CorsLayer;

    // Load .env file if present. Must run BEFORE tracing_subscriber so
    // RUST_LOG from .env is picked up. dotenvy never overwrites existing
    // env vars, so `dx serve`'s PORT/IP take precedence automatically.
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("hermes=info".parse().unwrap()),
        )
        .init();

    let pool = init_db().await.expect("database init failed");
    set_global_pool(pool.clone());

    seed_admin_if_empty(&pool)
        .await
        .expect("admin seed failed");

    let storage_dir =
        std::env::var("STORAGE_DIR").unwrap_or_else(|_| format!("{}/storage/uploads", env!("CARGO_MANIFEST_DIR")));
    let storage: Arc<dyn server::storage::StorageBackend> = Arc::new(
        LocalStorage::new(&storage_dir)
            .await
            .expect("storage init failed"),
    );

    let state = AppState {
        db: pool.clone(),
        storage: storage.clone(),
    };

    let registry = SignalingRegistry::default();

    tokio::spawn(cleanup::run(pool, storage));

    // Custom API routes (state fully resolved → Router<()>)
    let api_router: Router = Router::new()
        .route("/api/upload", routing::post(upload_handler))
        .route("/f/{file_id}", routing::get(download_handler))
        .route("/share/{token}", routing::get(share_link_handler))
        .with_state(state.clone())
        .route(
            "/ws/signal/{session_id}",
            routing::get(signaling_ws_handler).with_state(registry),
        )
        .layer(CorsLayer::permissive())
        .layer(DefaultBodyLimit::disable());

    // Dioxus fullstack router: serves assets, server functions, and SSR fallback.
    // We merge the API router FIRST so those routes take priority.
    let router = Router::new()
        .serve_dioxus_application(ServeConfig::new(), App)
        .merge(api_router);

    // `fullstack_address_or_localhost` reads IP/PORT set by `dx serve`.
    // In production (no dx), it falls back to IP/PORT env vars or 127.0.0.1:8080.
    // We map HOST → IP so users can set HOST in .env for production.
    if std::env::var("IP").is_err() {
        if let Ok(host) = std::env::var("HOST") {
            std::env::set_var("IP", &host);
        }
    }
    let address = dioxus::cli_config::fullstack_address_or_localhost();
    tracing::info!("hermes listening on http://{address}");

    let listener = tokio::net::TcpListener::bind(address)
        .await
        .unwrap_or_else(|e| panic!("failed to bind to {address}: {e}"));

    axum::serve(listener, router.into_make_service())
        .await
        .expect("server error");
}
