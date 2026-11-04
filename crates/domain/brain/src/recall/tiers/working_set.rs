//! The pinned and context tiers and the lifecycle count over the
//! principal's candidate set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use sqlx::PgPool;
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::WithheldReason;
use systemprompt_traits::RepositoryError;

use super::TierFilter;
use crate::recall::held::project_key;

pub(super) async fn pinned(
    pool: &PgPool,
    f: &TierFilter,
) -> Result<Vec<BrainFactId>, RepositoryError> {
    Ok(sqlx::query_scalar!(
        r#"WITH cand AS (
               SELECT DISTINCT w.fact_id FROM brain_working_set w
               WHERE w.scope_key = ANY($1::text[])
           )
           SELECT f.id AS "id: BrainFactId"
           FROM cand c JOIN brain_facts f ON f.id = c.fact_id
           WHERE f.pinned
             AND f.scope_level = ANY($6::text[])
             AND (f.status = 'confirmed'
                  OR (f.status = 'proposed' AND f.owner_user_id = $2)
                  OR (f.status = 'stale' AND $3))
             AND (f.review_after IS NULL OR f.review_after >= $4)
             AND (f.valid_from IS NULL OR f.valid_from <= $4)
             AND (f.valid_to IS NULL OR f.valid_to > $4)
             AND f.observed_at <= $4
           ORDER BY f.importance_base DESC, f.id
           LIMIT $5"#,
        &f.keys,
        f.user.as_str(),
        f.include_stale,
        f.as_of,
        f.limit,
        &f.scopes,
    )
    .fetch_all(pool)
    .await?)
}

pub(super) async fn by_context(
    pool: &PgPool,
    f: &TierFilter,
) -> Result<Vec<BrainFactId>, RepositoryError> {
    let Some(project) = f.project.as_deref() else {
        return Ok(Vec::new());
    };
    let key = project_key(project);
    Ok(sqlx::query_scalar!(
        r#"WITH cand AS (
               SELECT DISTINCT w.fact_id FROM brain_working_set w
               WHERE w.scope_key = $7 AND w.scope_key = ANY($1::text[])
           )
           SELECT f.id AS "id: BrainFactId"
           FROM cand c JOIN brain_facts f ON f.id = c.fact_id
           WHERE f.scope_level = 'project'
             AND 'project' = ANY($6::text[])
             AND (f.status = 'confirmed'
                  OR (f.status = 'proposed' AND f.owner_user_id = $2)
                  OR (f.status = 'stale' AND $3))
             AND (f.review_after IS NULL OR f.review_after >= $4)
             AND (f.valid_from IS NULL OR f.valid_from <= $4)
             AND (f.valid_to IS NULL OR f.valid_to > $4)
             AND f.observed_at <= $4
           ORDER BY f.importance_base DESC, f.id
           LIMIT $5"#,
        &f.keys,
        f.user.as_str(),
        f.include_stale,
        f.as_of,
        f.limit,
        &f.scopes,
        key,
    )
    .fetch_all(pool)
    .await?)
}

pub(super) async fn withheld_counts(
    pool: &PgPool,
    f: &TierFilter,
) -> Result<BTreeMap<WithheldReason, u32>, RepositoryError> {
    let row = sqlx::query!(
        r#"WITH cand AS (
               SELECT DISTINCT w.fact_id FROM brain_working_set w
               WHERE w.scope_key = ANY($1::text[])
           )
           SELECT count(*) FILTER (WHERE f.status = 'stale' AND NOT $3) AS "stale!",
                  count(*) FILTER (WHERE f.status = 'disputed') AS "disputed!",
                  count(*) FILTER (
                      WHERE (f.status IN ('confirmed', 'proposed') OR (f.status = 'stale' AND $3))
                        AND f.review_after IS NOT NULL AND f.review_after < $4
                  ) AS "review_due!"
           FROM cand c JOIN brain_facts f ON f.id = c.fact_id
           WHERE f.scope_level = ANY($5::text[])
             AND (f.status <> 'proposed' OR f.owner_user_id = $2)
             AND (f.valid_from IS NULL OR f.valid_from <= $4)
             AND (f.valid_to IS NULL OR f.valid_to > $4)
             AND f.observed_at <= $4"#,
        &f.keys,
        f.user.as_str(),
        f.include_stale,
        f.as_of,
        &f.scopes,
    )
    .fetch_one(pool)
    .await?;
    let mut counts = BTreeMap::new();
    for (reason, count) in [
        (WithheldReason::Stale, row.stale),
        (WithheldReason::Disputed, row.disputed),
        (WithheldReason::ReviewDue, row.review_due),
    ] {
        if count > 0 {
            counts.insert(reason, u32::try_from(count).unwrap_or(u32::MAX));
        }
    }
    Ok(counts)
}
