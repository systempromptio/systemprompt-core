//! The full-text tier: the weighted search vector (`A` identity fields,
//! `B` other fields, `C` statement and synonym expansions) filtered by the
//! plan's required words (served by the GIN index) and ranked by the
//! cover density of every word, normalised by document length. A hit is
//! admitted through the working-set probe and the common predicate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use systemprompt_identifiers::BrainFactId;
use systemprompt_traits::RepositoryError;

use super::TierFilter;
use crate::recall::search_plan::SearchPlan;

pub(super) async fn full_text(
    pool: &PgPool,
    f: &TierFilter,
    plan: &SearchPlan,
) -> Result<Vec<BrainFactId>, RepositoryError> {
    Ok(sqlx::query_scalar!(
        r#"SELECT f.id AS "id: BrainFactId" FROM brain_facts f
           WHERE f.search_weighted @@ to_tsquery('simple', $7)
             AND EXISTS (SELECT 1 FROM v_brain_working_set w
                         WHERE w.fact_id = f.id AND w.scope_key = ANY($1::text[]))
             AND f.scope_level = ANY($6::text[])
             AND (f.status = 'confirmed'
                  OR (f.status = 'proposed' AND f.owner_user_id = $2)
                  OR (f.status = 'stale' AND $3))
             AND (f.review_after IS NULL OR f.review_after >= $4)
             AND (f.valid_from IS NULL OR f.valid_from <= $4)
             AND (f.valid_to IS NULL OR f.valid_to > $4)
             AND f.observed_at <= $4
           ORDER BY ts_rank_cd(f.search_weighted, to_tsquery('simple', $8), 32) DESC,
                    f.importance_base DESC, f.id
           LIMIT $5"#,
        &f.keys,
        f.user.as_str(),
        f.include_stale,
        f.as_of,
        f.limit,
        &f.scopes,
        &plan.filter,
        &plan.rank,
    )
    .fetch_all(pool)
    .await?)
}
