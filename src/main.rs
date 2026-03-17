//! Application entry point.
//!
//! # Server mode
//!
//! Starts an Axum HTTP server on `0.0.0.0:8080` (or configured address).
//! The router exposes:
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
        config::HermesConfig,
        db::{init_db, set_global_pool},
        download::{download_handler, share_link_handler},
        signaling::{signaling_ws_handler, SignalingRegistry},
        storage::{LocalStorage, S3Storage, StorageRouter},
        upload::{upload_handler, AppState},
    };
    use tower_http::cors::CorsLayer;

    // Load .env file if present. Must run BEFORE HermesConfig::load() so
    // env vars from .env are visible. dotenvy never overwrites existing
    // env vars, so `dx serve`'s PORT/IP take precedence automatically.
    let _ = dotenvy::dotenv();

    let cfg = HermesConfig::load();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(cfg.server.log.parse().unwrap_or_else(|_| {
                    "hermes=info".parse().unwrap()
                })),
        )
        .init();

    // Override DATABASE_URL so sqlx picks it up
    std::env::set_var("DATABASE_URL", &cfg.database.url);

    let pool = init_db().await.expect("database init failed");
    set_global_pool(pool.clone());

    seed_admin_if_empty(&pool, &cfg.admin.email, cfg.admin.password.as_deref())
        .await
        .expect("admin seed failed");

    cfg.sync_to_db(&pool)
        .await
        .expect("config sync to db failed");

    // Build storage backends from config
    let local = if let Some(local_cfg) = &cfg.storage.local {
        let backend = LocalStorage::new(&local_cfg.path)
            .await
            .expect("local storage init failed");
        Some(Arc::new(backend))
    } else {
        None
    };

    let s3 = if let Some(s3_cfg) = &cfg.storage.s3 {
        let backend = S3Storage::new(s3_cfg).expect("S3 storage init failed");
        Some(Arc::new(backend))
    } else {
        None
    };

    if local.is_none() && s3.is_none() {
        panic!("no storage backend configured — add [storage.local] or [storage.s3] to hermes.toml");
    }

    let router = Arc::new(StorageRouter::new(cfg.storage.clone(), local, s3));

    let state = AppState {
        db: pool.clone(),
        storage: router.clone(),
    };

    let registry = SignalingRegistry::default();

    tokio::spawn(cleanup::run(pool, router));

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
    // We map HOST → IP so users can set HOST in hermes.toml for production.
    if std::env::var("IP").is_err() {
        std::env::set_var("IP", &cfg.server.host);
    }
    if std::env::var("PORT").is_err() {
        std::env::set_var("PORT", cfg.server.port.to_string());
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
