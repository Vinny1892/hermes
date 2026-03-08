-- Files stored on the server (cache/upload mode).
CREATE TABLE IF NOT EXISTS files (
    id          TEXT PRIMARY KEY,
    filename    TEXT NOT NULL,
    size        INTEGER NOT NULL,
    mime_type   TEXT NOT NULL,
    backend     TEXT NOT NULL DEFAULT 'local',
    storage_key TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    expires_at  TEXT NOT NULL
);

-- Short-lived public links for downloading a specific file.
-- Expiration of the link (10 min) is independent from the file's own TTL (7 days).
CREATE TABLE IF NOT EXISTS share_links (
    token      TEXT PRIMARY KEY,
    file_id    TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    FOREIGN KEY (file_id) REFERENCES files (id) ON DELETE CASCADE
);

-- WebRTC signaling sessions for P2P transfers.
CREATE TABLE IF NOT EXISTS p2p_sessions (
    id                  TEXT PRIMARY KEY,
    created_at          TEXT NOT NULL,
    expires_at          TEXT NOT NULL,
    peer_a_connected    INTEGER NOT NULL DEFAULT 0,
    peer_b_connected    INTEGER NOT NULL DEFAULT 0,
    state               TEXT NOT NULL DEFAULT 'waiting'
);

CREATE INDEX IF NOT EXISTS idx_files_expires_at        ON files (expires_at);
CREATE INDEX IF NOT EXISTS idx_share_links_expires_at  ON share_links (expires_at);
CREATE INDEX IF NOT EXISTS idx_p2p_sessions_expires_at ON p2p_sessions (expires_at);
