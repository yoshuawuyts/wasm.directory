//! Publisher-supplied creation times, without release-ranking fallbacks.

use chrono::{DateTime, Utc};
use tracing::warn;
use wasm_package_manager_migration::entities::oci_manifest;

/// Parse the first usable publisher timestamp, preferring the annotation
/// over the config. This does not substitute or clamp to an indexing time.
pub(super) fn publisher_time(candidates: [Option<&str>; 2]) -> Option<DateTime<Utc>> {
    candidates
        .into_iter()
        .flatten()
        .find_map(|value| DateTime::parse_from_rfc3339(value.trim()).ok())
        .map(|time| time.with_timezone(&Utc))
}

/// Select the manifest's publisher creation time, rejecting timestamps after
/// we already recorded this immutable content rather than inventing a date.
pub(super) fn manifest_created_at(manifest: &oci_manifest::Model) -> Option<String> {
    [
        (
            "org.opencontainers.image.created",
            manifest.oci_created.as_deref(),
        ),
        ("config.created", manifest.config_created.as_deref()),
    ]
    .into_iter()
    .find_map(|(source, candidate)| manifest_candidate(manifest, source, candidate))
    .map(|time| time.to_rfc3339())
}

fn manifest_candidate(
    manifest: &oci_manifest::Model,
    source: &str,
    candidate: Option<&str>,
) -> Option<DateTime<Utc>> {
    let candidate = candidate?.trim();
    if candidate.is_empty() {
        return None;
    }
    match publisher_time([Some(candidate), None]) {
        Some(time) if time <= manifest.created_at => Some(time),
        _ => {
            warn!(
                manifest_id = manifest.id,
                repository_id = manifest.oci_repository_id,
                digest = %manifest.digest,
                source,
                "Ignoring invalid publisher creation time or time after manifest indexing"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests;
