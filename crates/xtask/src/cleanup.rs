//! `cargo xtask cleanup-bad-tags` — one-off cleanup that purges
//! `oci_tag` rows whose tag does not parse as a strict `semver::Version`.
//!
//! Historically the indexer accepted tags like `vX.Y.Z`, `latest`,
//! `nightly`, and other non-conforming strings. Those rows clutter search
//! results and render incorrectly in the frontend. The indexer now rejects
//! them at ingestion time; this command removes already-stored rows.
//!
//! Connects to the same database the registry uses: `COMPONENT_DATABASE_URL`
//! if set, otherwise the default SQLite file under the platform data dir.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use anyhow::{Context, Result};
use sea_orm::{
    Database, DatabaseConnection, EntityTrait, QueryFilter, Statement,
    sea_query::{Expr, ExprTrait},
};

use wasm_package_manager_migration::entities::{oci_repository, oci_tag};

pub(crate) fn run(dry_run: bool) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("building tokio runtime")?;
    runtime.block_on(run_async(dry_run))
}

async fn run_async(dry_run: bool) -> Result<()> {
    let url = database_url()?;
    let redacted = redact_url(&url);
    println!("Connecting to {redacted}...");
    let db = Database::connect(&url)
        .await
        .with_context(|| format!("connecting to {redacted}"))?;

    let tags = oci_tag::Entity::find()
        .find_also_related(oci_repository::Entity)
        .all(&db)
        .await
        .context("loading oci_tag rows")?;

    let total = tags.len();
    let mut bad_ids: Vec<i64> = Vec::new();
    for (row, repo) in &tags {
        if semver::Version::parse(&row.tag).is_err() {
            bad_ids.push(row.id);
            let name = match repo {
                Some(r) => format!("{}/{}", r.registry, r.repository),
                None => "<unknown repository>".to_owned(),
            };
            println!("  bad: {name}:{}", row.tag);
        }
    }

    println!(
        "Found {} bad tag rows out of {} total.",
        bad_ids.len(),
        total
    );

    if dry_run {
        println!("Dry run: not deleting. Re-run without --dry-run to apply.");
        return Ok(());
    }

    if bad_ids.is_empty() {
        println!("Nothing to delete.");
    } else {
        let deleted = oci_tag::Entity::delete_many()
            .filter(Expr::col(oci_tag::Column::Id).is_in(bad_ids.clone()))
            .exec(&db)
            .await
            .context("deleting bad oci_tag rows")?;
        println!("Deleted {} oci_tag rows.", deleted.rows_affected);
    }

    // Drop oci_repository rows that no longer have any tags AND no manifests.
    // These are leftover stubs that would otherwise still appear in listings
    // with an em-dash version.
    let orphan_repos = find_orphan_repositories(&db).await?;
    println!("Found {} orphan repository rows.", orphan_repos.len());
    for r in &orphan_repos {
        println!("  orphan: id={} {}/{}", r.id, r.registry, r.repository);
    }
    if !orphan_repos.is_empty() {
        let ids: Vec<i64> = orphan_repos.iter().map(|r| r.id).collect();
        let deleted = oci_repository::Entity::delete_many()
            .filter(Expr::col(oci_repository::Column::Id).is_in(ids))
            .exec(&db)
            .await
            .context("deleting orphan oci_repository rows")?;
        println!("Deleted {} oci_repository rows.", deleted.rows_affected);
    }

    Ok(())
}

/// Find repositories with zero tags AND zero manifests.
async fn find_orphan_repositories(db: &DatabaseConnection) -> Result<Vec<oci_repository::Model>> {
    let backend = db.get_database_backend();
    let sql = "SELECT r.* FROM oci_repository r \
               WHERE NOT EXISTS (SELECT 1 FROM oci_tag t WHERE t.oci_repository_id = r.id) \
               AND NOT EXISTS (SELECT 1 FROM oci_manifest m WHERE m.oci_repository_id = r.id)";
    let rows = oci_repository::Entity::find()
        .from_raw_sql(Statement::from_string(backend, sql.to_owned()))
        .all(db)
        .await
        .context("querying orphan repositories")?;
    Ok(rows)
}

fn database_url() -> Result<String> {
    if let Ok(url) = std::env::var("COMPONENT_DATABASE_URL") {
        return Ok(url);
    }
    // Default: SQLite file in the platform data dir, matching what
    // `wasm-package-manager` uses (`dirs::data_local_dir()`).
    let dir = dirs::data_local_dir().context("locating platform data directory")?;
    // The meta-registry server uses `wasm-registry`; the CLI uses `wasm`.
    // Default to the registry's database since that's what feeds search.
    let db_dir = dir.join("wasm-registry").join("db");
    let candidates = ["metadata-v2.db3", "metadata.db3"];
    let path = candidates
        .iter()
        .map(|n| db_dir.join(n))
        .find(|p| p.exists())
        .with_context(|| {
            format!(
                "no SQLite database found under {}\n\
                 set COMPONENT_DATABASE_URL to point at the registry database",
                db_dir.display()
            )
        })?;
    Ok(format!(
        "sqlite://{}?mode=rwc",
        path.to_str().context("non-UTF-8 database path")?
    ))
}

fn redact_url(url: &str) -> String {
    // Strip user:password@ if present.
    if let Some(scheme_end) = url.find("://") {
        let (scheme, rest) = url.split_at(scheme_end + 3);
        if let Some(at) = rest.find('@') {
            return format!("{scheme}***@{}", &rest[at + 1..]);
        }
    }
    url.to_owned()
}
