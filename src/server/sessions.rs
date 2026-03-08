//! P2P session management.
//!
//! A session represents a rendezvous point for two peers that want to
//! establish a direct WebRTC connection. The sender creates a session, shares
//! the resulting `session_id` with the receiver (via the shareable link), and
//! both peers connect to the WebSocket signaling endpoint.
//!
//! Sessions that never reach both peers expire after **10 minutes**.

use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::models::{CreateSessionResponse, P2pSession, SessionState};

/// Creates a new signaling session and persists it to the database.
///
/// The caller should share `CreateSessionResponse::session_id` with the
/// remote peer so they can connect to the signaling WebSocket.
///
/// # Errors
///
/// Returns a [`sqlx::Error`] if the insert fails.
pub async fn create_session(
    db: &SqlitePool,
    base_url: &str,
) -> Result<CreateSessionResponse, sqlx::Error> {
    let id = Uuid::new_v4();
    let id_str = id.to_string();
    let now = Utc::now().to_rfc3339();
    let expires = (Utc::now() + chrono::Duration::minutes(10)).to_rfc3339();

    sqlx::query(
        "INSERT INTO p2p_sessions (id, created_at, expires_at, peer_a_connected, peer_b_connected, state)
         VALUES (?, ?, ?, 0, 0, 'waiting')",
    )
    .bind(&id_str)
    .bind(&now)
    .bind(&expires)
    .execute(db)
    .await?;

    let ws_base = base_url.replace("http://", "ws://").replace("https://", "wss://");

    Ok(CreateSessionResponse {
        session_id: id,
        signal_url: format!("{ws_base}/ws/signal/{id}?role=sender"),
    })
}

/// Returns an active (non-expired, non-closed) session by ID, or `None`.
#[allow(dead_code)]
pub async fn get_active_session(
    db: &SqlitePool,
    session_id: Uuid,
) -> Result<Option<P2pSession>, sqlx::Error> {
    let id_str = session_id.to_string();
    let now = Utc::now().to_rfc3339();

    let row = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, created_at, expires_at, state
         FROM p2p_sessions
         WHERE id = ? AND expires_at > ? AND state != 'closed'",
    )
    .bind(&id_str)
    .bind(&now)
    .fetch_optional(db)
    .await?;

    let (id_s, created_s, expires_s, state_s) = match row {
        Some(r) => r,
        None => return Ok(None),
    };

    let state = match state_s.as_str() {
        "waiting" => SessionState::Waiting,
        "handshaking" => SessionState::Handshaking,
        "connected" => SessionState::Connected,
        _ => SessionState::Closed,
    };

    Ok(Some(P2pSession {
        id: Uuid::parse_str(&id_s).unwrap_or(session_id),
        created_at: created_s
            .parse::<chrono::DateTime<Utc>>()
            .unwrap_or_else(|_| Utc::now()),
        expires_at: expires_s
            .parse::<chrono::DateTime<Utc>>()
            .unwrap_or_else(|_| Utc::now()),
        state,
    }))
}

/// Transitions a session to the `handshaking` state (both peers connected).
#[allow(dead_code)]
pub async fn mark_handshaking(db: &SqlitePool, session_id: Uuid) -> Result<(), sqlx::Error> {
    let id_str = session_id.to_string();
    sqlx::query("UPDATE p2p_sessions SET state = 'handshaking' WHERE id = ?")
        .bind(&id_str)
        .execute(db)
        .await?;
    Ok(())
}

/// Marks a session as `closed`.
///
/// The signaling server calls this when either peer disconnects.
#[allow(dead_code)]
pub async fn close_session(db: &SqlitePool, session_id: Uuid) -> Result<(), sqlx::Error> {
    let id_str = session_id.to_string();
    sqlx::query("UPDATE p2p_sessions SET state = 'closed' WHERE id = ?")
        .bind(&id_str)
        .execute(db)
        .await?;
    Ok(())
}

/// Deletes all sessions whose `expires_at` is in the past.
///
/// Called periodically by the cleanup background task.
///
/// Returns the number of sessions deleted.
pub async fn purge_expired_sessions(db: &SqlitePool) -> Result<u64, sqlx::Error> {
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query("DELETE FROM p2p_sessions WHERE expires_at < ?")
        .bind(&now)
        .execute(db)
        .await?;
    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::db::test_pool;

    #[tokio::test]
    async fn create_and_retrieve_session() {
        let db = test_pool().await;
        let resp = create_session(&db, "http://localhost:8080").await.unwrap();

        let session = get_active_session(&db, resp.session_id)
            .await
            .unwrap()
            .expect("session should exist");

        assert_eq!(session.state, SessionState::Waiting);
        assert!(session.expires_at > Utc::now());
    }

    #[tokio::test]
    async fn closed_session_not_returned() {
        let db = test_pool().await;
        let resp = create_session(&db, "http://localhost").await.unwrap();

        close_session(&db, resp.session_id).await.unwrap();

        let result = get_active_session(&db, resp.session_id).await.unwrap();
        assert!(result.is_none(), "closed session should not be returned");
    }

    #[tokio::test]
    async fn mark_handshaking_changes_state() {
        let db = test_pool().await;
        let resp = create_session(&db, "http://localhost").await.unwrap();

        mark_handshaking(&db, resp.session_id).await.unwrap();

        let session = get_active_session(&db, resp.session_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(session.state, SessionState::Handshaking);
    }

    #[tokio::test]
    async fn purge_removes_expired_sessions() {
        let db = test_pool().await;

        // Insert a session that is already expired.
        let id = Uuid::new_v4().to_string();
        let past = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        sqlx::query(
            "INSERT INTO p2p_sessions (id, created_at, expires_at, peer_a_connected, peer_b_connected, state)
             VALUES (?, ?, ?, 0, 0, 'waiting')",
        )
        .bind(&id)
        .bind(&past)
        .bind(&past)
        .execute(&db)
        .await
        .unwrap();

        let purged = purge_expired_sessions(&db).await.unwrap();
        assert_eq!(purged, 1);

        // Confirm it's gone.
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM p2p_sessions WHERE id = ?")
                .bind(&id)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(row.0, 0);
    }

}
