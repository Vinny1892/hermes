//! Authentication helpers: password hashing, admin seeding, login, and session
//! validation.
//!
//! All database access uses `sqlx::query(…)` (no compile-time macros) so the
//! crate builds without `DATABASE_URL` set at compile time.

use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::models::auth::{LoginResponse, Role, UserInfo};

// ── Password hashing ──────────────────────────────────────────────────────────

pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| anyhow!("password hashing failed: {e}"))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

// ── First-run seed ────────────────────────────────────────────────────────────

/// Creates an admin user with the given `email` if the `users` table is empty.
///
/// If `password` is `None`, a random 16-character password is generated.
/// Credentials are printed to the log — change them immediately after the
/// first login.
pub async fn seed_admin_if_empty(
    pool: &SqlitePool,
    email: &str,
    password: Option<&str>,
) -> Result<()> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;

    if count > 0 {
        return Ok(());
    }

    let generated;
    let password = match password {
        Some(p) => p,
        None => {
            generated = random_password(16);
            &generated
        }
    };

    let hash = hash_password(password)?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO users (id, email, password_hash, role, created_at, updated_at)
         VALUES (?, ?, ?, 'ADMIN', ?, ?)",
    )
    .bind(&id)
    .bind(email)
    .bind(&hash)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    tracing::warn!(
        "\n\
         ╔══════════════════════════════════════════════╗\n\
         ║     HERMES — FIRST-RUN ADMIN CREDENTIALS    ║\n\
         ╠══════════════════════════════════════════════╣\n\
         ║  email   : {email:<39}║\n\
         ║  password: {password:<39}║\n\
         ╠══════════════════════════════════════════════╣\n\
         ║  !! Change this password after first login !!║\n\
         ╚══════════════════════════════════════════════╝"
    );

    Ok(())
}

fn random_password(len: usize) -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

// ── Login ─────────────────────────────────────────────────────────────────────

/// Validates credentials and creates a 24-hour session token.
pub async fn login(pool: &SqlitePool, email: &str, password: &str) -> Result<LoginResponse> {
    let row = sqlx::query(
        "SELECT id, email, password_hash, role FROM users WHERE email = ?",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("invalid credentials"))?;

    let stored_hash: String = row.get("password_hash");
    if !verify_password(password, &stored_hash) {
        return Err(anyhow!("invalid credentials"));
    }

    let role_str: String = row.get("role");
    let role = Role::try_from(role_str.as_str()).map_err(|e| anyhow!(e))?;
    let user_id: String = row.get("id");
    let email_out: String = row.get("email");

    let token = Uuid::new_v4().to_string();
    let now = Utc::now();
    let now_str = now.to_rfc3339();
    let expires_at = (now + Duration::hours(24)).to_rfc3339();

    sqlx::query(
        "INSERT INTO user_sessions (token, user_id, created_at, expires_at)
         VALUES (?, ?, ?, ?)",
    )
    .bind(&token)
    .bind(&user_id)
    .bind(&now_str)
    .bind(&expires_at)
    .execute(pool)
    .await?;

    tracing::info!(email = %email_out, role = %role_str, "user logged in");

    Ok(LoginResponse {
        token,
        email: email_out,
        role,
    })
}

// ── Session validation ────────────────────────────────────────────────────────

/// Resolves a session token to the owning user, or errors if expired/unknown.
pub async fn validate_session(pool: &SqlitePool, token: &str) -> Result<UserInfo> {
    let now = Utc::now().to_rfc3339();

    let row = sqlx::query(
        "SELECT u.id, u.email, u.role
         FROM user_sessions s
         JOIN users u ON u.id = s.user_id
         WHERE s.token = ? AND s.expires_at > ?",
    )
    .bind(token)
    .bind(&now)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("session not found or expired"))?;

    let role_str: String = row.get("role");
    let role = Role::try_from(role_str.as_str()).map_err(|e| anyhow!(e))?;

    Ok(UserInfo {
        id: row.get("id"),
        email: row.get("email"),
        role,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::db::test_pool;

    #[tokio::test]
    async fn test_seed_creates_admin() {
        let pool = test_pool().await;
        seed_admin_if_empty(&pool, "admin@hermes.local", None).await.unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_seed_is_idempotent() {
        let pool = test_pool().await;
        seed_admin_if_empty(&pool, "admin@hermes.local", None).await.unwrap();
        seed_admin_if_empty(&pool, "admin@hermes.local", None).await.unwrap(); // second call must not insert

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_hash_and_verify() {
        let hash = hash_password("hunter2").unwrap();
        assert!(verify_password("hunter2", &hash));
        assert!(!verify_password("wrong", &hash));
    }

    #[tokio::test]
    async fn test_login_and_session() {
        let pool = test_pool().await;

        // Insert a user manually so we know the password
        let hash = hash_password("secret").unwrap();
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO users (id, email, password_hash, role, created_at, updated_at)
             VALUES (?, 'user@test.com', ?, 'USER', ?, ?)",
        )
        .bind(&id)
        .bind(&hash)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        let resp = login(&pool, "user@test.com", "secret").await.unwrap();
        assert_eq!(resp.email, "user@test.com");
        assert_eq!(resp.role, Role::User);

        let info = validate_session(&pool, &resp.token).await.unwrap();
        assert_eq!(info.email, "user@test.com");
    }

    #[tokio::test]
    async fn test_invalid_credentials() {
        let pool = test_pool().await;
        seed_admin_if_empty(&pool, "admin@hermes.local", None).await.unwrap();

        // wrong email
        assert!(login(&pool, "nobody@example.com", "pass").await.is_err());
    }
}
