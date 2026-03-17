use chrono::Utc;
use hermes::server::{auth, db::test_pool};
use uuid::Uuid;

// ── Helpers ────────────────────────────────────────────────────────────────────

async fn insert_user(
    pool: &sqlx::SqlitePool,
    email: &str,
    password: &str,
    role: &str,
) -> String {
    let id = Uuid::new_v4().to_string();
    let hash = auth::hash_password(password).unwrap();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO users (id, email, password_hash, role, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(email)
    .bind(&hash)
    .bind(role)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .unwrap();
    id
}

// ── Seed ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn seed_creates_admin_when_table_empty() {
    let pool = test_pool().await;
    auth::seed_admin_if_empty(&pool, "admin@hermes.local", None).await.unwrap();

    let (email, role): (String, String) =
        sqlx::query_as("SELECT email, role FROM users LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(email, "admin@hermes.local");
    assert_eq!(role, "ADMIN");
}

#[tokio::test]
async fn seed_is_idempotent_when_users_exist() {
    let pool = test_pool().await;
    auth::seed_admin_if_empty(&pool, "admin@hermes.local", None).await.unwrap();
    auth::seed_admin_if_empty(&pool, "admin@hermes.local", None).await.unwrap(); // must not insert a second row

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

// ── Login ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn correct_credentials_return_login_response() {
    let pool = test_pool().await;
    insert_user(&pool, "alice@example.com", "s3cret", "USER").await;

    let resp = auth::login(&pool, "alice@example.com", "s3cret").await.unwrap();

    assert_eq!(resp.email, "alice@example.com");
    assert_eq!(resp.role, hermes::models::auth::Role::User);
    assert!(!resp.token.is_empty());
}

#[tokio::test]
async fn wrong_password_is_rejected() {
    let pool = test_pool().await;
    insert_user(&pool, "bob@example.com", "correct", "USER").await;

    let err = auth::login(&pool, "bob@example.com", "wrong").await;
    assert!(err.is_err());
    // Error message must NOT reveal whether the email exists (prevent enumeration)
    assert_eq!(err.unwrap_err().to_string(), "invalid credentials");
}

#[tokio::test]
async fn unknown_email_is_rejected_with_same_message() {
    let pool = test_pool().await;

    let err = auth::login(&pool, "nobody@example.com", "anything").await;
    assert!(err.is_err());
    assert_eq!(err.unwrap_err().to_string(), "invalid credentials");
}

#[tokio::test]
async fn admin_role_is_parsed_correctly() {
    let pool = test_pool().await;
    insert_user(&pool, "admin@example.com", "pw", "ADMIN").await;

    let resp = auth::login(&pool, "admin@example.com", "pw").await.unwrap();
    assert_eq!(resp.role, hermes::models::auth::Role::Admin);
}

#[tokio::test]
async fn guest_role_is_parsed_correctly() {
    let pool = test_pool().await;
    insert_user(&pool, "guest@example.com", "pw", "GUEST").await;

    let resp = auth::login(&pool, "guest@example.com", "pw").await.unwrap();
    assert_eq!(resp.role, hermes::models::auth::Role::Guest);
}

// ── Session validation ────────────────────────────────────────────────────────

#[tokio::test]
async fn valid_token_resolves_to_user_info() {
    let pool = test_pool().await;
    insert_user(&pool, "carol@example.com", "pass", "USER").await;

    let resp = auth::login(&pool, "carol@example.com", "pass").await.unwrap();
    let info = auth::validate_session(&pool, &resp.token).await.unwrap();

    assert_eq!(info.email, "carol@example.com");
    assert_eq!(info.role, hermes::models::auth::Role::User);
    assert!(!info.id.is_empty());
}

#[tokio::test]
async fn unknown_token_is_rejected() {
    let pool = test_pool().await;

    let err = auth::validate_session(&pool, "this-token-does-not-exist").await;
    assert!(err.is_err());
}

#[tokio::test]
async fn expired_session_is_rejected() {
    let pool = test_pool().await;
    let user_id = insert_user(&pool, "dave@example.com", "pw", "USER").await;

    // Insert a session that expired one second ago
    let expired_at = (Utc::now() - chrono::Duration::seconds(1)).to_rfc3339();
    let now = Utc::now().to_rfc3339();
    let token = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO user_sessions (token, user_id, created_at, expires_at)
         VALUES (?, ?, ?, ?)",
    )
    .bind(&token)
    .bind(&user_id)
    .bind(&now)
    .bind(&expired_at)
    .execute(&pool)
    .await
    .unwrap();

    let err = auth::validate_session(&pool, &token).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn each_login_creates_independent_session_token() {
    let pool = test_pool().await;
    insert_user(&pool, "eve@example.com", "pw", "USER").await;

    let r1 = auth::login(&pool, "eve@example.com", "pw").await.unwrap();
    let r2 = auth::login(&pool, "eve@example.com", "pw").await.unwrap();

    assert_ne!(r1.token, r2.token, "each login must produce a unique token");

    // Both tokens must be independently valid
    assert!(auth::validate_session(&pool, &r1.token).await.is_ok());
    assert!(auth::validate_session(&pool, &r2.token).await.is_ok());
}
