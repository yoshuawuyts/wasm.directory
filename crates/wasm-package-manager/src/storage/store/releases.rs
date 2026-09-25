//! Shared release-time selection for listings and highlight timelines.

use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, FromQueryResult, Statement};

use super::bind_placeholders;

const RELEASE_ROWS_SQL: &str = "\
    SELECT t.id AS tag_id, t.oci_repository_id AS repo_id, t.tag AS tag, \
           t.created_at AS tag_indexed_at, m.created_at AS manifest_indexed_at, \
           m.oci_created AS oci_created, m.config_created AS config_created, \
           r.registry AS registry, r.repository AS repository, \
           r.wit_namespace AS wit_namespace, r.wit_name AS wit_name \
    FROM oci_tag t \
    JOIN oci_repository r ON r.id = t.oci_repository_id \
    LEFT JOIN oci_manifest m \
      ON m.oci_repository_id = t.oci_repository_id \
     AND m.digest = t.manifest_digest";

/// A tag joined with its repository and manifest publication metadata.
#[derive(FromQueryResult)]
pub(super) struct ReleaseRow {
    pub(super) tag_id: i64,
    pub(super) repo_id: i64,
    pub(super) tag: String,
    pub(super) tag_indexed_at: DateTime<Utc>,
    pub(super) manifest_indexed_at: Option<DateTime<Utc>>,
    pub(super) oci_created: Option<String>,
    pub(super) config_created: Option<String>,
    pub(super) registry: String,
    pub(super) repository: String,
    pub(super) wit_namespace: Option<String>,
    pub(super) wit_name: Option<String>,
}

impl ReleaseRow {
    /// Earliest sighting of the content, even if its tag was recreated.
    pub(super) fn indexed_at(&self) -> DateTime<Utc> {
        self.manifest_indexed_at
            .map_or(self.tag_indexed_at, |m| m.min(self.tag_indexed_at))
    }

    pub(super) fn released_at(&self) -> DateTime<Utc> {
        release_time(
            [self.oci_created.as_deref(), self.config_created.as_deref()],
            self.indexed_at(),
        )
    }
}

/// Load release candidates, optionally restricted to a batch of repositories.
pub(super) async fn release_rows(
    db: &DatabaseConnection,
    repo_ids: Option<&[i64]>,
) -> anyhow::Result<Vec<ReleaseRow>> {
    let backend = db.get_database_backend();
    let statement = match repo_ids {
        Some([]) => return Ok(Vec::new()),
        Some(ids) => {
            let placeholders = vec!["?"; ids.len()].join(", ");
            let sql = format!("{RELEASE_ROWS_SQL} WHERE t.oci_repository_id IN ({placeholders})");
            Statement::from_sql_and_values(
                backend,
                bind_placeholders(backend, &sql),
                ids.iter().copied().map(Into::into),
            )
        }
        None => Statement::from_string(backend, RELEASE_ROWS_SQL),
    };
    Ok(ReleaseRow::find_by_statement(statement).all(db).await?)
}

/// Prefer a valid OCI annotation, then config creation time, then index time.
/// Future publication dates are capped at the earliest indexing time.
pub(super) fn release_time(
    candidates: [Option<&str>; 2],
    indexed_at: DateTime<Utc>,
) -> DateTime<Utc> {
    candidates
        .into_iter()
        .flatten()
        .find_map(|s| DateTime::parse_from_rfc3339(s.trim()).ok())
        .map_or(indexed_at, |t| t.with_timezone(&Utc).min(indexed_at))
}
