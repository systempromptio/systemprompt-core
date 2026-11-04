//! P3 wave 3: the index-driven subject lookup (word n-grams on the btree,
//! trigram probes on the GIN) names the subjects the single alias scan
//! named, with the same scores, in the same order, over the recall seed
//! and aliases with spaces, a hyphen, punctuation at the message edges, a
//! possessive, three characters and a near miss, with and without term
//! statistics. The scan is kept here verbatim as the reference. The one
//! departure the design accepts, an alias that is only a fragment of a
//! longer word of the message, is pinned by its own test.

use chrono::Utc;
use systemprompt_brain::index::{AliasOwner, AliasRows};
use systemprompt_brain::recall::tiers::matched::resolve;
use systemprompt_brain::recall::tiers::{TierFilter, normalise_hints};
use systemprompt_brain::repository::{
    AliasRepository, SearchTerms, SearchTermsRepository, SubjectRepository,
};
use systemprompt_identifiers::{BrainFactId, UserId};
use systemprompt_models::brain::{RetrievalDoc, ScopeLevel, SubjectKey};

use crate::harness::{Harness, Setup};
use crate::recall_seed::{HINTS, PROJECT, seed};

const PREVIOUS: &str = "SELECT subject_kind, subject_id, max(score) AS score
    FROM (
        SELECT a.subject_kind, a.subject_id, a.alias_norm AS alias,
               CASE WHEN strpos($1, a.alias_norm) > 0 THEN 1.0::float8
                    ELSE word_similarity(a.alias_norm, $1)::float8 END AS score
        FROM brain_aliases a
        WHERE strpos($1, a.alias_norm) > 0 OR a.alias_norm <% $1
        UNION ALL
        SELECT s.subject_kind, s.subject_id, lower(s.subject_id) AS alias, 1.0::float8
        FROM brain_subjects s
        WHERE length(s.subject_id) >= 3 AND strpos($1, lower(s.subject_id)) > 0
    ) matched_hits GROUP BY subject_kind, subject_id
    HAVING max(score) >= $2
    ORDER BY max(score) DESC, subject_kind, subject_id
    LIMIT $3";

const PRODUCT: &str = "product";

const MESSAGE: &str = "Acme's payments: North Star Retail Group, the e-commerce hub, \
                       SAP and caloway; larchmont platfrom.";

type Named = (String, String, f64);

fn filter(reader: &UserId) -> TierFilter {
    let retrieval = RetrievalDoc::default();
    TierFilter {
        user: reader.clone(),
        project: Some(PROJECT.to_owned()),
        scopes: ScopeLevel::ALL
            .iter()
            .map(|s| s.as_str().to_owned())
            .collect(),
        keys: vec!["org".to_owned(), format!("project:{PROJECT}")],
        include_stale: false,
        as_of: Utc::now(),
        limit: 20,
        links_weight: retrieval.links_weight,
        alias_min_sim: retrieval.alias_min_sim,
        max_subjects: 20,
        asked_kinds: Vec::new(),
        alias_longest_wins: false,
        links_depth: 1,
        links_decay: retrieval.links_decay,
    }
}

async fn alias(h: &Harness, id: &str, alias: &str) {
    let subject = SubjectKey::new(PRODUCT, id);
    let mut conn = h.pg.acquire().await.expect("conn");
    SubjectRepository::ensure(&mut conn, &subject, None)
        .await
        .expect("subject");
    let fact = BrainFactId::generate();
    let rows = AliasRows::collect([(
        AliasOwner {
            subject: &subject,
            fact: &fact,
            updated_at: Utc::now(),
        },
        vec![alias.to_owned()],
    )]);
    AliasRepository::upsert_many(&mut conn, &rows)
        .await
        .expect("alias");
}

async fn previous(h: &Harness, f: &TierFilter, hints: &str) -> Vec<Named> {
    sqlx::query_as::<_, Named>(sqlx::AssertSqlSafe(PREVIOUS.to_owned()))
        .bind(hints)
        .bind(f.alias_min_sim)
        .bind(f.max_subjects)
        .fetch_all(&h.pg)
        .await
        .expect("previous matched SQL")
}

async fn current(h: &Harness, f: &TierFilter, hints: &str, terms: &SearchTerms) -> Vec<Named> {
    let matched = resolve(&h.pg, f, hints, terms).await.expect("matched");
    matched
        .kinds
        .into_iter()
        .zip(matched.ids)
        .zip(matched.scores)
        .map(|((kind, id), score)| (kind, id, score))
        .collect()
}

async fn statistics(h: &Harness) -> SearchTerms {
    let mut conn = h.pg.acquire().await.expect("conn");
    SearchTermsRepository::refresh(&mut conn)
        .await
        .expect("term statistics");
    drop(conn);
    SearchTermsRepository::load(&h.pg).await.expect("terms")
}

fn ids(named: &[Named]) -> Vec<&str> {
    named.iter().map(|(_, id, _)| id.as_str()).collect()
}

#[tokio::test]
async fn the_lookup_names_what_the_scan_named_with_the_same_scores_and_order() {
    let h = Harness::with("brain_matched_identity", Setup::confirming()).await;
    let seeded = seed(&h).await;
    alias(&h, "north-star", "North Star Retail Group").await;
    alias(&h, "e-commerce-hub", "e-commerce hub").await;
    alias(&h, "sap", "SAP").await;
    alias(&h, "calloway", "Calloway").await;
    alias(&h, "larchmont-platform", "Larchmont Platform").await;
    let f = filter(&seeded.reader);
    let loaded = statistics(&h).await;
    assert!(!loaded.is_empty(), "the seed facts carry words");

    for message in [
        MESSAGE,
        HINTS,
        "Catch me up on larchmont platform",
        "nothing named",
    ] {
        let hints = normalise_hints(message);
        let scan = previous(&h, &f, &hints).await;
        for terms in [&SearchTerms::default(), &loaded] {
            let lookup = current(&h, &f, &hints, terms).await;
            assert_eq!(lookup, scan, "{message:?}");
        }
    }

    let named = previous(&h, &f, &normalise_hints(MESSAGE)).await;
    assert_eq!(
        ids(&named),
        [
            "acme",
            "e-commerce-hub",
            "north-star",
            "sap",
            "larchmont-platform",
            "calloway"
        ],
        "four exact names at 1.0, then the two near misses: {named:?}"
    );
    assert!(named[..4].iter().all(|(_, _, s)| (s - 1.0).abs() < 1e-9));
    assert!(named[4..].iter().all(|(_, _, s)| (0.6..1.0).contains(s)));
    h.finish().await;
}

#[tokio::test]
async fn an_alias_inside_a_longer_word_is_no_longer_an_occurrence() {
    let h = Harness::new("brain_matched_fragment").await;
    alias(&h, "pay", "Pay").await;
    let reader = h.user("reader", &[]).await;
    let f = filter(&reader);
    let hints = normalise_hints("How is the payments run going?");
    let scan = previous(&h, &f, &hints).await;
    assert_eq!(ids(&scan), ["pay"], "the scan saw `pay` inside `payments`");
    let lookup = current(&h, &f, &hints, &SearchTerms::default()).await;
    assert!(lookup.is_empty(), "{lookup:?}");
    let whole = current(
        &h,
        &f,
        &normalise_hints("Pay is due"),
        &SearchTerms::default(),
    )
    .await;
    assert_eq!(ids(&whole), ["pay"], "the word itself still names it");
    h.finish().await;
}
