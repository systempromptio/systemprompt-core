//! The subject tier over the candidate set.
//!
//! A subject is matched when one of its aliases occurs in, or is
//! trigram-similar to, the normalised hints, or its id occurs in them
//! (score `1.0` for an occurrence, the word similarity otherwise). A
//! candidate fact hits directly when its own subject is matched, and at
//! one hop (`links_weight`) when one of its `brain_fact_links` names a
//! matched subject or its subject is linked from a matched subject's
//! fact. The hops are one `UNION ALL` deep by construction; no link is
//! followed from a linked subject.
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
           ),
           hits AS (
               SELECT f.id, m.score
               FROM cand c
               JOIN brain_facts f ON f.id = c.fact_id
               JOIN matched m ON m.subject_kind = f.subject_kind AND m.subject_id = f.subject_id
               UNION ALL
               SELECT l.fact_id, m.score * $8
               FROM brain_fact_links l
               JOIN matched m ON m.subject_kind = l.subject_kind AND m.subject_id = l.subject_id
               JOIN cand c ON c.fact_id = l.fact_id
               UNION ALL
               SELECT f.id, m.score * $8
               FROM matched m
               JOIN brain_facts mf ON mf.subject_kind = m.subject_kind AND mf.subject_id = m.subject_id
               JOIN brain_fact_links l ON l.fact_id = mf.id
               JOIN brain_facts f ON f.subject_kind = l.subject_kind AND f.subject_id = l.subject_id
               JOIN cand c ON c.fact_id = f.id
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
    )
    .fetch_all(pool)
    .await?)
}
