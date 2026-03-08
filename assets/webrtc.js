/**
 * WebRTC P2P file transfer — client-side logic.
 *
 * Exported functions (called from Rust via `eval()`):
 *   startP2pSender(signalUrl)   — Peer A: connects, creates offer, sends file.
 *   startP2pReceiver(sessionId) — Peer B: connects, waits for offer, receives file.
 *
 * Protocol (over DataChannel once WebRTC is connected):
 *   Sender → Receiver:
 *     { type: "file-start",  name, size, total_chunks }
 *     { type: "chunk",       index, data }   (base64, 64 KB each)
 *     { type: "file-end" }
 *   Receiver → Sender:
 *     { type: "ack", index }                 (stop-and-wait)
 *
 * ICE config uses Google STUN; swap in a TURN server via the
 * HERMES_TURN_URL / HERMES_TURN_USER / HERMES_TURN_PASS env vars
 * (injected at build time or passed via a /api/ice-config endpoint).
 */

const CHUNK_SIZE = 64 * 1024; // 64 KB
const ACK_TIMEOUT_MS = 10_000;
const MAX_RETRIES = 3;

const ICE_CONFIG = {
  iceServers: [
    { urls: "stun:stun.l.google.com:19302" },
    // Add TURN entries here for corporate/symmetric-NAT fallback:
    // { urls: "turn:your.turn.server:3478", username: "user", credential: "pass" }
  ],
};

// ── Global state ─────────────────────────────────────────────────────────────

let currentChannel = null;

// ── Helpers ──────────────────────────────────────────────────────────────────

function setStatus(msg) {
  const el = document.getElementById("p2p-status");
  if (el) el.textContent = msg;
}

function setProgress(pct) {
  const el = document.getElementById("p2p-progress");
  if (el) el.textContent = pct >= 0 ? `${pct}%` : "";
}

/**
 * Safely parses a JSON string. Returns null and sets an error status on failure.
 * @param {string} raw
 * @returns {object|null}
 */
function parseMessage(raw) {
  try {
    return JSON.parse(raw);
  } catch {
    console.error("P2P: failed to parse message");
    return null;
  }
}

/**
 * Encodes an ArrayBuffer to base64 without using spread (avoids stack overflow
 * on large buffers with String.fromCharCode(...array)).
 * @param {ArrayBuffer} buf
 * @returns {string}
 */
function bufferToBase64(buf) {
  const bytes = new Uint8Array(buf);
  let binary = "";
  for (let i = 0; i < bytes.byteLength; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary);
}

function openSignalingSocket(url) {
  // Ensure the protocol is correct (Axum 0.8 might still send http:// if not careful)
  const wsUrl = url.replace(/^http/, "ws");
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl);
    ws.onopen = () => resolve(ws);
    ws.onerror = () => reject(new Error("WebSocket connection failed"));
    setTimeout(() => reject(new Error("WebSocket connection timeout")), 10_000);
  });
}

// ── Sender (Peer A) ──────────────────────────────────────────────────────────

/**
 * Initialises the sender side.
 *
 * @param {string} signalUrl - Full WebSocket URL from the server (ws://…/ws/signal/{id}).
 */
window.startP2pSender = async function(signalUrl) {
  currentChannel = null;
  setStatus("Connecting to signaling server…");

  // Rewrite the URL to use the current page's host so the WebSocket
  // connects through the same origin (handles dx serve proxying).
  try {
    const parsed = new URL(signalUrl);
    parsed.host = location.host;
    parsed.protocol = location.protocol === "https:" ? "wss:" : "ws:";
    signalUrl = parsed.toString();
  } catch (_) { /* keep original URL if parsing fails */ }

  let ws;
  try {
    ws = await openSignalingSocket(signalUrl);
  } catch (e) {
    console.error("P2P Sender: socket error", e.message);
    setStatus("Error: " + e.message);
    return;
  }

  const pc = new RTCPeerConnection(ICE_CONFIG);
  const channel = pc.createDataChannel("hermes-file", { ordered: true });

  // Forward local ICE candidates.
  pc.onicecandidate = ({ candidate }) => {
    if (candidate) {
      ws.send(JSON.stringify({ type: "ice-candidate", candidate }));
    }
  };

  // Handle incoming signaling messages.
  ws.onmessage = async ({ data }) => {
    const msg = parseMessage(data);
    if (!msg) return;
    switch (msg.type) {
      case "peer-joined":
        setStatus("Receiver joined! Creating offer…");
        const offer = await pc.createOffer();
        await pc.setLocalDescription(offer);
        ws.send(JSON.stringify({ type: "offer", sdp: offer.sdp }));
        break;
      case "answer":
        await pc.setRemoteDescription({ type: "answer", sdp: msg.sdp });
        setStatus("Receiver accepted. Connecting…");
        break;
      case "ice-candidate":
        if (msg.candidate) {
          await pc.addIceCandidate(msg.candidate);
        }
        break;
      case "bye":
        setStatus("Receiver disconnected.");
        pc.close();
        currentChannel = null;
        break;
    }
  };

  setStatus("Waiting for receiver to connect…");

  // Attach file-sending logic once the channel is open.
  channel.onopen = () => {
    currentChannel = channel;
    setStatus("Connected! Select a file to send.");
    // If a file was already selected before the channel opened, send it now.
    const input = document.getElementById("p2p-file-input");
    if (input && input.files && input.files[0]) {
      sendFile(channel, input.files[0]);
    }
  };

  channel.onerror = (e) => {
    console.error("P2P Sender: DataChannel error", e.message);
    setStatus("Channel error.");
    currentChannel = null;
  };
  channel.onclose = () => {
    setStatus("Transfer complete.");
    currentChannel = null;
  };
}

/**
 * Called by Rust when a file is selected via standard input.
 */
window.startP2pTransfer = function() {
  const input = document.getElementById("p2p-file-input");
  const file = input?.files?.[0];
  if (!file) return;

  if (currentChannel && currentChannel.readyState === "open") {
    sendFile(currentChannel, file);
  } else {
    // If channel isn't open yet, startP2pSender's onopen will catch it.
    setStatus("Waiting for receiver to connect…");
  }
}

/**
 * Called by Rust when a file is dropped via Drag & Drop.
 */
window.startP2pTransferWithFile = function(file) {
  if (!file) return;

  if (currentChannel && currentChannel.readyState === "open") {
    sendFile(currentChannel, file);
  } else {
    setStatus("Waiting for receiver to connect…");
  }
}


/**
 * Sends `file` over `channel` using the stop-and-wait chunking protocol.
 *
 * @param {RTCDataChannel} channel
 * @param {File} file
 */
async function sendFile(channel, file) {
  const totalChunks = Math.ceil(file.size / CHUNK_SIZE);
  setStatus(`Sending ${file.name} (${totalChunks} chunks)…`);

  channel.send(
    JSON.stringify({ type: "file-start", name: file.name, size: file.size, total_chunks: totalChunks })
  );

  for (let i = 0; i < totalChunks; i++) {
    const slice = file.slice(i * CHUNK_SIZE, (i + 1) * CHUNK_SIZE);
    const buf = await slice.arrayBuffer();
    const b64 = bufferToBase64(buf);

    let ackReceived = false;
    let retries = 0;

    while (!ackReceived && retries < MAX_RETRIES) {
      channel.send(JSON.stringify({ type: "chunk", index: i, data: b64 }));

      await new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          retries++;
          reject(new Error("ack timeout"));
        }, ACK_TIMEOUT_MS);

        const prev = channel.onmessage;
        channel.onmessage = ({ data }) => {
          const msg = parseMessage(data);
          if (!msg) { clearTimeout(timeout); channel.onmessage = prev; resolve(); return; }
          if (msg.type === "ack" && msg.index === i) {
            clearTimeout(timeout);
            channel.onmessage = prev;
            ackReceived = true;
            resolve();
          } else if (prev) {
            prev({ data });
          }
        };
      }).catch(() => {});
    }

    if (!ackReceived) {
      channel.send(JSON.stringify({ type: "error", message: `chunk ${i} failed after ${MAX_RETRIES} retries` }));
      setStatus("Transfer failed.");
      return;
    }

    setProgress(Math.round(((i + 1) / totalChunks) * 100));
  }

  channel.send(JSON.stringify({ type: "file-end" }));
  setStatus("File sent successfully.");
  setProgress(-1);
}

// ── Receiver (Peer B) ─────────────────────────────────────────────────────────

/**
 * Initialises the receiver side.
 *
 * @param {string} sessionId - The P2P session UUID (without the WebSocket host).
 */
window.startP2pReceiver = async function(sessionId) {
  const wsUrl = `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/ws/signal/${sessionId}?role=receiver`;
  setStatus("Connecting to signaling server…");

  let ws;
  try {
    ws = await openSignalingSocket(wsUrl);
  } catch (e) {
    console.error("P2P Receiver: socket error", e.message);
    setStatus("Error: " + e.message);
    return;
  }

  const pc = new RTCPeerConnection(ICE_CONFIG);

  pc.onicecandidate = ({ candidate }) => {
    if (candidate) {
      ws.send(JSON.stringify({ type: "ice-candidate", candidate }));
    }
  };

  pc.ondatachannel = ({ channel }) => {
    setStatus("Connected! Waiting for file…");
    receiveFile(channel);
  };

  ws.onmessage = async ({ data }) => {
    const msg = parseMessage(data);
    if (!msg) return;
    switch (msg.type) {
      case "offer":
        setStatus("Sender found! Preparing connection…");
        await pc.setRemoteDescription({ type: "offer", sdp: msg.sdp });
        const answer = await pc.createAnswer();
        await pc.setLocalDescription(answer);
        ws.send(JSON.stringify({ type: "answer", sdp: answer.sdp }));
        break;
      case "ice-candidate":
        if (msg.candidate) {
          await pc.addIceCandidate(msg.candidate);
        }
        break;
      case "bye":
        setStatus("Sender disconnected.");
        pc.close();
        break;
    }
  };

  setStatus("Waiting for sender…");
}

/**
 * Listens on `channel` for the file-transfer sub-protocol and assembles
 * the file. A download link is created once all chunks arrive.
 *
 * @param {RTCDataChannel} channel
 */
function receiveFile(channel) {
  let meta = null;
  const chunks = [];

  channel.onmessage = ({ data }) => {
    const msg = parseMessage(data);
    if (!msg) return;
    switch (msg.type) {
      case "file-start":
        meta = msg;
        setStatus(`Receiving file…`);
        break;

      case "chunk": {
        // Decode base64 → Uint8Array
        const binary = atob(msg.data);
        const bytes = new Uint8Array(binary.length);
        for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
        chunks[msg.index] = bytes;
        channel.send(JSON.stringify({ type: "ack", index: msg.index }));
        if (meta) {
          setProgress(Math.round((chunks.filter(Boolean).length / meta.total_chunks) * 100));
        }
        break;
      }

      case "file-end": {
        const blob = new Blob(chunks);
        const url = URL.createObjectURL(blob);
        const el = document.getElementById("p2p-download");
        if (el) {
          // Use createElement to avoid XSS via peer-supplied filename.
          const a = document.createElement("a");
          a.href = url;
          a.download = meta?.name ?? "file";
          a.className = "btn";
          a.textContent = "Save file";
          el.replaceChildren(a);
        }
        setStatus("File received successfully.");
        setProgress(-1);
        break;
      }

      case "error":
        setStatus("Transfer error: " + msg.message);
        break;
    }
  };
}
