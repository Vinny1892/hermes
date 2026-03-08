//! Integration tests for the WebRTC signaling WebSocket relay.
//!
//! Spins up a real Axum server with the signaling handler and connects
//! two WebSocket clients (sender + receiver) to validate the full flow.
//!
//! Run with:
//!   cargo test --features server --test signaling

use axum::{routing, Router};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use hermes::server::signaling::{signaling_ws_handler, SignalingRegistry};

/// Starts the signaling server on a random port and returns the base URL.
async fn start_server() -> String {
    let registry = SignalingRegistry::default();
    let app: Router = Router::new()
        .route(
            "/ws/signal/{session_id}",
            routing::get(signaling_ws_handler),
        )
        .with_state(registry);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service())
            .await
            .unwrap();
    });

    format!("ws://127.0.0.1:{}", addr.port())
}

/// Helper: connect a WebSocket client with a given role.
async fn connect(
    base: &str,
    session_id: &str,
    role: &str,
) -> tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
> {
    let url = format!("{base}/ws/signal/{session_id}?role={role}");
    let (ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    ws
}

/// Helper: read next text message with a timeout.
async fn recv_text(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Option<String> {
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while let Some(Ok(msg)) = ws.next().await {
            if let Message::Text(text) = msg {
                return Some(text.to_string());
            }
        }
        None
    })
    .await;

    match result {
        Ok(text) => text,
        Err(_) => None, // timeout
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn sender_receives_peer_joined_when_receiver_connects() {
    let base = start_server().await;
    let session = uuid::Uuid::new_v4().to_string();

    let mut sender = connect(&base, &session, "sender").await;
    let mut receiver = connect(&base, &session, "receiver").await;

    // Sender should receive peer-joined.
    let msg = recv_text(&mut sender).await.expect("sender should receive peer-joined");
    let parsed: serde_json::Value = serde_json::from_str(&msg).unwrap();
    assert_eq!(parsed["type"], "peer-joined");

    // Clean up.
    let _ = sender.close(None).await;
    let _ = receiver.close(None).await;
}

#[tokio::test]
async fn receiver_first_sender_still_gets_peer_joined() {
    let base = start_server().await;
    let session = uuid::Uuid::new_v4().to_string();

    // Receiver connects first.
    let mut receiver = connect(&base, &session, "receiver").await;
    // Small delay to ensure receiver is registered.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let mut sender = connect(&base, &session, "sender").await;

    // Sender should still get peer-joined.
    let msg = recv_text(&mut sender).await.expect("sender should receive peer-joined");
    let parsed: serde_json::Value = serde_json::from_str(&msg).unwrap();
    assert_eq!(parsed["type"], "peer-joined");

    let _ = sender.close(None).await;
    let _ = receiver.close(None).await;
}

#[tokio::test]
async fn offer_forwarded_from_sender_to_receiver() {
    let base = start_server().await;
    let session = uuid::Uuid::new_v4().to_string();

    let mut sender = connect(&base, &session, "sender").await;
    let mut receiver = connect(&base, &session, "receiver").await;

    // Consume peer-joined on sender side.
    let _ = recv_text(&mut sender).await;

    // Sender sends an offer.
    let offer = r#"{"type":"offer","sdp":"v=0\r\n..."}"#;
    sender.send(Message::Text(offer.into())).await.unwrap();

    // Receiver should get the offer.
    let msg = recv_text(&mut receiver).await.expect("receiver should get the offer");
    let parsed: serde_json::Value = serde_json::from_str(&msg).unwrap();
    assert_eq!(parsed["type"], "offer");
    assert!(parsed["sdp"].as_str().unwrap().contains("v=0"));

    let _ = sender.close(None).await;
    let _ = receiver.close(None).await;
}

#[tokio::test]
async fn answer_forwarded_from_receiver_to_sender() {
    let base = start_server().await;
    let session = uuid::Uuid::new_v4().to_string();

    let mut sender = connect(&base, &session, "sender").await;
    let mut receiver = connect(&base, &session, "receiver").await;

    // Consume peer-joined.
    let _ = recv_text(&mut sender).await;

    // Receiver sends an answer.
    let answer = r#"{"type":"answer","sdp":"v=0\r\nanswer"}"#;
    receiver.send(Message::Text(answer.into())).await.unwrap();

    // Sender should get the answer.
    let msg = recv_text(&mut sender).await.expect("sender should get the answer");
    let parsed: serde_json::Value = serde_json::from_str(&msg).unwrap();
    assert_eq!(parsed["type"], "answer");

    let _ = sender.close(None).await;
    let _ = receiver.close(None).await;
}

#[tokio::test]
async fn ice_candidates_forwarded_both_directions() {
    let base = start_server().await;
    let session = uuid::Uuid::new_v4().to_string();

    let mut sender = connect(&base, &session, "sender").await;
    let mut receiver = connect(&base, &session, "receiver").await;

    // Consume peer-joined.
    let _ = recv_text(&mut sender).await;

    // Sender sends ICE candidate.
    let ice = r#"{"type":"ice-candidate","candidate":{"candidate":"a]","sdpMid":"0"}}"#;
    sender.send(Message::Text(ice.into())).await.unwrap();
    let msg = recv_text(&mut receiver).await.expect("receiver should get ICE");
    assert!(msg.contains("ice-candidate"));

    // Receiver sends ICE candidate back.
    let ice2 = r#"{"type":"ice-candidate","candidate":{"candidate":"b","sdpMid":"0"}}"#;
    receiver.send(Message::Text(ice2.into())).await.unwrap();
    let msg2 = recv_text(&mut sender).await.expect("sender should get ICE");
    assert!(msg2.contains("ice-candidate"));

    let _ = sender.close(None).await;
    let _ = receiver.close(None).await;
}

#[tokio::test]
async fn disconnecting_peer_sends_bye_to_other() {
    let base = start_server().await;
    let session = uuid::Uuid::new_v4().to_string();

    let mut sender = connect(&base, &session, "sender").await;
    let mut receiver = connect(&base, &session, "receiver").await;

    // Consume peer-joined.
    let _ = recv_text(&mut sender).await;

    // Sender disconnects.
    sender.close(None).await.unwrap();
    drop(sender);

    // Receiver should get a bye.
    let msg = recv_text(&mut receiver).await.expect("receiver should get bye");
    let parsed: serde_json::Value = serde_json::from_str(&msg).unwrap();
    assert_eq!(parsed["type"], "bye");

    let _ = receiver.close(None).await;
}

#[tokio::test]
async fn full_signaling_handshake() {
    let base = start_server().await;
    let session = uuid::Uuid::new_v4().to_string();

    let mut sender = connect(&base, &session, "sender").await;
    let mut receiver = connect(&base, &session, "receiver").await;

    // 1. Sender gets peer-joined.
    let msg = recv_text(&mut sender).await.unwrap();
    assert!(msg.contains("peer-joined"));

    // 2. Sender sends offer.
    sender
        .send(Message::Text(r#"{"type":"offer","sdp":"offer-sdp"}"#.into()))
        .await
        .unwrap();

    // 3. Receiver gets offer.
    let msg = recv_text(&mut receiver).await.unwrap();
    assert!(msg.contains("offer"));

    // 4. Receiver sends answer.
    receiver
        .send(Message::Text(r#"{"type":"answer","sdp":"answer-sdp"}"#.into()))
        .await
        .unwrap();

    // 5. Sender gets answer.
    let msg = recv_text(&mut sender).await.unwrap();
    assert!(msg.contains("answer"));

    // 6. Both exchange ICE candidates.
    sender
        .send(Message::Text(
            r#"{"type":"ice-candidate","candidate":{"candidate":"s1"}}"#.into(),
        ))
        .await
        .unwrap();
    let msg = recv_text(&mut receiver).await.unwrap();
    assert!(msg.contains("ice-candidate"));

    receiver
        .send(Message::Text(
            r#"{"type":"ice-candidate","candidate":{"candidate":"r1"}}"#.into(),
        ))
        .await
        .unwrap();
    let msg = recv_text(&mut sender).await.unwrap();
    assert!(msg.contains("ice-candidate"));

    // 7. Clean disconnect.
    sender.close(None).await.unwrap();
    let msg = recv_text(&mut receiver).await.unwrap();
    assert!(msg.contains("bye"));

    let _ = receiver.close(None).await;
}
