//! P3 count equivalence: the index-driven lifecycle count answers exactly
//! what the candidate-join count answered, for both stale policies and
//! for an `as_of` in the past. The previous SQL is kept here verbatim.

use chrono::{Duration, Utc};
use systemprompt_brain::recall::tiers::{self, TierFilter};
use systemprompt_models::brain::{RetrievalDoc, ScopeLevel, WithheldReason};

use crate::harness::{Harness, Setup};
use crate::recall_seed::{PROJECT, Seeded, seed};

const PREVIOUS: &str = "WITH cand AS (
    SELECT DISTINCT w.fact_id FROM brain_working_set w
    WHERE w.scope_key = ANY($1::text[])
)
SELECT count(*) FILTER (WHERE f.status = 'stale' AND NOT $3),
       count(*) FILTER (WHERE f.status = 'disputed'),
       count(*) FILTER (
           WHERE (f.status IN ('confirmed', 'proposed') OR (f.status = 'stale' AND $3))
             AND f.review_after IS NOT NULL AND f.review_after < $4
       )
FROM cand c JOIN brain_facts f ON f.id = c.fact_id
WHERE f.scope_level = ANY($5::text[])
  AND (f.status <> 'proposed' OR f.owner_user_id = $2)
  AND (f.valid_from IS NULL OR f.valid_from <= $4)
  AND (f.valid_to IS NULL OR f.valid_to > $4)
  AND f.observed_at <= $4";

fn filter(seeded: &Seeded, include_stale: bool, as_of: chrono::DateTime<Utc>) -> TierFilter {
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
        as_of,
        limit: 0,
        links_weight: retrieval.links_weight,
        alias_min_sim: retrieval.alias_min_sim,
        max_subjects: i64::from(retrieval.max_subjects),
        asked_kinds: Vec::new(),
        alias_longest_wins: false,
        links_depth: 1,
        links_decay: retrieval.links_decay,
    }
}

async fn previous(h: &Harness, f: &TierFilter) -> (i64, i64, i64) {
    sqlx::query_as::<_, (i64, i64, i64)>(PREVIOUS)
        .bind(&f.keys)
        .bind(f.user.as_str())
        .bind(f.include_stale)
        .bind(f.as_of)
        .bind(&f.scopes)
        .fetch_one(&h.pg)
        .await
        .expect("previous count SQL")
}

fn count(counts: &std::collections::BTreeMap<WithheldReason, u32>, reason: WithheldReason) -> i64 {
    i64::from(counts.get(&reason).copied().unwrap_or(0))
}

#[tokio::test]
async fn the_lifecycle_count_answers_what_the_candidate_join_count_answered() {
    let h = Harness::with("brain_p3_withheld_equivalence", Setup::confirming()).await;
    let seeded = seed(&h).await;
    sqlx::query("UPDATE brain_facts SET review_after = now() - interval '1 day' WHERE id = $1")
        .bind(seeded.excluded[0].as_str())
        .execute(&h.pg)
        .await
        .expect("a stale fact also due for review");
    for include_stale in [false, true] {
        for as_of in [Utc::now(), Utc::now() - Duration::days(400)] {
            let f = filter(&seeded, include_stale, as_of);
            let got = tiers::withheld_counts(&h.pg, &f).await.expect("counts");
            let expected = previous(&h, &f).await;
            assert_eq!(
                (
                    count(&got, WithheldReason::Stale),
                    count(&got, WithheldReason::Disputed),
                    count(&got, WithheldReason::ReviewDue),
                ),
                expected,
                "include_stale={include_stale} as_of={as_of}"
            );
        }
    }
    let now = filter(&seeded, false, Utc::now());
    let got = tiers::withheld_counts(&h.pg, &now).await.expect("counts");
    assert_eq!(count(&got, WithheldReason::Stale), 1);
    assert_eq!(count(&got, WithheldReason::Disputed), 1);
    assert_eq!(
        count(&got, WithheldReason::ReviewDue),
        1,
        "duex; stalex is stale, not due"
    );
    let marker = filter(&seeded, true, Utc::now());
    let got = tiers::withheld_counts(&h.pg, &marker)
        .await
        .expect("counts");
    assert_eq!(count(&got, WithheldReason::Stale), 0);
    assert_eq!(
        count(&got, WithheldReason::ReviewDue),
        2,
        "under the marker policy the overdue stale fact is due"
    );
    h.finish().await;
}
