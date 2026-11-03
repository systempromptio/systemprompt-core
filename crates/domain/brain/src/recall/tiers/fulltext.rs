//! The full-text tier over the candidate set: the weighted search vector
//! (`A` identity fields, `B` other fields, `C` statement and synonym
//! expansions) against the synonym-expanded query, ranked by cover
//! density normalised by document length.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use systemprompt_identifiers::BrainFactId;
use systemprompt_traits::RepositoryError;

use super::TierFilter;

pub(super) async fn full_text(
    pool: &PgPool,
    f: &TierFilter,
    query: &str,
) -> Result<Vec<BrainFactId>, RepositoryError> {
    Ok(sqlx::query_scalar!(
        r#"WITH cand AS (
               SELECT DISTINCT w.fact_id FROM brain_working_set w
               WHERE w.scope_key = ANY($1::text[])
           )
           SELECT f.id AS "id: BrainFactId"
           FROM cand c JOIN brain_facts f ON f.id = c.fact_id
           WHERE f.search_weighted @@ websearch_to_tsquery('simple', $7)
             AND f.scope_level = ANY($6::text[])
             AND (f.status = 'confirmed'
                  OR (f.status = 'proposed' AND f.owner_user_id = $2)
                  OR (f.status = 'stale' AND $3))
             AND (f.review_after IS NULL OR f.review_after >= $4)
             AND (f.valid_from IS NULL OR f.valid_from <= $4)
             AND (f.valid_to IS NULL OR f.valid_to > $4)
             AND f.observed_at <= $4
           ORDER BY ts_rank_cd(f.search_weighted, websearch_to_tsquery('simple', $7), 32) DESC,
                    f.importance_base DESC, f.id
           LIMIT $5"#,
        &f.keys,
        f.user.as_str(),
        f.include_stale,
        f.as_of,
        f.limit,
        &f.scopes,
        query,
    )
    .fetch_all(pool)
    .await?)
}
