//! The effective level of many users on one document in a single query:
//! each user's subject pairs are tagged with the user's index, and the best
//! unexpired grant on the document or its folder chain is taken per index.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentFolderId, DocumentId};
use systemprompt_models::documents::AccessLevel;

use super::grant_row::level_from_rank;
use super::grants::DocumentGrantRepository;
use super::rows::RepoResult;
use crate::documents::models::SubjectPairs;

impl DocumentGrantRepository {
    /// One level per entry of `subjects`, in order; `None` for no grant.
    pub async fn effective_levels(
        &self,
        document_id: &DocumentId,
        folder_ancestors: &[DocumentFolderId],
        subjects: &[SubjectPairs],
    ) -> RepoResult<Vec<Option<AccessLevel>>> {
        let folders: Vec<String> = folder_ancestors
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();
        let mut owners: Vec<i32> = Vec::new();
        let mut kinds: Vec<String> = Vec::new();
        let mut values: Vec<String> = Vec::new();
        for (index, pairs) in subjects.iter().enumerate() {
            let owner = i32::try_from(index).unwrap_or(i32::MAX);
            for (kind, value) in pairs.kinds.iter().zip(&pairs.values) {
                owners.push(owner);
                kinds.push(kind.clone());
                values.push(value.clone());
            }
        }
        let rows = sqlx::query!(
            r#"SELECT p.owner AS "owner!",
                      MAX(CASE g.access_level
                          WHEN 'view' THEN 1 WHEN 'comment' THEN 2 WHEN 'edit' THEN 3
                          WHEN 'share' THEN 4 WHEN 'own' THEN 5 END) AS "rank"
               FROM UNNEST($3::int4[], $4::text[], $5::text[]) AS p(owner, kind, value)
               JOIN document_grants g
                 ON g.subject_kind = p.kind AND g.subject_value = p.value
               WHERE (g.document_id = $1::text::uuid OR g.folder_id = ANY($2::text[]::uuid[]))
                 AND (g.valid_until IS NULL OR g.valid_until > now())
               GROUP BY p.owner"#,
            document_id.as_str(),
            &folders,
            &owners,
            &kinds,
            &values,
        )
        .fetch_all(&*self.pools.read)
        .await?;
        let mut levels = vec![None; subjects.len()];
        for row in rows {
            let level = row.rank.and_then(level_from_rank);
            if let Some(slot) = usize::try_from(row.owner).ok().and_then(|i| levels.get_mut(i)) {
                *slot = level;
            }
        }
        Ok(levels)
    }
}
