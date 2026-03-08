//! Integration tests for P2P session management.
//!
//! Run with:
//!   cargo test --test sessions
//!
//! Each test uses an in-memory SQLite database so tests are fully isolated
//! and do not require a database file on disk.

use hermes::server::{
    db::test_pool,
    sessions::{
        close_session, create_session, get_active_session, mark_handshaking,
        purge_expired_sessions,
    },
};
use hermes::models::SessionState;
use uuid::Uuid;

const BASE: &str = "http://localhost:8080";

// ── Create ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn create_returns_valid_session_id() {
    let db = test_pool().await;
    let resp = create_session(&db, BASE).await.unwrap();

    assert!(!resp.session_id.is_nil());
    assert!(resp.signal_url.contains(&resp.session_id.to_string()));
}

#[tokio::test]
async fn newly_created_session_is_waiting() {
    let db = test_pool().await;
    let resp = create_session(&db, BASE).await.unwrap();

    let session = get_active_session(&db, resp.session_id)
        .await
        .unwrap()
        .expect("session should exist");

    assert_eq!(session.state, SessionState::Waiting);
}

#[tokio::test]
async fn session_expires_in_the_future() {
    let db = test_pool().await;
    let resp = create_session(&db, BASE).await.unwrap();

    let session = get_active_session(&db, resp.session_id)
        .await
        .unwrap()
        .unwrap();

    assert!(session.expires_at > chrono::Utc::now());
}

// ── State transitions ─────────────────────────────────────────────────────────

#[tokio::test]
async fn mark_handshaking_transitions_state() {
    let db = test_pool().await;
    let resp = create_session(&db, BASE).await.unwrap();
    mark_handshaking(&db, resp.session_id).await.unwrap();

    let session = get_active_session(&db, resp.session_id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(session.state, SessionState::Handshaking);
}

#[tokio::test]
async fn close_session_hides_it_from_get_active() {
    let db = test_pool().await;
    let resp = create_session(&db, BASE).await.unwrap();
    close_session(&db, resp.session_id).await.unwrap();

    let result = get_active_session(&db, resp.session_id).await.unwrap();
    assert!(result.is_none(), "closed session must not be returned");
}

// ── Lookup ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn unknown_session_id_returns_none() {
    let db = test_pool().await;
    let result = get_active_session(&db, Uuid::new_v4()).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn multiple_sessions_are_independent() {
    let db = test_pool().await;

    let a = create_session(&db, BASE).await.unwrap();
    let b = create_session(&db, BASE).await.unwrap();

    close_session(&db, a.session_id).await.unwrap();

    // Session A is closed; session B should still be active.
    assert!(get_active_session(&db, a.session_id).await.unwrap().is_none());
    assert!(get_active_session(&db, b.session_id).await.unwrap().is_some());
}

// ── Purge ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn purge_removes_only_expired_sessions() {
    let db = test_pool().await;

    // Active session (expires in future).
    let active = create_session(&db, BASE).await.unwrap();

    // Manually insert an already-expired session.
    let expired_id = Uuid::new_v4().to_string();
    let past = (chrono::Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
    sqlx::query(
        "INSERT INTO p2p_sessions (id, created_at, expires_at, peer_a_connected, peer_b_connected, state)
         VALUES (?, ?, ?, 0, 0, 'waiting')",
    )
    .bind(&expired_id)
    .bind(&past)
    .bind(&past)
    .execute(&db)
    .await
    .unwrap();

    let purged = purge_expired_sessions(&db).await.unwrap();
    assert_eq!(purged, 1, "only the expired session should be purged");

    // Active session must survive.
    assert!(
        get_active_session(&db, active.session_id)
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn purge_with_no_expired_sessions_returns_zero() {
    let db = test_pool().await;
    create_session(&db, BASE).await.unwrap(); // active
    let count = purge_expired_sessions(&db).await.unwrap();
    assert_eq!(count, 0);
}
