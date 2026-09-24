//! Leader election for the background indexer.
//!
//! Several meta-registry replicas may share a single Postgres database. Only
//! one of them should run the indexer at a time; otherwise every replica
//! repeats the same upstream discovery and competes for the same queue rows.
//!
//! On Postgres we elect a leader with a session-scoped advisory lock held on
//! a dedicated single-connection pool. The lock is released automatically
//! when the process (or its connection) goes away, letting another replica
//! take over. SQLite is single-process, so the lease is always held there.

use anyhow::Context;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};

use super::db_config::{Backend, DbConfig};

/// Fixed advisory-lock key for electing the indexer leader.
///
/// Derived once from the ASCII bytes `b"cmpindx!"` interpreted as a
/// big-endian signed 64-bit integer. It MUST NOT change, or replicas running
/// different binaries could each believe they are the leader.
const POSTGRES_INDEXER_ADVISORY_LOCK_KEY: i64 = 7_164_506_180_342_282_273;

/// A claim on the right to run the background indexer.
///
/// Call [`IndexerLease::try_hold`] before every indexing cycle: it acquires
/// the lease if it is free, confirms it is still held otherwise, and returns
/// `false` when another replica owns it.
#[derive(Debug)]
pub struct IndexerLease {
    /// Dedicated connection holding the advisory lock (Postgres only).
    conn: Option<DatabaseConnection>,
}

impl IndexerLease {
    /// Open a lease handle for the given database configuration.
    ///
    /// This does not acquire the lease yet; see [`IndexerLease::try_hold`].
    ///
    /// # Errors
    ///
    /// Returns an error if the dedicated Postgres connection cannot be
    /// established.
    pub(crate) async fn connect(cfg: Option<&DbConfig>) -> anyhow::Result<Self> {
        let Some(cfg) = cfg.filter(|c| matches!(c.backend, Backend::Postgres)) else {
            return Ok(Self { conn: None });
        };
        // Advisory locks are bound to a session, so the lease needs a single
        // long-lived physical connection. Disable idle/lifetime recycling so
        // the pool doesn't silently drop the lock by replacing the
        // connection; if it is replaced anyway (e.g. a network blip), the
        // next `try_hold` either re-acquires the lock or reports that
        // another replica took over.
        let mut opts = cfg.to_connect_options();
        opts.max_connections(1)
            .min_connections(1)
            .idle_timeout(None)
            .max_lifetime(None);
        let conn = Database::connect(opts).await.with_context(|| {
            format!(
                "failed to open indexer lease connection to {}",
                cfg.redacted_url()
            )
        })?;
        Ok(Self { conn: Some(conn) })
    }

    /// Acquire or re-confirm the lease.
    ///
    /// Returns `true` when this process holds the lease and should run the
    /// indexer. This never blocks. Advisory locks are re-entrant, so the
    /// lock is only requested when `pg_locks` shows this session doesn't
    /// already hold it; otherwise every renewal would bump the lock count.
    /// Checking `pg_locks` also catches a pool reconnect, where the new
    /// session has to re-acquire the lock.
    ///
    /// # Errors
    ///
    /// Returns an error if the lease query fails.
    pub async fn try_hold(&self) -> anyhow::Result<bool> {
        let Some(conn) = &self.conn else {
            return Ok(true);
        };
        let (class_id, obj_id) = lock_key_parts(POSTGRES_INDEXER_ADVISORY_LOCK_KEY);
        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            HOLD_LEASE_SQL,
            [
                POSTGRES_INDEXER_ADVISORY_LOCK_KEY.into(),
                class_id.into(),
                obj_id.into(),
            ],
        );
        let row = conn
            .query_one_raw(stmt)
            .await
            .context("failed to query indexer advisory lock")?
            .context("indexer advisory lock query returned no rows")?;
        let held: bool = row
            .try_get("", "held")
            .context("failed to decode indexer advisory lock result")?;
        Ok(held)
    }

    /// Release one level of the advisory lock, as if the lease had been
    /// acquired only once. Returns whether an unlock happened.
    #[cfg(test)]
    pub(crate) async fn unlock_once(&self) -> anyhow::Result<bool> {
        let Some(conn) = &self.conn else {
            return Ok(false);
        };
        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_unlock($1) AS released",
            [POSTGRES_INDEXER_ADVISORY_LOCK_KEY.into()],
        );
        let row = conn
            .query_one_raw(stmt)
            .await?
            .context("advisory unlock query returned no rows")?;
        Ok(row.try_get("", "released")?)
    }
}

/// Acquire the indexer lock unless this session already holds it.
///
/// `CASE` only evaluates the volatile `pg_try_advisory_lock` when the
/// `EXISTS` branch is false, so an existing hold isn't counted twice.
const HOLD_LEASE_SQL: &str = "SELECT CASE WHEN EXISTS ( \
        SELECT 1 FROM pg_locks \
        WHERE locktype = 'advisory' AND pid = pg_backend_pid() AND granted \
          AND classid::bigint = $2 AND objid::bigint = $3 AND objsubid = 1 \
    ) THEN true ELSE pg_try_advisory_lock($1) END AS held";

/// Split a 64-bit advisory-lock key into the `classid` (high 32 bits) and
/// `objid` (low 32 bits) columns that `pg_locks` reports for it.
fn lock_key_parts(key: i64) -> (i64, i64) {
    let bits = u64::from_be_bytes(key.to_be_bytes());
    let high = i64::try_from(bits >> 32).expect("32-bit value fits in i64");
    let low = i64::try_from(bits & 0xFFFF_FFFF).expect("32-bit value fits in i64");
    (high, low)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sqlite_lease_is_always_held() {
        let lease = IndexerLease::connect(None)
            .await
            .expect("sqlite lease must connect");
        assert!(lease.try_hold().await.expect("sqlite lease must succeed"));
        assert!(lease.try_hold().await.expect("sqlite lease must stay held"));
    }

    #[test]
    fn lock_key_differs_from_migration_key() {
        assert_ne!(
            POSTGRES_INDEXER_ADVISORY_LOCK_KEY,
            super::super::store::POSTGRES_MIGRATION_ADVISORY_LOCK_KEY
        );
    }

    #[test]
    fn lock_key_parts_split_high_and_low_words() {
        assert_eq!(lock_key_parts(0x0000_0001_0000_0002), (1, 2));
        assert_eq!(lock_key_parts(-1), (0xFFFF_FFFF, 0xFFFF_FFFF));
        assert_eq!(
            lock_key_parts(POSTGRES_INDEXER_ADVISORY_LOCK_KEY),
            (0x636D_7069, 0x6E64_7821)
        );
    }

    #[test]
    fn lock_key_matches_documented_bytes() {
        assert_eq!(
            POSTGRES_INDEXER_ADVISORY_LOCK_KEY,
            i64::from_be_bytes(*b"cmpindx!")
        );
    }
}
