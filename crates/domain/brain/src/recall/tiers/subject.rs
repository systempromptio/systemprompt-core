//! The subject and linked tiers over the matched subjects.
//!
//! Both tiers read the subjects [`super::matched`] resolved once for the
//! recall. The subject tier holds the candidates whose own subject is
//! matched; the linked tier holds the candidates one of whose
//! `brain_fact_links` names a matched subject, scored by `links_weight`.
//! The hop is one deep by construction: no link is followed from a linked
//! fact, and nothing is reached through a matched subject's other facts.
//! Each tier drives from its subject index (`idx_brain_facts_subject`,
//! `idx_brain_fact_links_subject`) and admits a fact through the
//! working-set probe and the common predicate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use systemprompt_identifiers::BrainFactId;
use systemprompt_traits::RepositoryError;

use super::TierFilter;
use super::matched::MatchedSubjects;

pub(super) async fn by_subject(
    pool: &PgPool,
    f: &TierFilter,
    matched: &MatchedSubjects,
) -> Result<Vec<BrainFactId>, RepositoryError> {
    if matched.is_empty() {
        return Ok(Vec::new());
    }
    Ok(sqlx::query_scalar!(
        r#"WITH matched AS (
               SELECT * FROM unnest($7::text[], $8::text[], $9::float8[])
               AS m(subject_kind, subject_id, score)
           ),
           hits AS (
               SELECT f.id, max(m.score) AS score
               FROM matched m
               JOIN brain_facts f
                 ON f.subject_kind = m.subject_kind AND f.subject_id = m.subject_id
               WHERE EXISTS (SELECT 1 FROM brain_working_set w
                             WHERE w.fact_id = f.id AND w.scope_key = ANY($1::text[]))
                 AND f.scope_level = ANY($6::text[])
                 AND (f.status = 'confirmed'
                      OR (f.status = 'proposed' AND f.owner_user_id = $2)
                      OR (f.status = 'stale' AND $3))
                 AND (f.review_after IS NULL OR f.review_after >= $4)
                 AND (f.valid_from IS NULL OR f.valid_from <= $4)
                 AND (f.valid_to IS NULL OR f.valid_to > $4)
                 AND f.observed_at <= $4
               GROUP BY f.id
           )
           SELECT h.id AS "id: BrainFactId"
           FROM hits h JOIN brain_facts f ON f.id = h.id
           ORDER BY h.score DESC, f.importance_base DESC, f.id
           LIMIT $5"#,
        &f.keys,
        f.user.as_str(),
        f.include_stale,
        f.as_of,
        f.limit,
        &f.scopes,
        &matched.kinds,
        &matched.ids,
        &matched.scores,
    )
    .fetch_all(pool)
    .await?)
}

pub(super) async fn by_linked(
    pool: &PgPool,
    f: &TierFilter,
    matched: &MatchedSubjects,
) -> Result<Vec<BrainFactId>, RepositoryError> {
    if matched.is_empty() {
        return Ok(Vec::new());
    }
    Ok(sqlx::query_scalar!(
        r#"WITH matched AS (
               SELECT * FROM unnest($7::text[], $8::text[], $9::float8[])
               AS m(subject_kind, subject_id, score)
           ),
           hits AS (
               SELECT f.id, max(m.score * $10) AS score
               FROM matched m
               JOIN brain_fact_links l
                 ON l.subject_kind = m.subject_kind AND l.subject_id = m.subject_id
               JOIN brain_facts f ON f.id = l.fact_id
               WHERE NOT (f.subject_kind = m.subject_kind AND f.subject_id = m.subject_id)
                 AND EXISTS (SELECT 1 FROM brain_working_set w
                             WHERE w.fact_id = f.id AND w.scope_key = ANY($1::text[]))
                 AND f.scope_level = ANY($6::text[])
                 AND (f.status = 'confirmed'
                      OR (f.status = 'proposed' AND f.owner_user_id = $2)
                      OR (f.status = 'stale' AND $3))
                 AND (f.review_after IS NULL OR f.review_after >= $4)
                 AND (f.valid_from IS NULL OR f.valid_from <= $4)
                 AND (f.valid_to IS NULL OR f.valid_to > $4)
                 AND f.observed_at <= $4
               GROUP BY f.id
           )
           SELECT h.id AS "id: BrainFactId"
           FROM hits h JOIN brain_facts f ON f.id = h.id
           ORDER BY h.score DESC, f.importance_base DESC, f.id
           LIMIT $5"#,
        &f.keys,
        f.user.as_str(),
        f.include_stale,
        f.as_of,
        f.limit,
        &f.scopes,
        &matched.kinds,
        &matched.ids,
        &matched.scores,
        f.links_weight,
    )
    .fetch_all(pool)
    .await?)
}
