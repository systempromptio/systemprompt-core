//! Candidates inherit immutable dependencies; comparisons require one resource.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ManagedRepository, NewRevision};
use crate::managed::error::invalid;
use crate::managed::{FileChange, ManagedError, Result, diff_files};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ManagedResourceId, ResourceRevisionId, UserId};

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextCandidate {
    pub path: String,
    pub content: String,
    pub rationale: String,
}

impl std::fmt::Debug for TextCandidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextCandidate")
            .field("path", &self.path)
            .field("content_bytes", &self.content.len())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RevisionComparison {
    pub baseline: ResourceRevisionId,
    pub candidate: ResourceRevisionId,
    pub changes: Vec<FileChange>,
    pub dependencies_changed: bool,
    pub source_snapshot_changed: bool,
}

impl ManagedRepository {
    pub async fn create_text_candidate(
        &self,
        owner: &UserId,
        baseline: &ResourceRevisionId,
        edit: &TextCandidate,
    ) -> Result<ResourceRevisionId> {
        if edit.content.len() > 1024 * 1024 {
            return Err(invalid("Text edits exceed 1 MiB"));
        }
        let resource = self.revision_resource(owner, baseline).await?;
        let manifest = self.get_revision(owner, baseline).await?;
        let mut files = self.get_revision_files(owner, baseline).await?;
        let file = files
            .0
            .get_mut(&edit.path)
            .ok_or_else(|| invalid("Candidate file is absent from the baseline"))?;
        if std::str::from_utf8(&file.bytes).is_err() {
            return Err(invalid("Binary files require an asset revision"));
        }
        if file.bytes == edit.content.as_bytes() {
            return Err(invalid("Candidate file is unchanged"));
        }
        file.bytes = edit.content.as_bytes().to_vec();
        self.create_revision(
            owner,
            &NewRevision {
                resource_id: resource,
                snapshot_id: manifest.snapshot_id,
                parent_id: Some(baseline.clone()),
                files,
                dependencies: manifest.dependencies,
                rationale: edit.rationale.clone(),
            },
        )
        .await
    }

    pub async fn compare_revisions(
        &self,
        owner: &UserId,
        baseline: &ResourceRevisionId,
        candidate: &ResourceRevisionId,
    ) -> Result<RevisionComparison> {
        if self.revision_resource(owner, baseline).await?
            != self.revision_resource(owner, candidate).await?
        {
            return Err(invalid("Compare revisions of the same resource"));
        }
        let before = self.get_revision(owner, baseline).await?;
        let after = self.get_revision(owner, candidate).await?;
        Ok(RevisionComparison {
            baseline: baseline.clone(),
            candidate: candidate.clone(),
            changes: diff_files(&before, &after),
            dependencies_changed: before.dependencies != after.dependencies,
            source_snapshot_changed: before.snapshot_id != after.snapshot_id,
        })
    }

    async fn revision_resource(
        &self,
        owner: &UserId,
        revision: &ResourceRevisionId,
    ) -> Result<ManagedResourceId> {
        let id = sqlx::query_scalar!(
            "SELECT resource_id FROM managed_revisions WHERE owner_id=$1 AND id=$2",
            owner.as_str(),
            revision.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ManagedError::Unavailable)?;
        Ok(ManagedResourceId::new(id))
    }
}
