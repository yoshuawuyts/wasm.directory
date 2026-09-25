//! Portable SQL selecting only tags whose own manifest supplies a match.

// Registered identities override extracted names, which can be synthetic for
// compiled components. Unregistered components have no usable WIT source name:
// their results use OCI identity, never a shared synthetic `root:component`.
// Unattached WIT packages still participate in the dependency graph.
const SOURCES: &str = "\
    package_sources AS ( \
        SELECT wp.id AS package_id, m.id AS manifest_id, m.digest AS digest, \
               r.id AS repo_id, r.registry AS registry, r.repository AS repository, \
               CASE \
                   WHEN r.wit_namespace IS NOT NULL AND r.wit_name IS NOT NULL \
                       THEN r.wit_namespace || ':' || r.wit_name \
                   WHEN r.kind = 'component' OR EXISTS ( \
                       SELECT 1 FROM wasm_component c WHERE c.oci_manifest_id = m.id \
                   ) THEN NULL \
                   ELSE wp.package_name \
               END AS source_name \
        FROM wit_package wp \
        LEFT JOIN oci_manifest m ON m.id = wp.oci_manifest_id \
        LEFT JOIN oci_repository r ON r.id = m.oci_repository_id \
    )";

const MATCHING_TAGS: &str = "\
    JOIN oci_tag t ON t.oci_repository_id = s.repo_id \
                  AND t.manifest_digest = s.digest";

/// The world-member table is selected solely by internal code.
#[derive(Clone, Copy)]
pub(super) enum WorldDirection {
    Import,
    Export,
}

/// Traverse package identities with UNION, not UNION ALL, so cycles terminate
/// without a depth cutoff. The final join rechecks the originating package's
/// own edges: a newer manifest with no matching edge cannot supply its tag.
pub(super) fn dependents() -> String {
    format!(
        "WITH RECURSIVE {SOURCES}, \
         reverse_dependencies(package_name) AS ( \
             SELECT CAST(? AS TEXT) \
             UNION \
             SELECT s.source_name \
             FROM package_sources s \
             JOIN wit_package_dependency d ON d.dependent_id = s.package_id \
             JOIN reverse_dependencies closure ON closure.package_name = d.declared_package \
             WHERE s.source_name IS NOT NULL \
         ) \
         SELECT DISTINCT s.repo_id AS repo_id, s.registry AS registry, \
                s.repository AS repository, s.source_name AS source_name, t.tag AS tag, \
                CAST(NULL AS BIGINT) AS world_id, CAST(NULL AS TEXT) AS world_name \
         FROM package_sources s \
         JOIN wit_package_dependency d ON d.dependent_id = s.package_id \
         JOIN reverse_dependencies closure ON closure.package_name = d.declared_package \
         {MATCHING_TAGS} \
         WHERE s.source_name IS NULL OR s.source_name <> ?"
    )
}

/// Match a world's own exact declarations, with no dependency traversal.
pub(super) fn worlds(direction: WorldDirection, with_interface: bool) -> String {
    let table = match direction {
        WorldDirection::Import => "wit_world_import",
        WorldDirection::Export => "wit_world_export",
    };
    let interface_filter = if with_interface {
        "AND member.declared_interface = ?"
    } else {
        ""
    };
    format!(
        "WITH {SOURCES} \
         SELECT DISTINCT s.repo_id AS repo_id, s.registry AS registry, \
                s.repository AS repository, s.source_name AS source_name, t.tag AS tag, \
                world.id AS world_id, world.name AS world_name \
         FROM package_sources s \
         JOIN wit_world world ON world.wit_package_id = s.package_id \
         JOIN {table} member ON member.wit_world_id = world.id \
         {MATCHING_TAGS} \
         WHERE member.declared_package = ? {interface_filter}"
    )
}
