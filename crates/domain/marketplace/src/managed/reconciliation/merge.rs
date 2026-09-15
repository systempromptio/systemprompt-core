//! Three-way merge verification for a resolved reconciliation revision.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::{AssetFile, ManagedError, Result, RevisionFiles};

pub(super) struct RecordedConflict {
    pub path: String,
    pub resolution: Option<String>,
    pub resolved_digest: Option<String>,
}

pub(super) struct ThreeWay<'a> {
    pub base: &'a RevisionFiles,
    pub candidate: &'a RevisionFiles,
    pub incoming: &'a RevisionFiles,
}

pub(super) fn verify_merge(
    sides: &ThreeWay<'_>,
    resolved: &RevisionFiles,
    conflicts: &[RecordedConflict],
) -> Result<()> {
    let mut paths = std::collections::BTreeSet::new();
    paths.extend(sides.base.0.keys().cloned());
    paths.extend(sides.candidate.0.keys().cloned());
    paths.extend(sides.incoming.0.keys().cloned());
    paths.extend(resolved.0.keys().cloned());
    for path in paths {
        let expected = expected_file(sides, resolved, conflicts, &path)?;
        if !same_file(resolved.0.get(&path), expected) {
            return Err(ManagedError::Conflict(format!(
                "Resolved revision differs from the recorded merge at {path}"
            )));
        }
    }
    Ok(())
}

fn expected_file<'a>(
    sides: &ThreeWay<'a>,
    resolved: &'a RevisionFiles,
    conflicts: &[RecordedConflict],
    path: &str,
) -> Result<Option<&'a AssetFile>> {
    let base_file = sides.base.0.get(path);
    let candidate_file = sides.candidate.0.get(path);
    let incoming_file = sides.incoming.0.get(path);
    if same_file(candidate_file, incoming_file) {
        return Ok(candidate_file);
    }
    if same_file(candidate_file, base_file) {
        return Ok(incoming_file);
    }
    if same_file(incoming_file, base_file) {
        return Ok(candidate_file);
    }
    let conflict = conflicts
        .iter()
        .find(|conflict| conflict.path == path)
        .ok_or_else(|| ManagedError::Conflict("Three-way conflict record is missing".to_owned()))?;
    match conflict.resolution.as_deref() {
        Some("candidate") => Ok(candidate_file),
        Some("incoming") => Ok(incoming_file),
        Some("delete") => Ok(None),
        Some("manual") => {
            let actual = resolved.0.get(path).ok_or_else(|| {
                ManagedError::Conflict("Manual resolution content is missing".to_owned())
            })?;
            let digest = super::super::AssetDigest::of(&actual.bytes);
            if conflict.resolved_digest.as_deref() != Some(digest.as_str()) {
                return Err(ManagedError::Conflict(
                    "Manual resolution digest does not match supplied content".to_owned(),
                ));
            }
            Ok(Some(actual))
        },
        _ => Err(ManagedError::Conflict(
            "Invalid reconciliation resolution".to_owned(),
        )),
    }
}

pub(super) fn same_file(left: Option<&AssetFile>, right: Option<&AssetFile>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.bytes == right.bytes
                && left.media_type == right.media_type
                && left.executable == right.executable
        },
        _ => false,
    }
}
