//! The linked tier's declared depth. A delivery concern on a project that
//! names no client is one subject hop beyond the client: at `links_depth`
//! 1 "Acme's risks" never reaches it, at 2 it is reached through the
//! engagement fact's client <-> project bridge, scored a decay below the
//! facts that name the client directly and ordered after them. Depth 1
//! answers exactly what the one-hop SQL answered over a corpus that has
//! links. With `links_require_readable_anchor` a client whose every fact
//! the reader cannot read (here: observed after `as_of`) bridges nothing.

use chrono::Utc;
use systemprompt_brain::recall::BrainRecallStore;
use systemprompt_brain::recall::tiers::matched::MatchedSubjects;
use systemprompt_brain::recall::tiers::{self, TierFilter};
use systemprompt_extension::brain::FactRecallSource;
use systemprompt_identifiers::{BrainFactId, UserId};
use systemprompt_models::auth::RequestActor;
use systemprompt_models::brain::{RecallCtx, RecallQuery, RetrievalDoc, ScopeLevel};

use crate::harness::{Harness, Setup};
use crate::plane;
use crate::recall_support::{ids, recall};
use crate::subject_links::{
    CLIENT, CLIENT_NOTE, ENGAGEMENT, PROJECT, RISK, declared_with_bridges, fact_of_kind,
    seed_bridges,
};

/// The one-hop linked SQL before `links_depth`, verbatim.
pub(crate) const LINKED_PREVIOUS: &str = "WITH matched AS (
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
SELECT h.id FROM hits h JOIN brain_facts f ON f.id = h.id
ORDER BY h.score DESC, f.importance_base DESC, f.id
LIMIT $5";

pub(crate) async fn linked_previous(
    h: &Harness,
    f: &TierFilter,
    matched: &MatchedSubjects,
) -> Vec<String> {
    sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(LINKED_PREVIOUS.to_owned()))
        .bind(&f.keys)
        .bind(f.user.as_str())
        .bind(f.include_stale)
        .bind(f.as_of)
        .bind(f.limit)
        .bind(&f.scopes)
        .bind(&matched.kinds)
        .bind(&matched.ids)
        .bind(&matched.scores)
        .bind(f.links_weight)
        .fetch_all(&h.pg)
        .await
        .expect("previous linked SQL")
}

fn strings(ids: &[BrainFactId]) -> Vec<String> {
    ids.iter().map(|id| id.as_str().to_owned()).collect()
}

fn filter(reader: &UserId, depth: i32) -> TierFilter {
    let retrieval = RetrievalDoc::default();
    TierFilter {
        user: reader.clone(),
        project: None,
        scopes: ScopeLevel::ALL
            .iter()
            .map(|s| s.as_str().to_owned())
            .collect(),
        keys: vec!["org".to_owned(), format!("user:{reader}")],
        include_stale: false,
        as_of: Utc::now(),
        limit: 20,
        links_weight: retrieval.links_weight,
        alias_min_sim: retrieval.alias_min_sim,
        max_subjects: i64::from(retrieval.max_subjects),
        asked_kinds: Vec::new(),
        alias_longest_wins: false,
        links_depth: depth,
        links_decay: retrieval.links_decay,
        links_require_readable_anchor: false,
    }
}

fn acme() -> MatchedSubjects {
    MatchedSubjects {
        kinds: vec!["client".to_owned()],
        ids: vec![CLIENT.to_owned()],
        scores: vec![1.0],
    }
}

#[tokio::test]
async fn depth_two_reaches_the_bridged_projects_facts_after_the_one_hop_hits() {
    let h = Harness::with("brain_recall_depth", Setup::confirming()).await;
    plane::apply(&h.pg, &declared_with_bridges()).await;
    let admin = h.user("admin", &["admin"]).await;
    seed_bridges(&h, &admin).await;
    let engagement = fact_of_kind(&h, PROJECT, ENGAGEMENT).await;
    let risk = fact_of_kind(&h, PROJECT, RISK).await;
    let note = fact_of_kind(&h, CLIENT, CLIENT_NOTE).await;

    let one = filter(&admin, 1);
    let depth_one = tiers::by_linked(&h.pg, &one, &acme())
        .await
        .expect("linked at depth 1");
    assert_eq!(
        depth_one,
        vec![engagement.clone()],
        "one hop reaches the fact that names the client and nothing about the project"
    );
    assert_eq!(
        strings(&depth_one),
        linked_previous(&h, &one, &acme()).await,
        "depth 1 is the one-hop SQL, row for row"
    );

    let two = filter(&admin, 2);
    let depth_two = tiers::by_linked(&h.pg, &two, &acme())
        .await
        .expect("linked at depth 2");
    assert_eq!(
        depth_two,
        vec![engagement, risk],
        "the bridge adds the project's risk at links_weight × links_decay, after the one-hop hit"
    );
    assert!(
        !depth_two.contains(&note),
        "a fact about the matched subject itself is the subject tier's, never the linked tier's"
    );
    h.finish().await;
}

async fn apply_depth(h: &Harness, depth: u32) {
    let mut declared = declared_with_bridges();
    declared.retrieval = Some(RetrievalDoc {
        links_depth: depth,
        ..RetrievalDoc::default()
    });
    plane::apply(&h.pg, &declared).await;
}

#[tokio::test]
async fn at_depth_one_the_clients_question_never_reaches_the_projects_risk() {
    let h = Harness::with("brain_recall_depth_one", Setup::confirming()).await;
    apply_depth(&h, 1).await;
    let admin = h.user("admin", &["admin"]).await;
    seed_bridges(&h, &admin).await;
    let risk = fact_of_kind(&h, PROJECT, RISK).await;
    let engagement = fact_of_kind(&h, PROJECT, ENGAGEMENT).await;

    let store = BrainRecallStore::new((*h.repos).clone(), h.profile.recall.stale);
    let ctx = RecallCtx::new(admin.clone(), RequestActor::Human, Utc::now())
        .with_held_keys(vec!["org".to_owned(), format!("user:{admin}")]);
    let query = RecallQuery::new("Acme's risks".to_owned(), 20);
    let subject = store.by_subject(&query, &ctx).await.expect("subject tier");
    assert!(
        subject.iter().all(|f| f.subject.id == CLIENT),
        "the message names the client and only the client: {subject:?}"
    );
    let linked = store.by_linked(&query, &ctx).await.expect("linked tier");
    assert_eq!(
        linked.iter().map(|f| f.id.clone()).collect::<Vec<_>>(),
        vec![engagement]
    );

    let got = ids(&recall(&h, &admin, "Acme's risks").await);
    assert!(!got.contains(&risk), "{got:?}");
    h.finish().await;
}

#[tokio::test]
async fn at_depth_two_the_clients_question_reaches_the_projects_risk() {
    let h = Harness::with("brain_recall_depth_two", Setup::confirming()).await;
    apply_depth(&h, 2).await;
    let admin = h.user("admin", &["admin"]).await;
    seed_bridges(&h, &admin).await;
    let risk = fact_of_kind(&h, PROJECT, RISK).await;
    let engagement = fact_of_kind(&h, PROJECT, ENGAGEMENT).await;

    let store = BrainRecallStore::new((*h.repos).clone(), h.profile.recall.stale);
    let ctx = RecallCtx::new(admin.clone(), RequestActor::Human, Utc::now())
        .with_held_keys(vec!["org".to_owned(), format!("user:{admin}")]);
    let query = RecallQuery {
        links_depth: 2,
        ..RecallQuery::new("Acme's risks".to_owned(), 20)
    };
    let linked = store.by_linked(&query, &ctx).await.expect("linked tier");
    assert_eq!(
        linked.iter().map(|f| f.id.clone()).collect::<Vec<_>>(),
        vec![engagement, risk.clone()],
        "the one-hop hit first, the bridged project's risk after it"
    );

    let got = ids(&recall(&h, &admin, "Acme's risks").await);
    assert!(got.contains(&risk), "{got:?}");
    h.finish().await;
}

/// Re-observes the client note in the future, so the anchor has no fact
/// readable at `as_of = now` while the project's facts stay readable.
async fn hide_the_client(h: &Harness) {
    let note = fact_of_kind(h, CLIENT, CLIENT_NOTE).await;
    sqlx::query("UPDATE brain_facts SET observed_at = now() + interval '1 day' WHERE id = $1")
        .bind(note.as_str())
        .execute(&h.pg)
        .await
        .expect("future-date the client note");
}

#[tokio::test]
async fn a_hidden_anchor_bridges_nothing_when_declared_so() {
    let h = Harness::with("brain_recall_depth_hidden_anchor", Setup::confirming()).await;
    apply_depth(&h, 2).await;
    let admin = h.user("admin", &["admin"]).await;
    seed_bridges(&h, &admin).await;
    hide_the_client(&h).await;
    let engagement = fact_of_kind(&h, PROJECT, ENGAGEMENT).await;
    let risk = fact_of_kind(&h, PROJECT, RISK).await;

    let store = BrainRecallStore::new((*h.repos).clone(), h.profile.recall.stale);
    let ctx = RecallCtx::new(admin.clone(), RequestActor::Human, Utc::now())
        .with_held_keys(vec!["org".to_owned(), format!("user:{admin}")]);
    let walked = RecallQuery {
        links_depth: 2,
        ..RecallQuery::new("Acme's risks".to_owned(), 20)
    };
    let linked = store.by_linked(&walked, &ctx).await.expect("linked tier");
    assert_eq!(
        linked.iter().map(|f| f.id.clone()).collect::<Vec<_>>(),
        vec![engagement, risk],
        "without the rule the hidden client still bridges to its project"
    );
    let guarded = RecallQuery {
        links_require_readable_anchor: true,
        ..walked
    };
    let linked = store.by_linked(&guarded, &ctx).await.expect("linked tier");
    assert!(
        linked.is_empty(),
        "a hidden anchor bridges nothing: {linked:?}"
    );
    h.finish().await;
}
