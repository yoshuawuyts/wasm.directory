//! The shared popularity/listing count: distinct other repository IDs.

use std::fmt::Write as _;

use sea_orm::{DatabaseConnection, FromQueryResult, Statement};

use super::bind_placeholders;

// Keep the target-repository join: a mirror is another repository, so an
// otherwise self-referential edge can count against a different tagged mirror.
const DEPENDENTS_SQL: &str = "\
    SELECT repo.wit_namespace AS wit_namespace, repo.wit_name AS wit_name, \
           COUNT(DISTINCT dependent_repo.id) AS dependents \
    FROM wit_package_dependency wpd \
    JOIN wit_package wp ON wpd.dependent_id = wp.id \
    JOIN oci_manifest om ON wp.oci_manifest_id = om.id \
    JOIN oci_repository dependent_repo ON om.oci_repository_id = dependent_repo.id \
    JOIN oci_repository repo \
      ON repo.wit_namespace || ':' || repo.wit_name = wpd.declared_package \
    WHERE dependent_repo.id <> repo.id \
      AND EXISTS (SELECT 1 FROM oci_tag t WHERE t.oci_repository_id = repo.id)";

#[derive(FromQueryResult)]
pub(super) struct DependentCount {
    pub(super) wit_namespace: String,
    pub(super) wit_name: String,
    pub(super) dependents: i64,
}

/// Aggregate all identities for popularity, or only the identities on a page.
pub(super) async fn dependent_counts(
    db: &DatabaseConnection,
    identities: Option<&[String]>,
) -> anyhow::Result<Vec<DependentCount>> {
    let mut sql = DEPENDENTS_SQL.to_owned();
    let values: Vec<sea_orm::Value> = match identities {
        Some([]) => return Ok(Vec::new()),
        Some(names) => {
            let placeholders = vec!["?"; names.len()].join(", ");
            write!(sql, " AND wpd.declared_package IN ({placeholders})")
                .expect("writing to a String cannot fail");
            names.iter().cloned().map(Into::into).collect()
        }
        None => Vec::new(),
    };
    sql.push_str(
        " GROUP BY repo.wit_namespace, repo.wit_name \
          ORDER BY dependents DESC, repo.wit_namespace ASC, repo.wit_name ASC",
    );
    let backend = db.get_database_backend();
    let statement =
        Statement::from_sql_and_values(backend, bind_placeholders(backend, &sql), values);
    Ok(DependentCount::find_by_statement(statement).all(db).await?)
}
