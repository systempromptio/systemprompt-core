//! P3 bit-identity: every tier's index-driven SQL returns the ids the
//! previous candidate-join SQL returned, in the same order, over a seeded
//! set that exercises every tier and every exclusion. The previous SQL is
//! kept here verbatim as the reference.

use chrono::Utc;
use systemprompt_brain::recall::search_plan::{SearchPolicy, plan};
use systemprompt_brain::recall::tiers::{self, FullTextInput, TierFilter, matched};
use systemprompt_brain::repository::SearchTermsRepository;
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::{RetrievalDoc, ScopeLevel, SynonymsDoc};

use crate::harness::{Harness, Setup};
use crate::recall_seed::{HINTS, PROJECT, Seeded, seed};

const CAND: &str = "WITH cand AS (
    SELECT DISTINCT w.fact_id FROM brain_working_set w
    WHERE w.scope_key = ANY($1::text[])
)";

const ADMITTED: &str = "AND f.scope_level = ANY($6::text[])
    AND (f.status = 'confirmed'
         OR (f.status = 'proposed' AND f.owner_user_id = $2)
         OR (f.status = 'stale' AND $3))
    AND (f.review_after IS NULL OR f.review_after >= $4)
    AND (f.valid_from IS NULL OR f.valid_from <= $4)
    AND (f.valid_to IS NULL OR f.valid_to > $4)
    AND f.observed_at <= $4";

const MATCHED: &str = "matched AS (
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
)";

fn filter(seeded: &Seeded, include_stale: bool) -> TierFilter {
    let retrieval = RetrievalDoc::default();
    TierFilter {
        user: seeded.reader.clone(),
        project: Some(PROJECT.to_owned()),
        scopes: ScopeLevel::ALL
            .iter()
            .map(|s| s.as_str().to_owned())
            .collect(),
        keys: vec![
            "org".to_owned(),
            format!("project:{PROJECT}"),
            format!("user:{}", seeded.reader),
        ],
        include_stale,
        as_of: Utc::now(),
        limit: 20,
        links_weight: retrieval.links_weight,
        alias_min_sim: retrieval.alias_min_sim,
        max_subjects: i64::from(retrieval.max_subjects),
    }
}

async fn previous(h: &Harness, sql: &str, f: &TierFilter, extra: &[&str]) -> Vec<String> {
    let mut query = sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(sql.to_owned()))
        .bind(&f.keys)
        .bind(f.user.as_str())
        .bind(f.include_stale)
        .bind(f.as_of)
        .bind(f.limit)
        .bind(&f.scopes);
    for value in extra {
        query = query.bind(*value);
    }
    query.fetch_all(&h.pg).await.expect("previous tier SQL")
}

fn strings(ids: &[BrainFactId]) -> Vec<String> {
    ids.iter().map(|id| id.as_str().to_owned()).collect()
}

#[tokio::test]
async fn every_tier_answers_exactly_what_the_candidate_join_sql_answered() {
    let h = Harness::with("brain_p3_bit_identity", Setup::confirming()).await;
    let seeded = seed(&h).await;
    for include_stale in [false, true] {
        let f = filter(&seeded, include_stale);
        let hints = tiers::normalise_hints(HINTS);

        let pinned_sql = format!(
            "{CAND} SELECT f.id FROM cand c JOIN brain_facts f ON f.id = c.fact_id
             WHERE f.pinned {ADMITTED} ORDER BY f.importance_base DESC, f.id LIMIT $5"
        );
        let pinned = tiers::pinned(&h.pg, &f).await.expect("pinned");
        assert_eq!(strings(&pinned), previous(&h, &pinned_sql, &f, &[]).await);
        assert_eq!(strings(&pinned), vec![seeded.pinned.as_str()]);

        let context_sql = "WITH cand AS (SELECT DISTINCT w.fact_id FROM brain_working_set w
                           WHERE w.scope_key = $7 AND w.scope_key = ANY($1::text[]))
             SELECT f.id FROM cand c JOIN brain_facts f ON f.id = c.fact_id
             WHERE f.scope_level = 'project' AND 'project' = ANY($6::text[])
               AND (f.status = 'confirmed'
                    OR (f.status = 'proposed' AND f.owner_user_id = $2)
                    OR (f.status = 'stale' AND $3))
               AND (f.review_after IS NULL OR f.review_after >= $4)
               AND (f.valid_from IS NULL OR f.valid_from <= $4)
               AND (f.valid_to IS NULL OR f.valid_to > $4)
               AND f.observed_at <= $4
             ORDER BY f.importance_base DESC, f.id LIMIT $5";
        let key = format!("project:{PROJECT}");
        let context = tiers::by_context(&h.pg, &f).await.expect("context");
        assert_eq!(
            strings(&context),
            previous(&h, &context_sql, &f, &[key.as_str()]).await
        );
        assert_eq!(strings(&context), vec![seeded.context.as_str()]);

        let matched = matched::resolve(&h.pg, &f, &hints).await.expect("matched");
        assert!(!matched.is_empty(), "the hints name acme");
        let subject_sql = format!(
            "{CAND}, {MATCHED},
             hits AS (SELECT f.id, m.score FROM cand c
                      JOIN brain_facts f ON f.id = c.fact_id
                      JOIN matched m ON m.subject_kind = f.subject_kind
                                    AND m.subject_id = f.subject_id),
             ranked AS (SELECT id, max(score) AS score FROM hits GROUP BY id)
             SELECT f.id FROM ranked r JOIN brain_facts f ON f.id = r.id
             WHERE true {ADMITTED}
             ORDER BY r.score DESC, f.importance_base DESC, f.id LIMIT $5"
        );
        let subject = tiers::by_subject(&h.pg, &f, &matched)
            .await
            .expect("subject");
        let previous_subject = sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(subject_sql))
            .bind(&f.keys)
            .bind(f.user.as_str())
            .bind(f.include_stale)
            .bind(f.as_of)
            .bind(f.limit)
            .bind(&f.scopes)
            .bind(&hints)
            .bind(f.alias_min_sim)
            .bind(f.max_subjects)
            .fetch_all(&h.pg)
            .await
            .expect("previous subject SQL");
        assert_eq!(strings(&subject), previous_subject);
        assert!(strings(&subject).contains(&seeded.subject.as_str().to_owned()));

        let retrieval = RetrievalDoc::default();
        let terms = SearchTermsRepository::load(&h.pg).await.expect("terms");
        let policy = SearchPolicy::new(retrieval.search_max_df, retrieval.search_quorum);
        let synonyms = SynonymsDoc::default();
        let Some(searched) = plan(HINTS, &synonyms, &terms, &policy) else {
            continue;
        };
        let text_sql = format!(
            "{CAND} SELECT f.id FROM cand c JOIN brain_facts f ON f.id = c.fact_id
             WHERE f.search_weighted @@ to_tsquery('simple', $7) {ADMITTED}
             ORDER BY ts_rank_cd(f.search_weighted, to_tsquery('simple', $8), 32) DESC,
                      f.importance_base DESC, f.id LIMIT $5"
        );
        let text = tiers::full_text(
            &h.pg,
            &f,
            FullTextInput {
                hints: HINTS,
                synonyms: &synonyms,
                terms: &terms,
                policy: &policy,
            },
        )
        .await
        .expect("full text");
        assert_eq!(
            strings(&text),
            previous(
                &h,
                &text_sql,
                &f,
                &[searched.filter.as_str(), searched.rank.as_str()]
            )
            .await
        );
        assert!(strings(&text).contains(&seeded.full_text.as_str().to_owned()));
        for excluded in &seeded.excluded[1..] {
            let id = excluded.as_str().to_owned();
            for (tier, ids) in [("pinned", &pinned), ("subject", &subject), ("text", &text)] {
                assert!(!strings(ids).contains(&id), "{tier} returned {id}");
            }
        }
        let stale = seeded.excluded[0].as_str().to_owned();
        if !include_stale {
            assert!(!strings(&subject).contains(&stale));
            assert!(!strings(&text).contains(&stale));
        }
    }
    h.finish().await;
}
