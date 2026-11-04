//! The subject and linked tiers over the candidate set.
//!
//! A subject is matched when one of its aliases occurs in, or is
//! trigram-similar to, the normalised hints, or its id occurs in them
//! (score `1.0` for an occurrence, the word similarity otherwise). The
//! subject tier holds the candidates whose own subject is matched; the
//! linked tier holds the candidates one of whose `brain_fact_links` names
//! a matched subject, scored by `links_weight`. The hop is one deep by
//! construction: no link is followed from a linked fact, and nothing is
//! reached through a matched subject's other facts.
//!
//! A subject is named only when its best score
//! reaches `alias_min_sim`, and the `max_subjects` best-scoring subjects
//! (ties by kind, then id) are the ones both tiers read. The trigram
//! operator keeps its own threshold (`pg_trgm.word_similarity_threshold`,
//! 0.6), so a lower `alias_min_sim` admits nothing more; a higher one
//! narrows the match.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use systemprompt_identifiers::BrainFactId;
use systemprompt_traits::RepositoryError;

use super::TierFilter;

pub(super) async fn by_subject(
    pool: &PgPool,
    f: &TierFilter,
    hints: &str,
) -> Result<Vec<BrainFactId>, RepositoryError> {
    Ok(sqlx::query_scalar!(
        r#"WITH cand AS (
               SELECT DISTINCT w.fact_id FROM brain_working_set w
               WHERE w.scope_key = ANY($1::text[])
           ),
           matched AS (
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
               HAVING max(score) >= $8
               ORDER BY max(score) DESC, subject_kind, subject_id
               LIMIT $9
           ),
           hits AS (
               SELECT f.id, m.score
               FROM cand c
               JOIN brain_facts f ON f.id = c.fact_id
               JOIN matched m ON m.subject_kind = f.subject_kind AND m.subject_id = f.subject_id
           ),
           ranked AS (SELECT id, max(score) AS score FROM hits GROUP BY id)
           SELECT f.id AS "id: BrainFactId"
           FROM ranked r JOIN brain_facts f ON f.id = r.id
           WHERE f.scope_level = ANY($6::text[])
             AND (f.status = 'confirmed'
                  OR (f.status = 'proposed' AND f.owner_user_id = $2)
                  OR (f.status = 'stale' AND $3))
             AND (f.review_after IS NULL OR f.review_after >= $4)
             AND (f.valid_from IS NULL OR f.valid_from <= $4)
             AND (f.valid_to IS NULL OR f.valid_to > $4)
             AND f.observed_at <= $4
           ORDER BY r.score DESC, f.importance_base DESC, f.id
           LIMIT $5"#,
        &f.keys,
        f.user.as_str(),
        f.include_stale,
        f.as_of,
        f.limit,
        &f.scopes,
        hints,
        f.alias_min_sim,
        f.max_subjects,
    )
    .fetch_all(pool)
    .await?)
}

pub(super) async fn by_linked(
    pool: &PgPool,
    f: &TierFilter,
    hints: &str,
) -> Result<Vec<BrainFactId>, RepositoryError> {
    Ok(sqlx::query_scalar!(
        r#"WITH cand AS (
               SELECT DISTINCT w.fact_id FROM brain_working_set w
               WHERE w.scope_key = ANY($1::text[])
           ),
           matched AS (
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
               HAVING max(score) >= $9
               ORDER BY max(score) DESC, subject_kind, subject_id
               LIMIT $10
           ),
           hits AS (
               SELECT l.fact_id AS id, m.score * $8 AS score
               FROM brain_fact_links l
               JOIN matched m ON m.subject_kind = l.subject_kind AND m.subject_id = l.subject_id
               JOIN cand c ON c.fact_id = l.fact_id
               JOIN brain_facts lf ON lf.id = l.fact_id
               WHERE NOT (lf.subject_kind = m.subject_kind AND lf.subject_id = m.subject_id)
           ),
           ranked AS (SELECT id, max(score) AS score FROM hits GROUP BY id)
           SELECT f.id AS "id: BrainFactId"
           FROM ranked r JOIN brain_facts f ON f.id = r.id
           WHERE f.scope_level = ANY($6::text[])
             AND (f.status = 'confirmed'
                  OR (f.status = 'proposed' AND f.owner_user_id = $2)
                  OR (f.status = 'stale' AND $3))
             AND (f.review_after IS NULL OR f.review_after >= $4)
             AND (f.valid_from IS NULL OR f.valid_from <= $4)
             AND (f.valid_to IS NULL OR f.valid_to > $4)
             AND f.observed_at <= $4
           ORDER BY r.score DESC, f.importance_base DESC, f.id
           LIMIT $5"#,
        &f.keys,
        f.user.as_str(),
        f.include_stale,
        f.as_of,
        f.limit,
        &f.scopes,
        hints,
        f.links_weight,
        f.alias_min_sim,
        f.max_subjects,
    )
    .fetch_all(pool)
    .await?)
}
