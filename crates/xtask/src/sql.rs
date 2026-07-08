//! `cargo xtask sql` — database migration tooling.
//!
//! With Rust-defined SeaORM migrations under
//! `crates/wasm-package-manager-migration/`, the legacy schema-diff
//! tooling (sqlite3def, schema.sql) is gone. The only remaining task is a
//! sanity-check that exercises the migrator against real backends — this is
//! what `cargo xtask sql check` does, and what `cargo xtask test` runs in CI.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use anyhow::{Context, Result};

/// `cargo xtask sql install` — kept for backwards compatibility with old CI
/// scripts; today there's nothing to install.
pub(crate) fn install() {
    println!("`cargo xtask sql install` is a no-op since the SeaORM port.");
    println!("Migrations live under crates/wasm-package-manager-migration/.");
}

/// `cargo xtask sql migrate` — placeholder. Hand-author migration files
/// directly; there's no diff-based generator any more.
pub(crate) fn migrate(_name: &str) -> Result<()> {
    anyhow::bail!(
        "`cargo xtask sql migrate` is no longer supported. \
         Hand-author a new migration under \
         crates/wasm-package-manager-migration/src/migrations/ \
         and register it in `Migrator::migrations()`."
    );
}

/// `cargo xtask sql check` — apply migrations to ephemeral databases.
///
/// Always runs against in-memory SQLite. When `COMPONENT_DATABASE_URL` is
/// set to a Postgres URL, also runs against that database (CI uses this to
/// verify the Postgres schema applies cleanly).
pub(crate) fn check() -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("building tokio runtime")?;
    runtime.block_on(check_async())
}

async fn check_async() -> Result<()> {
    use sea_orm::Database;
    use wasm_package_manager_migration::{Migrator, MigratorTrait};

    println!("Applying migrations to in-memory SQLite...");
    let sqlite_db = Database::connect("sqlite::memory:")
        .await
        .context("connecting to in-memory SQLite")?;
    Migrator::up(&sqlite_db, None)
        .await
        .context("running migrations against SQLite")?;
    println!("  OK");

    if let Ok(url) = std::env::var("COMPONENT_DATABASE_URL")
        && (url.starts_with("postgres://") || url.starts_with("postgresql://"))
    {
        // Print a redacted form so passwords don't leak into CI logs or
        // terminal history.
        let redacted = redact_url(&url);
        println!("Applying migrations to {redacted}...");
        let pg_db = Database::connect(url.clone())
            .await
            .with_context(|| format!("connecting to {redacted}"))?;
        // Non-destructive validation: only run `up`. Previously this also
        // ran `Migrator::down` to make the check repeatable, but that is
        // destructive and would wipe a real database if `COMPONENT_DATABASE_URL`
        // were ever pointed at a non-test instance. If `up` fails because
        // migrations are already applied, that's a signal the operator should
        // reset the test database manually.
        Migrator::up(&pg_db, None)
            .await
            .context("running migrations against Postgres")?;
        println!("  OK");
    } else {
        println!("Skipping Postgres check (set COMPONENT_DATABASE_URL=postgres://... to enable).");
    }
    Ok(())
}

/// Strip any password from a URL of the form `scheme://user:pass@host/...`.
///
/// Mirrors `wasm_package_manager::storage::redact_url` so that this
/// crate doesn't need to pull in the full package-manager dependency.
fn redact_url(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return url.to_owned();
    };
    let after_scheme = &url[scheme_end + 3..];
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

#[cfg(test)]
mod tests {
    use super::redact_url;

    #[test]
    fn redact_password() {
        assert_eq!(
            redact_url("postgres://alice:hunter2@db.example.com:5432/wasm"),
            "postgres://alice:[REDACTED]@db.example.com:5432/wasm"
        );
    }

    #[test]
    fn redact_no_userinfo() {
        assert_eq!(
            redact_url("postgres://db.example.com/wasm"),
            "postgres://db.example.com/wasm"
        );
    }
}
