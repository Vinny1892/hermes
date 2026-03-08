//! Database initialisation and shared pool type.
//!
//! Uses SQLite via `sqlx`. Migrations in `migrations/` are applied
//! automatically on startup.

use std::sync::OnceLock;

use anyhow::Result;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

/// Global connection pool set once at server startup.
static POOL: OnceLock<SqlitePool> = OnceLock::new();

/// Stores `pool` as the global pool. Panics if called more than once.
pub fn set_global_pool(pool: SqlitePool) {
    POOL.set(pool).expect("global pool already set");
}

/// Returns a reference to the global pool.
///
/// # Panics
///
/// Panics if [`set_global_pool`] has not been called yet.
pub fn global_pool() -> &'static SqlitePool {
    POOL.get().expect("DB pool not initialised — call set_global_pool first")
}

/// Initialises the SQLite connection pool and runs all pending migrations.
///
/// The database path is read from `DATABASE_URL` (e.g. `sqlite:hermes.db`).
/// Defaults to `sqlite:hermes.db` when the variable is not set.
///
/// # Errors
///
/// Returns an error if the database cannot be opened or a migration fails.
pub async fn init_db() -> Result<SqlitePool> {
    // CARGO_MANIFEST_DIR is embedded at compile time so the DB file lands in the
    // project root even when `dx serve` runs the binary from a different CWD.
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| format!("sqlite:{}/hermes.db", env!("CARGO_MANIFEST_DIR")));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

/// Creates an in-memory SQLite pool with migrations applied.
///
/// Used by unit tests (via `#[tokio::test]`) and integration tests in `tests/`.
#[allow(dead_code)]
pub async fn test_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("failed to open in-memory SQLite");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations failed");

    pool
}
