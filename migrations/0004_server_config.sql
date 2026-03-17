-- Key-value store for server configuration.
-- Seeded on every boot from hermes.toml + env vars.
-- The frontend reads and writes rows here at runtime.
CREATE TABLE IF NOT EXISTS server_config (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
