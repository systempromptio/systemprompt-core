use chrono::Utc;
use systemprompt_brain::index::{AliasOwner, AliasRows};
use systemprompt_brain::recall::tiers::matched::resolve;
use systemprompt_brain::recall::tiers::{TierFilter, normalise_hints};
use systemprompt_brain::repository::{
    AliasRepository, SearchTerms, SearchTermsRepository, SubjectRepository,
};
use systemprompt_extension::brain::MemoryRecall;
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::{RetrievalDoc, ScopeLevel, SubjectKey, WithheldReason};

use crate::harness::{Harness, Setup};
use crate::plane;
use crate::recall_seed::{PROJECT, seed};
use crate::recall_support::{ids, injected, principal, request};

const PRODUCT: &str = "product";

fn filter(longest_wins: bool) -> TierFilter {
    let retrieval = RetrievalDoc::default();
    TierFilter {
        user: systemprompt_identifiers::UserId::new("u-names"),
        project: None,
        scopes: ScopeLevel::ALL
            .iter()
            .map(|s| s.as_str().to_owned())
            .collect(),
        keys: vec!["org".to_owned()],
        include_stale: false,
        as_of: Utc::now(),
        limit: 20,
        links_weight: retrieval.links_weight,
        alias_min_sim: retrieval.alias_min_sim,
        max_subjects: i64::from(retrieval.max_subjects),
        asked_kinds: Vec::new(),
        alias_longest_wins: longest_wins,
        links_depth: 1,
        links_decay: retrieval.links_decay,
    }
}

async fn alias(h: &Harness, subject: &SubjectKey, alias: &str) {
    let mut conn = h.pg.acquire().await.expect("conn");
    SubjectRepository::ensure(&mut conn, subject, None)
        .await
        .expect("subject");
    let fact = BrainFactId::generate();
    let rows = AliasRows::collect([(
        AliasOwner {
            subject,
            fact: &fact,
            updated_at: Utc::now(),
        },
        vec![alias.to_owned()],
    )]);
    AliasRepository::upsert_many(&mut conn, &rows)
        .await
        .expect("alias");
}

#[tokio::test]
async fn the_longest_exact_alias_wins_over_a_subject_it_contains() {
    let h = Harness::new("brain_names_longest").await;
    let client = SubjectKey::new(PRODUCT, "larchmont");
    let project = SubjectKey::new(PRODUCT, "larchmont-platform");
    alias(&h, &client, "Larchmont").await;
    alias(&h, &project, "Larchmont Platform").await;
    let hints = normalise_hints("Catch me up on Larchmont Platform");

    let both = resolve(&h.pg, &filter(false), &hints, &SearchTerms::default())
        .await
        .expect("matched");
    assert_eq!(both.ids, vec!["larchmont", "larchmont-platform"]);
    assert!(both.scores.iter().all(|s| (s - 1.0).abs() < 1e-9));

    let longest = resolve(&h.pg, &filter(true), &hints, &SearchTerms::default())
        .await
        .expect("matched");
    assert_eq!(longest.ids, vec!["larchmont-platform"]);

    let alone = resolve(
        &h.pg,
        &filter(true),
        &normalise_hints("Catch me up on Larchmont"),
        &SearchTerms::default(),
    )
    .await
    .expect("matched");
    assert_eq!(alone.ids, vec!["larchmont"], "nothing contains it");
    h.finish().await;
}

#[tokio::test]
async fn a_message_naming_a_ghost_withholds_the_full_text_hits_but_a_known_subject_does_not() {
    let h = Harness::with("brain_names_unknown", Setup::confirming()).await;
    let seeded = seed(&h).await;
    let mut declared = plane::declared();
    declared.retrieval = Some(RetrievalDoc {
        unknown_name_withholds_full_text: true,
        ..RetrievalDoc::default()
    });
    plane::apply(&h.pg, &declared).await;
    let mut conn = h.pg.acquire().await.expect("conn");
    SearchTermsRepository::refresh(&mut conn)
        .await
        .expect("term statistics");
    drop(conn);

    let ghost = h
        .recall()
        .recall(&request(
            principal(&seeded.reader, Some(PROJECT)),
            "Any news on Zorblax payments?",
        ))
        .await
        .expect("recall");
    let got = ids(&ghost);
    assert!(
        !got.contains(&seeded.full_text),
        "the payments fact was a full-text hit about a name no fact carries: {got:?}"
    );
    assert_eq!(
        injected(&ghost)
            .expect("the context and pinned facts still inject")
            .withheld
            .by_reason
            .get(&WithheldReason::UnknownName),
        Some(&1)
    );

    let known = h
        .recall()
        .recall(&request(
            principal(&seeded.reader, Some(PROJECT)),
            "Any news on Acme payments?",
        ))
        .await
        .expect("recall");
    let got = ids(&known);
    assert!(got.contains(&seeded.full_text), "{got:?}");
    assert!(got.contains(&seeded.subject), "{got:?}");
    assert_eq!(
        injected(&known)
            .expect("injected")
            .withheld
            .by_reason
            .get(&WithheldReason::UnknownName),
        None
    );
    h.finish().await;
}
