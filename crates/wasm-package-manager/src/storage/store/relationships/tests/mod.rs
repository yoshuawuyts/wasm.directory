mod dependents;
mod fixtures;
mod pagination;
mod worlds;

use sea_orm::{ConnectionTrait, DbBackend, Statement};
use wasm_meta_registry_types::RelationshipTarget;

use super::super::{Store, bind_placeholders};
use fixtures::{fixture_store, package, repository, seed_release};

fn target(package: &str, interface: Option<&str>) -> RelationshipTarget {
    RelationshipTarget::new(package, interface).expect("valid relationship target")
}

#[test]
fn relationship_queries_bind_for_postgres_and_sqlite() {
    let queries = [
        (super::queries::dependents(), 2),
        (
            super::queries::worlds(super::WorldDirection::Import, false),
            1,
        ),
        (
            super::queries::worlds(super::WorldDirection::Export, true),
            2,
        ),
    ];
    for (query, count) in queries {
        assert_eq!(query.matches('?').count(), count);
        assert_eq!(bind_placeholders(DbBackend::Sqlite, &query), query);
        let postgres = bind_placeholders(DbBackend::Postgres, &query);
        assert!(!postgres.contains('?'));
        assert!(postgres.contains("$1"));
        assert_eq!(postgres.contains("$2"), count == 2);
    }
}

#[tokio::test]
async fn relationship_query_failures_are_not_empty_pages() {
    for table in [
        "wit_package_dependency",
        "wit_world_import",
        "wit_world_export",
    ] {
        let store = fixture_store().await;
        store
            .db
            .execute_unprepared(&format!("DROP TABLE {table}"))
            .await
            .expect("remove relationship table");
        let query = target("wasi:io", None);
        let failed = match table {
            "wit_package_dependency" => store.list_dependents(&query, 0, 10).await.is_err(),
            "wit_world_import" => store.list_importing_worlds(&query, 0, 10).await.is_err(),
            _ => store.list_exporting_worlds(&query, 0, 10).await.is_err(),
        };
        assert!(failed, "{table} query must propagate the SQL failure");
    }
}

#[tokio::test]
async fn relationship_candidate_decode_failures_are_not_empty_pages() {
    let store = fixture_store().await;
    let source = package(&store, "test:source", "1.0.0", &["wasi:io"]).await;
    store
        .db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE oci_repository SET registry = X'80' WHERE id = ?",
            [source.repo_id.into()],
        ))
        .await
        .expect("corrupt candidate text");
    assert!(
        store
            .list_dependents(&target("wasi:io", None), 0, 10)
            .await
            .is_err()
    );
}
