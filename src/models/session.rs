//! P2P session models shared between client and server.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Current state of a WebRTC signaling session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Waiting for the second peer to join.
    Waiting,
    /// Both peers connected; offer/answer exchange in progress.
    Handshaking,
    /// P2P DataChannel established; file transfer can begin.
    Connected,
    /// Session is over (transfer complete, timeout, or error).
    Closed,
}

/// A P2P signaling session record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct P2pSession {
    /// Session ID shared via the invite link.
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    /// Session expires if both peers don't connect within 10 minutes.
    pub expires_at: DateTime<Utc>,
    pub state: SessionState,
}

/// Returned when a new P2P session is created.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateSessionResponse {
    pub session_id: Uuid,
    /// WebSocket URL the client must connect to: `ws://<host>/ws/signal/{session_id}`.
    pub signal_url: String,
}

/// A WebRTC signaling message exchanged over WebSocket.
///
/// The server acts as a relay: it reads a message from one peer and forwards
/// it verbatim to the other. Both peers serialise/deserialise these as JSON.
///
/// # File-transfer sub-protocol
///
/// Once the DataChannel is open, peers exchange:
/// 1. [`SignalMessage::FileStart`] — metadata before any chunks.
/// 2. [`SignalMessage::Chunk`] — 64 KB chunks (base64-encoded).
/// 3. [`SignalMessage::Ack`] — receiver acknowledges each chunk before the
///    sender sends the next one (stop-and-wait flow control).
/// 4. [`SignalMessage::FileEnd`] — sender signals end of transfer.
///
/// On timeout (3 missed acks), the sender sends [`SignalMessage::Error`] and
/// closes the channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SignalMessage {
    /// Server notifies Peer A that Peer B has connected.
    PeerJoined,
    /// WebRTC offer SDP from Peer A (the sender).
    Offer { sdp: String },
    /// WebRTC answer SDP from Peer B (the receiver).
    Answer { sdp: String },
    /// ICE candidate from either peer.
    IceCandidate { candidate: serde_json::Value },
    /// Peer is disconnecting cleanly.
    Bye,
    /// Begins a file transfer; sent before the first [`Chunk`].
    FileStart {
        name: String,
        size: u64,
        /// Total number of 64 KB chunks.
        total_chunks: u32,
    },
    /// A single 64 KB chunk (base64-encoded data).
    Chunk { index: u32, data: String },
    /// Receiver acknowledges chunk `index`; sender may send the next chunk.
    Ack { index: u32 },
    /// Sender signals that all chunks have been delivered.
    FileEnd,
    /// Error notification; either peer may send this before closing.
    Error { message: String },
}
