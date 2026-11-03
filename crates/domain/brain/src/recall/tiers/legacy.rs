//! The scope-predicate tier SQL recall ran before the working set, kept
//! for `brain.recall.working_set: false` and removed in the next release.
//! Each tier filters `brain_facts` by scope level and key for the
//! principal; a project fact with topics is admitted here and decided by
//! `visible()`. The only change from the original is the `as_of` instant
//! and the `observed_at` predicate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use systemprompt_identifiers::BrainFactId;
use systemprompt_traits::RepositoryError;

use super::TierFilter;

pub(super) async fn pinned(
    pool: &PgPool,
    f: &TierFilter,
) -> Result<Vec<BrainFactId>, RepositoryError> {
    Ok(sqlx::query_scalar!(
        r#"SELECT id AS "id: BrainFactId" FROM brain_facts
           WHERE pinned
             AND scope_level = ANY($1::text[])
             AND (scope_level IN ('org', 'group')
                  OR (scope_level = 'user' AND scope_key = $2)
                  OR (scope_level = 'project' AND (scope_key = $3 OR cardinality(topics) > 0)))
             AND (status = 'confirmed'
                  OR (status = 'proposed' AND owner_user_id = $2)
                  OR (status = 'stale' AND $4))
             AND (review_after IS NULL OR review_after >= $5)
             AND (valid_from IS NULL OR valid_from <= $5)
             AND (valid_to IS NULL OR valid_to > $5)
             AND observed_at <= $5
           ORDER BY importance_base DESC, id LIMIT $6"#,
        &f.scopes,
        f.user.as_str(),
        f.project.as_deref(),
        f.include_stale,
        f.as_of,
        f.limit,
    )
    .fetch_all(pool)
    .await?)
}

pub(super) async fn by_subject(
    pool: &PgPool,
    f: &TierFilter,
    hints: &str,
) -> Result<Vec<BrainFactId>, RepositoryError> {
    Ok(sqlx::query_scalar!(
        r#"WITH matched AS (
               SELECT subject_kind, subject_id, max(score) AS score FROM (
                   SELECT a.subject_kind, a.subject_id,
                          CASE WHEN strpos($7, a.alias_norm) > 0 THEN 1.0::float8
                               ELSE word_similarity(a.alias_norm, $7)::float8 END AS score
                   FROM brain_aliases a
                   WHERE strpos($7, a.alias_norm) > 0 OR a.alias_norm <% $7
                   UNION ALL
                   SELECT s.subject_kind, s.subject_id, 1.0::float8
                   FROM brain_subjects s
                   WHERE length(s.subject_id) >= 3 AND strpos($7, lower(s.subject_id)) > 0
               ) hits GROUP BY subject_kind, subject_id
           )
           SELECT f.id AS "id: BrainFactId" FROM brain_facts f
           JOIN matched m ON m.subject_kind = f.subject_kind AND m.subject_id = f.subject_id
           WHERE f.scope_level = ANY($1::text[])
             AND (f.scope_level IN ('org', 'group')
                  OR (f.scope_level = 'user' AND f.scope_key = $2)
                  OR (f.scope_level = 'project' AND (f.scope_key = $3 OR cardinality(f.topics) > 0)))
             AND (f.status = 'confirmed'
                  OR (f.status = 'proposed' AND f.owner_user_id = $2)
                  OR (f.status = 'stale' AND $4))
             AND (f.review_after IS NULL OR f.review_after >= $5)
             AND (f.valid_from IS NULL OR f.valid_from <= $5)
             AND (f.valid_to IS NULL OR f.valid_to > $5)
             AND f.observed_at <= $5
           ORDER BY m.score DESC, f.importance_base DESC, f.id LIMIT $6"#,
        &f.scopes,
        f.user.as_str(),
        f.project.as_deref(),
        f.include_stale,
        f.as_of,
        f.limit,
        hints,
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
    Ok(sqlx::query_scalar!(
        r#"SELECT id AS "id: BrainFactId" FROM brain_facts
           WHERE scope_level = 'project' AND scope_key = $3
             AND 'project' = ANY($1::text[])
             AND (status = 'confirmed'
                  OR (status = 'proposed' AND owner_user_id = $2)
                  OR (status = 'stale' AND $4))
             AND (review_after IS NULL OR review_after >= $5)
             AND (valid_from IS NULL OR valid_from <= $5)
             AND (valid_to IS NULL OR valid_to > $5)
             AND observed_at <= $5
           ORDER BY importance_base DESC, id LIMIT $6"#,
        &f.scopes,
        f.user.as_str(),
        project,
        f.include_stale,
        f.as_of,
        f.limit,
    )
    .fetch_all(pool)
    .await?)
}

pub(super) async fn full_text(
    pool: &PgPool,
    f: &TierFilter,
    query: &str,
) -> Result<Vec<BrainFactId>, RepositoryError> {
    Ok(sqlx::query_scalar!(
        r#"SELECT id AS "id: BrainFactId" FROM brain_facts
           WHERE search @@ websearch_to_tsquery('simple', $7)
             AND scope_level = ANY($1::text[])
             AND (scope_level IN ('org', 'group')
                  OR (scope_level = 'user' AND scope_key = $2)
                  OR (scope_level = 'project' AND (scope_key = $3 OR cardinality(topics) > 0)))
             AND (status = 'confirmed'
                  OR (status = 'proposed' AND owner_user_id = $2)
                  OR (status = 'stale' AND $4))
             AND (review_after IS NULL OR review_after >= $5)
             AND (valid_from IS NULL OR valid_from <= $5)
             AND (valid_to IS NULL OR valid_to > $5)
             AND observed_at <= $5
           ORDER BY ts_rank_cd(search, websearch_to_tsquery('simple', $7)) DESC,
                    importance_base DESC, id
           LIMIT $6"#,
        &f.scopes,
        f.user.as_str(),
        f.project.as_deref(),
        f.include_stale,
        f.as_of,
        f.limit,
        query,
    )
    .fetch_all(pool)
    .await?)
}
