//! The retrieval tiers' SQL. Every tier applies the same filter: the scope
//! levels the context searches; a user-scoped fact only for its owner; a
//! project fact for the principal's project or when a topic may widen it
//! (decided later by `visible()`); confirmed facts, proposed facts only for
//! their owner, and stale facts only when the profile injects them with a
//! marker; never a fact whose review is due or that is outside its validity
//! window. Each tier returns fact ids in rank order.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt_identifiers::{BrainFactId, UserId};
use systemprompt_traits::RepositoryError;

const MAX_QUERY_WORDS: usize = 32;
const MIN_WORD_CHARS: usize = 3;

/// The principal-side filter every tier applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TierFilter {
    pub user: UserId,
    pub project: Option<String>,
    pub scopes: Vec<String>,
    pub include_stale: bool,
    pub now: DateTime<Utc>,
    pub limit: i64,
}

pub async fn pinned(pool: &PgPool, f: &TierFilter) -> Result<Vec<BrainFactId>, RepositoryError> {
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
           ORDER BY importance_base DESC, id LIMIT $6"#,
        &f.scopes,
        f.user.as_str(),
        f.project.as_deref(),
        f.include_stale,
        f.now,
        f.limit,
    )
    .fetch_all(pool)
    .await?)
}

pub async fn by_subject(
    pool: &PgPool,
    f: &TierFilter,
    hints: &str,
) -> Result<Vec<BrainFactId>, RepositoryError> {
    if hints.is_empty() {
        return Ok(Vec::new());
    }
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
           ORDER BY m.score DESC, f.importance_base DESC, f.id LIMIT $6"#,
        &f.scopes,
        f.user.as_str(),
        f.project.as_deref(),
        f.include_stale,
        f.now,
        f.limit,
        hints,
    )
    .fetch_all(pool)
    .await?)
}

pub async fn by_context(
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
           ORDER BY importance_base DESC, updated_at DESC, id LIMIT $6"#,
        &f.scopes,
        f.user.as_str(),
        project,
        f.include_stale,
        f.now,
        f.limit,
    )
    .fetch_all(pool)
    .await?)
}

pub async fn full_text(
    pool: &PgPool,
    f: &TierFilter,
    hints: &str,
) -> Result<Vec<BrainFactId>, RepositoryError> {
    let query = search_query(hints);
    if query.is_empty() {
        return Ok(Vec::new());
    }
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
           ORDER BY ts_rank_cd(search, websearch_to_tsquery('simple', $7)) DESC, id
           LIMIT $6"#,
        &f.scopes,
        f.user.as_str(),
        f.project.as_deref(),
        f.include_stale,
        f.now,
        f.limit,
        query,
    )
    .fetch_all(pool)
    .await?)
}

#[must_use]
pub fn search_query(hints: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    for word in hints.split(|c: char| !c.is_alphanumeric()) {
        let word = word.to_lowercase();
        if word.chars().count() >= MIN_WORD_CHARS && !words.contains(&word) {
            words.push(word);
            if words.len() == MAX_QUERY_WORDS {
                break;
            }
        }
    }
    words.join(" or ")
}

#[must_use]
pub fn normalise_hints(hints: &str) -> String {
    hints
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
