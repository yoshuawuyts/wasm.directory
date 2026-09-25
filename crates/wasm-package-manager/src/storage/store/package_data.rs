//! Page-batched tags and first indexed manifest descriptions.

use std::collections::{BTreeSet, HashMap};

use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, FromQueryResult, QueryFilter, Statement,
};
use tokio_stream::StreamExt;
use wasm_package_manager_migration::entities::{oci_repository, oci_tag};

use super::{bind_placeholders, package_metadata::BATCH_SIZE, sort_versioned_tags};

type VersionedTags = HashMap<i64, Vec<(semver::Version, String)>>;

pub(super) struct PackageData {
    tags: HashMap<i64, Vec<String>>,
    descriptions: HashMap<i64, String>,
}

impl PackageData {
    pub(super) async fn load(
        db: &DatabaseConnection,
        repos: &[oci_repository::Model],
    ) -> anyhow::Result<Self> {
        let ids: Vec<_> = repos
            .iter()
            .map(|repo| repo.id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut tags = HashMap::new();
        let mut descriptions = HashMap::new();
        for batch in ids.chunks(BATCH_SIZE) {
            load_tags(db, batch, &mut tags).await?;
            for row in load_descriptions(db, batch).await? {
                descriptions.insert(row.repo_id, row.description);
            }
        }
        Ok(Self {
            tags: tags
                .into_iter()
                .map(|(id, tags)| (id, sort_versioned_tags(tags)))
                .collect(),
            descriptions,
        })
    }

    pub(super) fn tags(&self, repo_id: i64) -> Vec<String> {
        self.tags.get(&repo_id).cloned().unwrap_or_default()
    }

    pub(super) fn description(&self, repo_id: i64) -> Option<String> {
        self.descriptions.get(&repo_id).cloned()
    }
}

async fn load_tags(
    db: &DatabaseConnection,
    ids: &[i64],
    tags: &mut VersionedTags,
) -> anyhow::Result<()> {
    let mut rows = oci_tag::Entity::find()
        .filter(oci_tag::Column::OciRepositoryId.is_in(ids.iter().copied()))
        .stream(db)
        .await?;
    while let Some(row) = rows.next().await {
        let row = row?;
        let Some(version) = crate::manager::parse_tag_as_semver(&row.tag) else {
            continue;
        };
        tags.entry(row.oci_repository_id)
            .or_default()
            .push((version, row.tag));
    }
    Ok(())
}

#[derive(FromQueryResult)]
struct Description {
    repo_id: i64,
    description: String,
}

async fn load_descriptions(
    db: &DatabaseConnection,
    ids: &[i64],
) -> anyhow::Result<Vec<Description>> {
    let placeholders = vec!["?"; ids.len()].join(", ");
    let sql = format!(
        "SELECT m.oci_repository_id AS repo_id, m.oci_description AS description \
         FROM oci_manifest m \
         JOIN ( \
             SELECT MIN(id) AS id FROM oci_manifest \
             WHERE oci_repository_id IN ({placeholders}) AND oci_description IS NOT NULL \
             GROUP BY oci_repository_id \
         ) first_description ON first_description.id = m.id"
    );
    let backend = db.get_database_backend();
    let statement = Statement::from_sql_and_values(
        backend,
        bind_placeholders(backend, &sql),
        ids.iter().copied().map(Into::into),
    );
    Ok(Description::find_by_statement(statement).all(db).await?)
}
