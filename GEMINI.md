# GEMINI.md — Hermes Project Context

## Project Overview

**Hermes** is a fullstack file-sharing application built with **Rust** and **Dioxus 0.7**. It provides two primary methods for sharing files:
1.  **Server-based Upload/Download:** Files are uploaded to the server via multipart POST requests and stored locally. They are served via streaming and managed with a 7-day TTL.
2.  **P2P Transfer (WebRTC):** Direct peer-to-peer file transfer using WebRTC DataChannels, with the server acting only as a signaling relay.

### Key Technologies
- **Frontend/Backend:** Dioxus 0.7 (Fullstack)
- **Web Framework:** Axum (Server-side)
- **Database:** SQLite (via SQLx) for metadata, share links, and P2P sessions.
- **P2P:** WebRTC DataChannels (Logic in `assets/webrtc.js`).
- **Styling:** Tailwind CSS.

## Architecture & Structure

- `src/main.rs`: Entry point. Launches the Axum server (on server target) or the Dioxus WASM app (on client target).
- `src/api.rs`: Dioxus **Server Functions**. These run on the server but are callable from the client as if they were local async functions.
- `src/app.rs`: Main application component, routing, and shared UI (Navbar).
- `src/pages/`: Individual application views (Home, Download, Receive).
- `src/components/`: Reusable UI components (Uploader, ProgressBar).
- `src/server/`: Exclusive server-side logic (Database, Storage, Signaling, Cleanup).
- `src/models/`: Shared data structures (JSON/Serde) used by both client and server.
- `assets/webrtc.js`: JavaScript implementation of the WebRTC protocol used for P2P transfers.
- `migrations/`: SQL files for SQLite schema management.

## Building and Running

### Development
To run the project with hot-reloading for both the frontend and backend:
```bash
dx serve --platform web
```
The application will be available at `http://localhost:8080`.

### Production Build
To build the optimized WASM and server binary:
```bash
dx build --platform web --release
```

### Server-Only Run
To run the server without the Dioxus CLI (requires a prior build of assets):
```bash
cargo run --features server
```

### Testing
The project includes unit and integration tests. Most require the `server` feature to run.
```bash
# Run all tests
cargo test --features server

# Run specific integration test suites
cargo test --features server --test storage
cargo test --features server --test sessions
```

## Development Conventions

- **Server Functions:** Use `#[server]` in `src/api.rs` for any operation requiring database access or server-side logic.
- **Platform Gating:** Use `#[cfg(target_arch = "wasm32")]` for browser-specific code (e.g., `eval` for JS) and `#[cfg(not(target_arch = "wasm32"))]` (or the `server` feature) for server-side code.
- **Logging:** Use the `tracing` crate. Logs are configured in `main.rs` and filtered via `RUST_LOG`.
- **Database:** Migrations are applied automatically on server startup. Use `sqlx` for queries.
- **Storage:** Local storage is the default. The `StorageBackend` trait in `src/server/storage/mod.rs` allows for future S3-compatible backends.
- **Clean Code:** Adhere to the existing structure where UI logic is separated from server-side infrastructure. Components should remain lightweight, delegating heavy lifting to server functions or JS helpers.
