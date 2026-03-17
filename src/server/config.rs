//! Application configuration loaded from `hermes.toml` with env-var overrides.
//!
//! Loading priority (highest first):
//! 1. Environment variables
//! 2. `hermes.toml` (or path in `HERMES_CONFIG` env var)
//! 3. Built-in defaults
//!
//! # Example `hermes.toml`
//!
//! ```toml
//! [server]
//! host     = "0.0.0.0"
//! port     = 8080
//! base_url = "http://localhost:8080"
//! log      = "hermes=info"
//!
//! [database]
//! url = "sqlite:hermes.db"
//!
//! [admin]
//! email = "admin@hermes.local"
//!
//! [storage]
//! default_quota       = "1GB"
//! default_local_ratio = 100
//!
//! [storage.local]
//! path = "./storage/uploads"
//! ```

use serde::Deserialize;
use sqlx::SqlitePool;

// ── Top-level config ──────────────────────────────────────────────────────────

/// Root configuration for the Hermes server.
#[derive(Debug, Clone, Deserialize)]
pub struct HermesConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub admin: AdminConfig,
    #[serde(default)]
    pub storage: StorageAppConfig,
}

// ── Sub-sections ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub base_url: String,
    pub log: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AdminConfig {
    pub email: String,
    /// When `None` a random password is generated on first boot.
    pub password: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageAppConfig {
    /// Maximum total bytes a new user may store.
    #[serde(default = "default_quota")]
    pub default_quota: QuotaBytes,
    /// Fraction (0–100) of user quota allocated to the local backend.
    #[serde(default = "default_local_ratio")]
    pub default_local_ratio: u8,
    /// Present ⟹ local filesystem backend is enabled.
    pub local: Option<LocalStorageConfig>,
    /// Present ⟹ S3-compatible backend is enabled.
    pub s3: Option<S3StorageConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LocalStorageConfig {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct S3StorageConfig {
    pub bucket: String,
    pub region: String,
    /// Custom endpoint URL.  Omit to use the AWS default.
    pub endpoint: Option<String>,
    pub access_key_id: String,
    pub secret_access_key: String,
}

// ── QuotaBytes ────────────────────────────────────────────────────────────────

/// A storage quota expressed as a byte count or as "unlimited".
#[derive(Debug, Clone, PartialEq)]
pub enum QuotaBytes {
    Unlimited,
    Bytes(u64),
}

impl QuotaBytes {
    /// Returns `Some(bytes)` or `None` for unlimited.
    pub fn as_option(&self) -> Option<u64> {
        match self {
            QuotaBytes::Unlimited => None,
            QuotaBytes::Bytes(n) => Some(*n),
        }
    }

    /// Format as a human-readable string suitable for display and round-trip
    /// through [`parse_quota_str`].
    pub fn to_human(&self) -> String {
        match self {
            QuotaBytes::Unlimited => "unlimited".to_owned(),
            QuotaBytes::Bytes(b) => {
                const TB: u64 = 1_024 * 1_024 * 1_024 * 1_024;
                const GB: u64 = 1_024 * 1_024 * 1_024;
                const MB: u64 = 1_024 * 1_024;
                const KB: u64 = 1_024;
                if *b % TB == 0 { format!("{}TB", b / TB) }
                else if *b % GB == 0 { format!("{}GB", b / GB) }
                else if *b % MB == 0 { format!("{}MB", b / MB) }
                else if *b % KB == 0 { format!("{}KB", b / KB) }
                else { b.to_string() }
            }
        }
    }

    /// Returns `true` if the quota is unlimited or if `used + size <= quota`.
    pub fn has_space(&self, used: u64, size: u64) -> bool {
        match self {
            QuotaBytes::Unlimited => true,
            QuotaBytes::Bytes(q) => used.saturating_add(size) <= *q,
        }
    }
}

impl<'de> Deserialize<'de> for QuotaBytes {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        parse_quota_str(&s).map_err(serde::de::Error::custom)
    }
}

/// Parse a human-readable quota string into [`QuotaBytes`].
///
/// Supported formats: `"0"`, `"unlimited"`, `"500MB"`, `"1GB"`, `"2TB"`, …
pub fn parse_quota_str(s: &str) -> Result<QuotaBytes, String> {
    let s = s.trim();
    if s == "0" || s.eq_ignore_ascii_case("unlimited") {
        return Ok(QuotaBytes::Unlimited);
    }

    let (num_part, suffix) = split_number_suffix(s);
    let n: u64 = num_part
        .parse()
        .map_err(|_| format!("invalid quota value: {s:?}"))?;

    let multiplier: u64 = match suffix.to_ascii_uppercase().as_str() {
        "" | "B" => 1,
        "KB" | "K" => 1_024,
        "MB" | "M" => 1_024 * 1_024,
        "GB" | "G" => 1_024 * 1_024 * 1_024,
        "TB" | "T" => 1_024u64 * 1_024 * 1_024 * 1_024,
        other => return Err(format!("unknown size suffix: {other:?}")),
    };

    Ok(QuotaBytes::Bytes(n * multiplier))
}

fn split_number_suffix(s: &str) -> (&str, &str) {
    let split = s
        .char_indices()
        .find(|(_, c)| c.is_alphabetic())
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    (&s[..split], &s[split..])
}

// ── Defaults ──────────────────────────────────────────────────────────────────

fn default_quota() -> QuotaBytes {
    QuotaBytes::Bytes(1_073_741_824) // 1 GB
}

fn default_local_ratio() -> u8 {
    100
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_owned(),
            port: 8080,
            base_url: "http://localhost:8080".to_owned(),
            log: "hermes=info".to_owned(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite:hermes.db".to_owned(),
        }
    }
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            email: "admin@hermes.local".to_owned(),
            password: None,
        }
    }
}

impl Default for StorageAppConfig {
    fn default() -> Self {
        Self {
            default_quota: default_quota(),
            default_local_ratio: default_local_ratio(),
            local: Some(LocalStorageConfig {
                path: "./storage/uploads".to_owned(),
            }),
            s3: None,
        }
    }
}

// ── Loader ────────────────────────────────────────────────────────────────────

impl HermesConfig {
    /// Load configuration from the TOML file and apply env-var overrides.
    ///
    /// File path: `HERMES_CONFIG` env var → `hermes.toml` in the current directory.
    /// If no file is found, defaults are used.
    pub fn load() -> Self {
        let path = std::env::var("HERMES_CONFIG")
            .unwrap_or_else(|_| "hermes.toml".to_owned());

        let mut cfg: HermesConfig = if std::path::Path::new(&path).exists() {
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
            toml::from_str(&text)
                .unwrap_or_else(|e| panic!("invalid TOML in {path}: {e}"))
        } else {
            HermesConfig {
                server: ServerConfig::default(),
                database: DatabaseConfig::default(),
                admin: AdminConfig::default(),
                storage: StorageAppConfig::default(),
            }
        };

        cfg.apply_env_overrides();
        cfg
    }

    /// Apply env-var overrides on top of whatever was loaded from TOML.
    fn apply_env_overrides(&mut self) {
        if let Ok(v) = std::env::var("HOST") {
            self.server.host = v;
        }
        if let Ok(v) = std::env::var("PORT") {
            if let Ok(p) = v.parse() {
                self.server.port = p;
            }
        }
        if let Ok(v) = std::env::var("BASE_URL") {
            self.server.base_url = v;
        }
        if let Ok(v) = std::env::var("RUST_LOG") {
            self.server.log = v;
        }
        if let Ok(v) = std::env::var("DATABASE_URL") {
            self.database.url = v;
        }
        if let Ok(v) = std::env::var("ADMIN_EMAIL") {
            self.admin.email = v;
        }
        if let Ok(v) = std::env::var("ADMIN_PASSWORD") {
            self.admin.password = Some(v);
        }

        if let Ok(v) = std::env::var("STORAGE_DEFAULT_QUOTA") {
            match parse_quota_str(&v) {
                Ok(q) => self.storage.default_quota = q,
                Err(e) => eprintln!("STORAGE_DEFAULT_QUOTA ignored: {e}"),
            }
        }
        if let Ok(v) = std::env::var("STORAGE_DEFAULT_LOCAL_RATIO") {
            match v.parse::<u8>() {
                Ok(r) if r <= 100 => self.storage.default_local_ratio = r,
                _ => eprintln!("STORAGE_DEFAULT_LOCAL_RATIO ignored: must be 0–100"),
            }
        }

        // Local storage path
        if let Ok(v) = std::env::var("STORAGE_DIR") {
            match &mut self.storage.local {
                Some(local) => local.path = v,
                None => {
                    self.storage.local = Some(LocalStorageConfig { path: v });
                }
            }
        }

        // S3 settings
        let bucket = std::env::var("S3_BUCKET").ok();
        let region = std::env::var("S3_REGION").ok();
        let endpoint = std::env::var("S3_ENDPOINT").ok();
        let key_id = std::env::var("AWS_ACCESS_KEY_ID").ok();
        let secret = std::env::var("AWS_SECRET_ACCESS_KEY").ok();

        if bucket.is_some() || region.is_some() || key_id.is_some() || secret.is_some() {
            let s3 = self.storage.s3.get_or_insert_with(|| S3StorageConfig {
                bucket: String::new(),
                region: String::new(),
                endpoint: None,
                access_key_id: String::new(),
                secret_access_key: String::new(),
            });
            if let Some(v) = bucket {
                s3.bucket = v;
            }
            if let Some(v) = region {
                s3.region = v;
            }
            if endpoint.is_some() {
                s3.endpoint = endpoint;
            }
            if let Some(v) = key_id {
                s3.access_key_id = v;
            }
            if let Some(v) = secret {
                s3.secret_access_key = v;
            }
        }
    }
}

// ── DB sync ───────────────────────────────────────────────────────────────────

/// Keys stored in `server_config`.
pub mod keys {
    pub const STORAGE_DEFAULT_QUOTA: &str = "storage.default_quota";
    pub const STORAGE_DEFAULT_LOCAL_RATIO: &str = "storage.default_local_ratio";
    pub const SERVER_BASE_URL: &str = "server.base_url";
    pub const SERVER_LOG: &str = "server.log";
    pub const STORAGE_LOCAL_PATH: &str = "storage.local.path";
    pub const STORAGE_S3_BUCKET: &str = "storage.s3.bucket";
    pub const STORAGE_S3_REGION: &str = "storage.s3.region";
    pub const STORAGE_S3_ENDPOINT: &str = "storage.s3.endpoint";
    pub const STORAGE_S3_ACCESS_KEY_ID: &str = "storage.s3.access_key_id";
    pub const STORAGE_S3_SECRET_ACCESS_KEY: &str = "storage.s3.secret_access_key";
}

impl HermesConfig {
    /// Upsert all resolved config values into the `server_config` table.
    ///
    /// Called once at boot after [`HermesConfig::load`].  Env-var / TOML
    /// values always win and overwrite whatever the frontend may have saved.
    pub async fn sync_to_db(&self, pool: &SqlitePool) -> anyhow::Result<()> {
        let quota_str = self.storage.default_quota.to_human();

        let pairs: &[(&str, String)] = &[
            (keys::STORAGE_DEFAULT_QUOTA, quota_str),
            (
                keys::STORAGE_DEFAULT_LOCAL_RATIO,
                self.storage.default_local_ratio.to_string(),
            ),
            (keys::SERVER_BASE_URL, self.server.base_url.clone()),
            (keys::SERVER_LOG, self.server.log.clone()),
        ];

        let now = chrono::Utc::now().to_rfc3339();

        for (key, value) in pairs {
            sqlx::query(
                "INSERT INTO server_config (key, value, updated_at)
                 VALUES (?, ?, ?)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value,
                                                updated_at = excluded.updated_at",
            )
            .bind(key)
            .bind(value)
            .bind(&now)
            .execute(pool)
            .await?;
        }

        if let Some(local) = &self.storage.local {
            upsert(pool, keys::STORAGE_LOCAL_PATH, &local.path, &now).await?;
        }
        if let Some(s3) = &self.storage.s3 {
            upsert(pool, keys::STORAGE_S3_BUCKET, &s3.bucket, &now).await?;
            upsert(pool, keys::STORAGE_S3_REGION, &s3.region, &now).await?;
            upsert(pool, keys::STORAGE_S3_ACCESS_KEY_ID, &s3.access_key_id, &now).await?;
            upsert(pool, keys::STORAGE_S3_SECRET_ACCESS_KEY, &s3.secret_access_key, &now).await?;
            if let Some(ep) = &s3.endpoint {
                upsert(pool, keys::STORAGE_S3_ENDPOINT, ep, &now).await?;
            }
        }

        Ok(())
    }
}

async fn upsert(pool: &SqlitePool, key: &str, value: &str, now: &str) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO server_config (key, value, updated_at)
         VALUES (?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value,
                                        updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(value)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

/// Read a single key from `server_config`.
pub async fn db_get(pool: &SqlitePool, key: &str) -> Option<String> {
    sqlx::query_scalar("SELECT value FROM server_config WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .unwrap_or(None)
}

/// Write a single key to `server_config`.
pub async fn db_set(pool: &SqlitePool, key: &str, value: &str) -> anyhow::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    upsert(pool, key, value, &now).await
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quota_unlimited() {
        assert_eq!(parse_quota_str("0").unwrap(), QuotaBytes::Unlimited);
        assert_eq!(parse_quota_str("unlimited").unwrap(), QuotaBytes::Unlimited);
        assert_eq!(parse_quota_str("Unlimited").unwrap(), QuotaBytes::Unlimited);
    }

    #[test]
    fn parse_quota_sizes() {
        assert_eq!(parse_quota_str("1GB").unwrap(), QuotaBytes::Bytes(1_073_741_824));
        assert_eq!(parse_quota_str("500MB").unwrap(), QuotaBytes::Bytes(524_288_000));
        assert_eq!(
            parse_quota_str("2TB").unwrap(),
            QuotaBytes::Bytes(2 * 1_024 * 1_024 * 1_024 * 1_024),
        );
        assert_eq!(parse_quota_str("1024").unwrap(), QuotaBytes::Bytes(1024));
    }

    #[test]
    fn parse_quota_short_suffixes() {
        assert_eq!(parse_quota_str("1K").unwrap(), QuotaBytes::Bytes(1_024));
        assert_eq!(parse_quota_str("1M").unwrap(), QuotaBytes::Bytes(1_024 * 1_024));
        assert_eq!(parse_quota_str("1G").unwrap(), QuotaBytes::Bytes(1_024 * 1_024 * 1_024));
        assert_eq!(
            parse_quota_str("1T").unwrap(),
            QuotaBytes::Bytes(1_024u64 * 1_024 * 1_024 * 1_024),
        );
    }

    #[test]
    fn parse_quota_unknown_suffix_returns_err() {
        assert!(parse_quota_str("100PB").is_err());
        assert!(parse_quota_str("100XYZ").is_err());
    }

    #[test]
    fn parse_quota_invalid_number_returns_err() {
        assert!(parse_quota_str("abcGB").is_err());
        assert!(parse_quota_str("").is_err());
    }

    #[test]
    fn quota_to_human_round_trips() {
        for s in &["1KB", "500MB", "1GB", "2TB"] {
            let q = parse_quota_str(s).unwrap();
            let back = parse_quota_str(&q.to_human()).unwrap();
            assert_eq!(q, back, "round-trip failed for {s}");
        }
    }

    #[test]
    fn quota_to_human_unlimited() {
        assert_eq!(QuotaBytes::Unlimited.to_human(), "unlimited");
    }

    #[test]
    fn quota_has_space() {
        let q = QuotaBytes::Bytes(100);
        assert!(q.has_space(0, 100));
        assert!(!q.has_space(1, 100));
        assert!(QuotaBytes::Unlimited.has_space(u64::MAX, u64::MAX));
    }

    #[test]
    fn quota_has_space_at_exact_limit() {
        let q = QuotaBytes::Bytes(1_000);
        assert!(q.has_space(0, 1_000));  // exactly fits
        assert!(!q.has_space(1, 1_000)); // one byte over
        assert!(q.has_space(500, 500));  // split exactly
    }
}
