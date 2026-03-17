-- Associate files with users (nullable: pre-auth files remain NULL)
ALTER TABLE files ADD COLUMN user_id TEXT REFERENCES users(id) ON DELETE SET NULL;

-- Per-user storage quota and backend preferences
CREATE TABLE IF NOT EXISTS user_storage_config (
    user_id          TEXT PRIMARY KEY,
    quota_bytes      INTEGER,   -- NULL = use global default
    local_ratio      INTEGER,   -- NULL = use global default (0–100)
    backend_override TEXT,      -- NULL = automatic | 'local' | 's3'
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
