# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**Hermes** is a P2P + server-cached file sharing application built with [Dioxus 0.7](https://dioxuslabs.com/learn/0.7) fullstack and Rust. Users can share files either by uploading to the server (7-day TTL, shareable links) or via direct WebRTC P2P transfer. Targets web (default), desktop, and mobile from a single codebase.

## Commands

```sh
# Serve with hot-reload (builds WASM frontend + server)
dx serve --platform web

# Build for production
cargo build --features server --release

# Run all tests (server feature required for DB/storage tests)
cargo test --features server

# Run a specific test suite
cargo test --features server --test storage
cargo test --features server --test sessions

# Lint
cargo clippy --features server
```

Install the `dx` CLI: `curl -sSL http://dioxus.dev/install.sh | sh`

Tailwind CSS is compiled automatically by `dx serve` (detects `tailwind.css` in project root).

## Architecture

### Module layout

```
src/
  main.rs          — entry point; server Axum setup OR dioxus::launch(App)
  lib.rs           — re-exports for integration tests
  api.rs           — Dioxus server functions (compiled on client + server)
  app.rs           — Route enum, root App component, Navbar layout
  models/          — shared types: FileRecord, UploadResponse, SignalMessage, …
  components/      — reusable UI: FileUploader, ProgressBar
  pages/           — route components: Home, Download, Receive
  server/          — server-only modules (cfg(not(target_arch = "wasm32")))
    db.rs          — SQLite pool init + global pool accessor
    storage/       — StorageBackend trait + LocalStorage implementation
    upload.rs      — POST /api/upload Axum handler + insert_test_file helper
    download.rs    — GET /f/:file_id + GET /share/:token handlers
    sessions.rs    — P2P session CRUD (create, get, close, purge)
    signaling.rs   — WebRTC relay over WebSocket + SignalingRegistry
    cleanup.rs     — background task: purge expired files & sessions hourly
migrations/        — sqlx SQL migrations (applied automatically at startup)
assets/
  webrtc.js        — browser WebRTC + DataChannel file-transfer logic
  main.css         — base styles
```

### Route tree

```
/                    → Home      (mode selector + FileUploader)
/f/:file_id          → Download  (file info + download button)
/receive/:session_id → Receive   (P2P receiver, boots webrtc.js)
```

### Server HTTP endpoints (custom Axum routes)

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/upload` | Multipart upload → returns `{ file_id, download_url }` |
| `GET`  | `/f/:file_id` | Stream file with `Content-Disposition: attachment` |
| `GET`  | `/share/:token` | Resolve 10-min share link → 307 redirect to `/f/…` |
| `GET`  | `/ws/signal/:session_id` | WebRTC signaling WebSocket relay |

### Dioxus server functions (`src/api.rs`)

Available on both WASM and server. On the client they become HTTP calls; on the server they run directly using the global pool.

| Function | Description |
|----------|-------------|
| `get_file_info(file_id)` | Returns `FileInfo` for the download page |
| `generate_share_link(file_id)` | Creates a 10-min share token |
| `create_p2p_session()` | Creates a signaling session, returns WS URL |

### State management

- The SQLite pool is initialised once in `main` via `server::db::init_db()` and stored in a `OnceLock` (`server::db::global_pool()`). Server functions access it through this global.
- Axum handlers receive state via `State<AppState>` (contains pool + `Arc<dyn StorageBackend>`).
- The `SignalingRegistry` (WebRTC relay map) is shared across handlers via Axum `State`.

### WebRTC P2P (`assets/webrtc.js`)

The Rust layer only handles signaling session lifecycle. The actual WebRTC connection and file chunking happens entirely in JavaScript:

- `startP2pSender(signalUrl)` — called from Dioxus `use_effect` on the Home page
- `startP2pReceiver(sessionId)` — called on the Receive page
- Stop-and-wait flow control: sender waits for `ack` per 64 KB chunk before sending the next
- Fallback order: direct P2P → STUN NAT traversal → TURN relay

### Cross-compilation notes

- `#[cfg(target_arch = "wasm32")]` guards client-only code (eval calls, JS upload)
- `#[cfg(not(target_arch = "wasm32"))]` gates the entire `server/` module
- Server functions in `api.rs` are not cfg-gated — the `#[server]` macro splits them

## Dioxus 0.7 API Notes

- `cx`, `Scope`, `use_state` are **gone**.
- State: `use_signal(|| init)` — read with `signal()` or `.read()`, write with `.write()`.
- Derived values: `use_memo(move || ...)`.
- Async data: `use_resource(move || async move { ... })` returns `None` while loading.
- For SSR hydration: use `use_server_future` instead of `use_resource`.
- Props: owned types only (`String`, not `&str`), must impl `PartialEq + Clone`.
- `#[component]` on a function with typed args — do NOT also define a separate Props struct.
- Shared state: `use_context_provider(|| val)` (parent) + `use_context::<T>()` (child).
- `eval(script)` is only available on WASM; guard with `#[cfg(target_arch = "wasm32")]` in code that also compiles on the server.
- `use_effect` takes a sync closure; use `spawn(async { … })` inside for async work.

## Clippy Rules

`clippy.toml` forbids holding `GenerationalRef`, `GenerationalRefMut`, or `WriteLock` across `.await` points — this panics at runtime. Always drop signal borrows before awaiting.

## Database

SQLite via `sqlx` (no compile-time macros — uses `sqlx::query(...)` to avoid requiring `DATABASE_URL` at build time). Migrations are in `migrations/` and run automatically on startup.

For tests: `server::db::test_pool()` creates an in-memory DB with all migrations applied.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | `sqlite:hermes.db` | SQLite file path |
| `PORT` | `8080` | HTTP listen port |
| `BASE_URL` | `http://localhost:8080` | Used to build WebSocket URLs in P2P sessions |
| `RUST_LOG` | — | Log filter (e.g. `hermes=debug`) |
