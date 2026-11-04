//! The linked tier over the matched subjects: the facts reached from a
//! named subject through the declared links, `links_depth` hops deep.
//!
//! At depth 1 a hit is a candidate one of whose `brain_fact_links` names
//! a matched subject and whose own subject is not that subject, scored
//! `score × links_weight`. At depth 2 the walk also crosses the bridges a
//! fact declares between its subject and the subjects its entity fields
//! name (`brain_subject_links`, maintained with the links): for each
//! bridged subject `B` of a matched subject the tier adds the facts about
//! `B` at `score × links_weight × links_decay` and the facts linked to
//! `B` (not about it) at `score × links_weight × links_decay ×
//! links_weight`. One SQL serves both depths: the depth-2 branches are
//! fenced by `links_depth >= 2`, every branch is admitted through the
//! working-set probe and the common predicate, a fact reached more than
//! once keeps its best score, and depth 1 returns exactly the rows the
//! one-hop query returned. No link is followed from a linked fact and no
//! bridge from a bridged subject, so the reach is bounded by the
//! declared depth, never by the graph.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use systemprompt_identifiers::BrainFactId;
use systemprompt_traits::RepositoryError;

use super::TierFilter;
use super::matched::MatchedSubjects;

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
           reach AS (
               SELECT l.fact_id AS id, m.score * $10 AS score,
                      m.subject_kind AS not_kind, m.subject_id AS not_id
               FROM matched m
               JOIN brain_fact_links l
                 ON l.subject_kind = m.subject_kind AND l.subject_id = m.subject_id
               UNION ALL
               SELECT a.id, m.score * $10 * $12::float8,
                      m.subject_kind, m.subject_id
               FROM matched m
               JOIN brain_subject_links b
                 ON b.subject_kind = m.subject_kind AND b.subject_id = m.subject_id
               JOIN brain_facts a
                 ON a.subject_kind = b.other_kind AND a.subject_id = b.other_id
               WHERE $11::int4 >= 2
               UNION ALL
               SELECT l.fact_id, m.score * $10 * $12::float8 * $10,
                      b.other_kind, b.other_id
               FROM matched m
               JOIN brain_subject_links b
                 ON b.subject_kind = m.subject_kind AND b.subject_id = m.subject_id
               JOIN brain_fact_links l
                 ON l.subject_kind = b.other_kind AND l.subject_id = b.other_id
               WHERE $11::int4 >= 2
           ),
           hits AS (
               SELECT r.id, max(r.score) AS score
               FROM reach r
               JOIN brain_facts f ON f.id = r.id
               WHERE NOT (f.subject_kind = r.not_kind AND f.subject_id = r.not_id)
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
               GROUP BY r.id
           )
           SELECT h.id AS "id!: BrainFactId"
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
        f.links_depth,
        f.links_decay,
    )
    .fetch_all(pool)
    .await?)
}
