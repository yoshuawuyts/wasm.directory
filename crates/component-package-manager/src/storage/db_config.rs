//! Database connection configuration for the package-manager store.
//!
//! Two backends are supported, selected at runtime by the URL scheme:
//!
//! * `sqlite://...` — local file or `sqlite::memory:`. Default if no URL
//!   is configured. Auto-migrates on `Store::open`.
//! * `postgres://...` — remote PostgreSQL. Auto-migrates on `Store::open`
//!   too, with the migration step serialized by a Postgres advisory lock
//!   to avoid races between replicas.
//!
//! Configuration sources, in order of precedence:
//! 1. `COMPONENT_DATABASE_URL` environment variable.
//! 2. `database.url` field in the config file (not yet wired).
//! 3. Built-in default: a SQLite file under the platform data directory.
//!
//! Optional tuning (env vars):
//! * `COMPONENT_DATABASE_MAX_CONNECTIONS` (Postgres only; default 8)
//! * `COMPONENT_DATABASE_CONNECT_TIMEOUT_SECS` (default 10)

use std::time::Duration;

use sea_orm::{ConnectOptions, DbBackend};

/// Environment variable specifying the database connection URL.
const ENV_DATABASE_URL: &str = "COMPONENT_DATABASE_URL";
/// Environment variable for the Postgres pool's max connection count.
const ENV_MAX_CONNECTIONS: &str = "COMPONENT_DATABASE_MAX_CONNECTIONS";
/// Environment variable for the connection acquisition timeout.
const ENV_CONNECT_TIMEOUT: &str = "COMPONENT_DATABASE_CONNECT_TIMEOUT_SECS";

/// Default Postgres pool size.
const DEFAULT_PG_MAX_CONNECTIONS: u32 = 8;
/// Default connection acquisition timeout.
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;

/// Selected database backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// SQLite (`sqlite://...` or `sqlite::memory:`).
    Sqlite,
    /// PostgreSQL (`postgres://...` or `postgresql://...`).
    Postgres,
}

impl Backend {
    /// Map this enum to SeaORM's [`DbBackend`].
    #[must_use]
    pub fn db_backend(self) -> DbBackend {
        match self {
            Self::Sqlite => DbBackend::Sqlite,
            Self::Postgres => DbBackend::Postgres,
        }
    }
}

/// Resolved database configuration.
#[derive(Debug, Clone)]
pub struct DbConfig {
    /// The connection URL.
    pub url: String,
    /// Backend selected from the URL scheme.
    pub backend: Backend,
    /// Pool size hint. SeaORM ignores this for SQLite (single connection).
    pub max_connections: u32,
    /// How long to wait for a free connection from the pool.
    pub connect_timeout: Duration,
}

impl DbConfig {
    /// Build a `DbConfig` from the runtime environment, falling back to a
    /// SQLite file at `default_sqlite_path` if no env var is set.
    ///
    /// # Errors
    /// Returns an error when the URL scheme is unsupported or when the
    /// numeric env vars cannot be parsed.
    pub fn from_env(default_sqlite_path: &std::path::Path) -> anyhow::Result<Self> {
        let url = std::env::var(ENV_DATABASE_URL)
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| sqlite_file_url(default_sqlite_path));

        let backend = backend_from_url(&url)?;
        let max_connections = read_env_u32(ENV_MAX_CONNECTIONS, DEFAULT_PG_MAX_CONNECTIONS)?;
        let connect_timeout = Duration::from_secs(read_env_u64(
            ENV_CONNECT_TIMEOUT,
            DEFAULT_CONNECT_TIMEOUT_SECS,
        )?);

        Ok(Self {
            url,
            backend,
            max_connections,
            connect_timeout,
        })
    }

    /// Build SeaORM `ConnectOptions` from this config.
    #[must_use]
    pub fn to_connect_options(&self) -> ConnectOptions {
        let mut opts = ConnectOptions::new(self.url.clone());
        opts.sqlx_logging(false)
            .acquire_timeout(self.connect_timeout)
            .connect_timeout(self.connect_timeout);
        if matches!(self.backend, Backend::Postgres) {
            opts.max_connections(self.max_connections);
        } else {
            // SQLite: keep a single connection so WAL semantics behave the
            // way the legacy rusqlite path expected.
            opts.max_connections(1);
        }
        opts
    }

    /// Return the URL with any password component redacted.
    ///
    /// Use this any time the URL is written to logs or error messages.
    #[must_use]
    pub fn redacted_url(&self) -> String {
        redact_url(&self.url)
    }
}

/// Build a `sqlite://` URL for a file path.
fn sqlite_file_url(path: &std::path::Path) -> String {
    format!("sqlite://{}?mode=rwc", path.display())
}

/// Determine the backend from a connection URL.
fn backend_from_url(url: &str) -> anyhow::Result<Backend> {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("sqlite:") {
        Ok(Backend::Sqlite)
    } else if lower.starts_with("postgres:") || lower.starts_with("postgresql:") {
        Ok(Backend::Postgres)
    } else {
        anyhow::bail!(
            "unsupported {ENV_DATABASE_URL} scheme: {} \
             (expected sqlite:// or postgres://)",
            redact_url(url)
        );
    }
}

/// Strip any password from a URL of the form `scheme://user:pass@host/...`.
#[must_use]
pub fn redact_url(url: &str) -> String {
    // Find the first `://` to anchor the parse.
    let Some(scheme_end) = url.find("://") else {
        return url.to_owned();
    };
    let after_scheme = &url[scheme_end + 3..];
    // Userinfo (if any) ends at the first `@`; only the part before the first
    // `/` may contain it.
    let path_start = after_scheme.find('/').unwrap_or(after_scheme.len());
    let authority = &after_scheme[..path_start];
    let path = &after_scheme[path_start..];
    let Some(at_idx) = authority.find('@') else {
        return url.to_owned();
    };
    let userinfo = &authority[..at_idx];
    let host = &authority[at_idx + 1..];
    let new_userinfo = match userinfo.find(':') {
        Some(c) => format!("{}:[REDACTED]", &userinfo[..c]),
        None => userinfo.to_owned(),
    };
    format!("{}://{}@{}{}", &url[..scheme_end], new_userinfo, host, path)
}

fn read_env_u32(key: &str, default: u32) -> anyhow::Result<u32> {
    match std::env::var(key) {
        Ok(s) if !s.is_empty() => s
            .parse::<u32>()
            .map_err(|e| anyhow::anyhow!("invalid {key}: {e}")),
        _ => Ok(default),
    }
}

fn read_env_u64(key: &str, default: u64) -> anyhow::Result<u64> {
    match std::env::var(key) {
        Ok(s) if !s.is_empty() => s
            .parse::<u64>()
            .map_err(|e| anyhow::anyhow!("invalid {key}: {e}")),
        _ => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_password() {
        assert_eq!(
            redact_url("postgres://alice:hunter2@db.example.com:5432/wasm"),
            "postgres://alice:[REDACTED]@db.example.com:5432/wasm"
        );
    }

    #[test]
    fn redact_no_password() {
        assert_eq!(
            redact_url("postgres://alice@db.example.com/wasm"),
            "postgres://alice@db.example.com/wasm"
        );
    }

    #[test]
    fn redact_no_userinfo() {
        assert_eq!(
            redact_url("postgres://db.example.com/wasm"),
            "postgres://db.example.com/wasm"
        );
    }

    #[test]
    fn redact_sqlite() {
        assert_eq!(
            redact_url("sqlite:///var/lib/wasm/db.sqlite?mode=rwc"),
            "sqlite:///var/lib/wasm/db.sqlite?mode=rwc"
        );
    }

    #[test]
    fn backend_detection() {
        assert_eq!(
            backend_from_url("sqlite::memory:").unwrap(),
            Backend::Sqlite
        );
        assert_eq!(
            backend_from_url("sqlite:///tmp/foo.db").unwrap(),
            Backend::Sqlite
        );
        assert_eq!(
            backend_from_url("postgres://x@y/z").unwrap(),
            Backend::Postgres
        );
        assert_eq!(
            backend_from_url("postgresql://x@y/z").unwrap(),
            Backend::Postgres
        );
        assert!(backend_from_url("mysql://x@y/z").is_err());
    }
}
