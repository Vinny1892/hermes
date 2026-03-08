//! WebRTC signaling server over WebSocket.
//!
//! The signaling server is a **relay only**: it forwards messages verbatim
//! from one peer to the other. No WebRTC logic runs on the server.
//!
//! # Connection flow
//!
//! ```text
//! Peer A connects → ws://<host>/ws/signal/{session_id}   (slot = 'a')
//! Peer B connects → ws://<host>/ws/signal/{session_id}   (slot = 'b')
//!
//! Peer A sends { "type": "offer",  "sdp": "..." }
//!   → server forwards to Peer B
//! Peer B sends { "type": "answer", "sdp": "..." }
//!   → server forwards to Peer A
//! Both peers exchange ICE candidates through the server
//! P2P connection established → file transfer starts
//! Either peer sends { "type": "bye" } or disconnects
//!   → server notifies the other peer and closes the session
//! ```
//!
//! # Capacity
//!
//! Each `session_id` allows exactly **two** peers. A third connection attempt
//! is rejected immediately.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::models::SignalMessage;

/// An unbounded channel for forwarding WebSocket messages to a peer.
type PeerTx = mpsc::UnboundedSender<Message>;

/// In-memory registry of active signaling sessions.
///
/// Cheap to clone — the inner data is behind an `Arc`.
#[derive(Default, Clone)]
pub struct SignalingRegistry(Arc<Mutex<HashMap<Uuid, SessionSlots>>>);

#[derive(Default)]
struct SessionSlots {
    peer_a: Option<PeerTx>,
    peer_b: Option<PeerTx>,
}

impl SignalingRegistry {
    /// Registers `tx` for `session_id` with an explicit role.
    ///
    /// `is_sender = true` → slot `'a'`; `false` → slot `'b'`.
    ///
    /// Returns the assigned slot, or `None` if that slot is already taken.
    /// When both slots are filled after this registration, `PeerJoined` is
    /// sent to the sender (slot `'a'`) so it creates the WebRTC offer.
    pub fn register(&self, session_id: Uuid, tx: PeerTx, is_sender: bool) -> Option<char> {
        let mut map = self.0.lock().unwrap();
        let slots = map.entry(session_id).or_default();

        if is_sender {
            if slots.peer_a.is_some() {
                tracing::warn!("Session {}: Sender slot already occupied", session_id);
                return None;
            }
            tracing::info!("Session {}: Sender (Peer A) registered", session_id);
            slots.peer_a = Some(tx);
            // If receiver already connected, notify sender immediately.
            if slots.peer_b.is_some() {
                tracing::info!("Session {}: Receiver already present — notifying sender", session_id);
                if let Some(ref peer_a) = slots.peer_a {
                    let msg = Message::Text(
                        serde_json::to_string(&SignalMessage::PeerJoined).unwrap_or_default().into(),
                    );
                    let _ = peer_a.send(msg);
                }
            }
            Some('a')
        } else {
            if slots.peer_b.is_some() {
                tracing::warn!("Session {}: Receiver slot already occupied", session_id);
                return None;
            }
            tracing::info!("Session {}: Receiver (Peer B) registered", session_id);
            slots.peer_b = Some(tx);
            // If sender already connected, notify sender now.
            if let Some(ref peer_a) = slots.peer_a {
                tracing::info!("Session {}: Notifying sender that receiver joined", session_id);
                let msg = Message::Text(
                    serde_json::to_string(&SignalMessage::PeerJoined).unwrap_or_default().into(),
                );
                let _ = peer_a.send(msg);
            }
            Some('b')
        }
    }

    /// Forwards `msg` to the peer opposite `from_slot`.
    pub fn forward(&self, session_id: Uuid, from_slot: char, msg: Message) {
        let map = self.0.lock().unwrap();
        if let Some(slots) = map.get(&session_id) {
            tracing::info!("Session {}: Forwarding message from slot '{}'", session_id, from_slot);
            let target = if from_slot == 'a' {
                slots.peer_b.as_ref()
            } else {
                slots.peer_a.as_ref()
            };
            if let Some(tx) = target {
                let _ = tx.send(msg);
            } else {
                tracing::warn!("Session {}: Target peer not found for slot '{}'", session_id, from_slot);
            }
        }
    }

    /// Removes `slot` from the session and sends a `bye` to the remaining peer.
    ///
    /// When both peers are gone the session entry is removed from the map.
    pub fn remove(&self, session_id: Uuid, slot: char) {
        tracing::info!("Session {}: Removing peer '{}'", session_id, slot);
        let mut map = self.0.lock().unwrap();
        if let Some(slots) = map.get_mut(&session_id) {
            let bye = Message::Text(
                serde_json::to_string(&SignalMessage::Bye).unwrap_or_default().into(),
            );
            if slot == 'a' {
                slots.peer_a = None;
                if let Some(tx) = &slots.peer_b {
                    let _ = tx.send(bye);
                }
            } else {
                slots.peer_b = None;
                if let Some(tx) = &slots.peer_a {
                    let _ = tx.send(bye);
                }
            }
            if slots.peer_a.is_none() && slots.peer_b.is_none() {
                tracing::info!("Session {}: Both peers disconnected, clearing session map", session_id);
                map.remove(&session_id);
            }
        }
    }
}

// ── Axum handler ─────────────────────────────────────────────────────────────

/// Query parameters for the signaling WebSocket endpoint.
#[derive(serde::Deserialize)]
pub struct SignalingQuery {
    role: Option<String>,
}

/// Axum WebSocket upgrade handler for `GET /ws/signal/{session_id}`.
///
/// Upgrades the HTTP connection and spawns the relay loop.
/// The `?role=sender` / `?role=receiver` query parameter determines which
/// slot the peer occupies, preventing the race condition where the receiver
/// connects first and accidentally takes the sender slot.
pub async fn signaling_ws_handler(
    ws: WebSocketUpgrade,
    Path(session_id): Path<Uuid>,
    Query(query): Query<SignalingQuery>,
    State(registry): State<SignalingRegistry>,
) -> impl IntoResponse {
    let is_sender = query.role.as_deref() != Some("receiver");
    tracing::info!(
        "WebSocket upgrade request for session: {} (role: {})",
        session_id,
        if is_sender { "sender" } else { "receiver" }
    );
    ws.on_upgrade(move |socket| relay_loop(socket, session_id, registry, is_sender))
}

async fn relay_loop(socket: WebSocket, session_id: Uuid, registry: SignalingRegistry, is_sender: bool) {
    tracing::info!("Starting relay loop for session: {}", session_id);
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    let slot = match registry.register(session_id, tx, is_sender) {
        Some(s) => s,
        None => return, // slot already taken — reject silently
    };

    let (mut ws_tx, mut ws_rx) = socket.split();

    // Drain the relay channel and write to the WebSocket.
    let write_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Read from the WebSocket and forward to the other peer.
    while let Some(Ok(msg)) = ws_rx.next().await {
        match msg {
            Message::Close(_) => break,
            Message::Text(_) | Message::Binary(_) => {
                registry.forward(session_id, slot, msg);
            }
            _ => {}
        }
    }

    registry.remove(session_id, slot);
    write_task.abort();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_two_peers() {
        let registry = SignalingRegistry::default();
        let id = Uuid::new_v4();

        let (tx_a, _rx_a) = mpsc::unbounded_channel();
        let (tx_b, _rx_b) = mpsc::unbounded_channel();

        assert_eq!(registry.register(id, tx_a, true), Some('a'));
        assert_eq!(registry.register(id, tx_b, false), Some('b'));
    }

    #[test]
    fn receiver_first_then_sender_notified() {
        let registry = SignalingRegistry::default();
        let id = Uuid::new_v4();

        let (tx_a, mut rx_a) = mpsc::unbounded_channel();
        let (tx_b, _rx_b) = mpsc::unbounded_channel();

        // Receiver connects first.
        assert_eq!(registry.register(id, tx_b, false), Some('b'));
        // Sender connects second — should immediately receive PeerJoined.
        assert_eq!(registry.register(id, tx_a, true), Some('a'));

        let msg = rx_a.try_recv().expect("sender should receive PeerJoined");
        if let Message::Text(text) = msg {
            let signal: SignalMessage = serde_json::from_str(&text).unwrap();
            assert!(matches!(signal, SignalMessage::PeerJoined));
        } else {
            panic!("expected text message");
        }
    }

    #[test]
    fn duplicate_sender_rejected() {
        let registry = SignalingRegistry::default();
        let id = Uuid::new_v4();

        let (tx_a1, _) = mpsc::unbounded_channel();
        let (tx_a2, _) = mpsc::unbounded_channel();

        registry.register(id, tx_a1, true);
        assert_eq!(registry.register(id, tx_a2, true), None);
    }

    #[test]
    fn remove_sends_bye_to_other_peer() {
        let registry = SignalingRegistry::default();
        let id = Uuid::new_v4();

        let (tx_a, _rx_a) = mpsc::unbounded_channel();
        let (tx_b, mut rx_b) = mpsc::unbounded_channel();

        registry.register(id, tx_a, true);
        registry.register(id, tx_b, false);
        registry.remove(id, 'a'); // Peer A disconnects

        // Peer B should have received a bye message.
        let msg = rx_b.try_recv().expect("peer B should receive bye");
        if let Message::Text(text) = msg {
            let signal: SignalMessage = serde_json::from_str(&text).unwrap();
            assert!(matches!(signal, SignalMessage::Bye));
        } else {
            panic!("expected text message");
        }
    }

    #[test]
    fn session_removed_after_both_peers_leave() {
        let registry = SignalingRegistry::default();
        let id = Uuid::new_v4();

        let (tx_a, _rx_a) = mpsc::unbounded_channel();
        let (tx_b, _rx_b) = mpsc::unbounded_channel();

        registry.register(id, tx_a, true);
        registry.register(id, tx_b, false);
        registry.remove(id, 'a');
        registry.remove(id, 'b');

        assert!(!registry.0.lock().unwrap().contains_key(&id));
    }

    #[test]
    fn forward_a_to_b() {
        let registry = SignalingRegistry::default();
        let id = Uuid::new_v4();

        let (tx_a, _rx_a) = mpsc::unbounded_channel();
        let (tx_b, mut rx_b) = mpsc::unbounded_channel();

        registry.register(id, tx_a, true);
        registry.register(id, tx_b, false);

        let payload = Message::Text(r#"{"type":"offer","sdp":"v=0"}"#.to_owned().into());
        registry.forward(id, 'a', payload);

        let received = rx_b.try_recv().expect("peer B should receive the message");
        if let Message::Text(t) = received {
            assert!(t.contains("offer"));
        } else {
            panic!("expected text");
        }
    }
}
