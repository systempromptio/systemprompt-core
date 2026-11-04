use std::sync::Arc;
use std::time::Duration;

use systemprompt_brain::recall::HeldKeys;
use systemprompt_brain::recall::visible::{Verdict, Visibility};
use systemprompt_extension::brain::SourceVerdict;
use systemprompt_identifiers::SessionId;
use systemprompt_models::brain::{FactScope, FactSource, FactStatus, Granularity, WithheldReason};
use systemprompt_models::notifications::AudienceSubject;

use crate::fixtures::document_source;
use crate::recall_fakes::{CountingAccess, EnumeratingResolver};
use crate::recall_fixtures::{brain_fact, principal, scored, uid, with_source, with_topic};
use crate::recall_visible::topic_decl;

fn session_fact(n: u32, owner: &str) -> systemprompt_models::brain::BrainFact {
    let mut fact = with_source(
        brain_fact(n, "a"),
        FactSource::Session {
            session_id: SessionId::new("s-1"),
            owner: uid(owner),
        },
    );
    fact.status = FactStatus::Proposed;
    fact
}

#[tokio::test]
async fn a_document_many_facts_cite_is_checked_once_and_every_fact_gets_its_own_verdict() {
    let access = Arc::new(CountingAccess::new(SourceVerdict::Readable));
    let visibility = Visibility::new(Arc::clone(&access) as _, Duration::from_millis(20));
    let ranked = vec![
        scored(
            with_source(brain_fact(1, "a"), document_source("doc-1", "r")),
            0.9,
        ),
        scored(
            with_source(brain_fact(2, "a"), document_source("doc-2", "r")),
            0.8,
        ),
        scored(
            with_source(brain_fact(3, "a"), document_source("doc-1", "r")),
            0.7,
        ),
        scored(session_fact(4, "carol"), 0.6),
        scored(
            with_source(brain_fact(5, "a"), document_source("doc-1", "r")),
            0.5,
        ),
    ];
    let expected: Vec<Verdict> = {
        let mut verdicts = Vec::new();
        for scored in &ranked {
            verdicts.push(
                visibility
                    .check(&scored.fact, &principal("bob", None))
                    .await,
            );
        }
        verdicts
    };
    let before = access.asked().len();
    let (visible, withheld) = visibility.filter(ranked, &principal("bob", None)).await;
    let mut asked = access.asked().split_off(before);
    asked.sort();
    assert_eq!(
        asked,
        vec!["doc-1", "doc-2"],
        "one check per distinct document"
    );
    let ids: Vec<u32> = visible
        .iter()
        .map(|s| s.fact.id.as_str()[2..10].parse().expect("n"))
        .collect();
    assert_eq!(ids, vec![1, 2, 3, 5]);
    assert_eq!(withheld.by_reason.get(&WithheldReason::Source), Some(&1));
    assert_eq!(
        expected,
        vec![
            Verdict::Visible,
            Verdict::Visible,
            Verdict::Visible,
            Verdict::Withheld(WithheldReason::Source),
            Verdict::Visible,
        ]
    );
}

#[tokio::test]
async fn a_hidden_document_withholds_every_fact_that_cites_it_as_source() {
    let access = Arc::new(CountingAccess::new(SourceVerdict::Hidden));
    let visibility = Visibility::new(Arc::clone(&access) as _, Duration::from_millis(20));
    let ranked = vec![
        scored(
            with_source(brain_fact(1, "a"), document_source("doc-1", "r")),
            0.9,
        ),
        scored(
            with_source(brain_fact(2, "a"), document_source("doc-1", "r")),
            0.8,
        ),
    ];
    let (visible, withheld) = visibility.filter(ranked, &principal("bob", None)).await;
    assert!(visible.is_empty());
    assert_eq!(withheld.by_reason.get(&WithheldReason::Source), Some(&2));
    assert_eq!(access.asked(), vec!["doc-1"]);
}

#[tokio::test]
async fn a_role_a_resolver_must_decide_is_asked_once_for_every_fact_under_it() {
    let resolver = Arc::new(EnumeratingResolver::new("roles", None));
    let visibility = Visibility::new(
        Arc::new(CountingAccess::new(SourceVerdict::Readable)) as _,
        Duration::from_millis(20),
    )
    .with_audiences(vec![Arc::clone(&resolver) as _])
    .with_topics(vec![topic_decl(
        "leads",
        vec![AudienceSubject::Role("lead".to_owned())],
        Granularity::Individual,
    )])
    .with_held(HeldKeys {
        keys: vec!["org".to_owned()],
        enumerated: Vec::new(),
    });
    let project = FactScope::Project("apollo".to_owned());
    let ranked: Vec<_> = vec![
        scored(with_topic(brain_fact(1, "a"), "leads"), 0.9),
        scored(with_topic(brain_fact(2, "a"), "leads"), 0.8),
        scored(with_topic(brain_fact(3, "a"), "leads"), 0.7),
    ]
    .into_iter()
    .map(|mut s| {
        s.fact.scope = project.clone();
        s
    })
    .collect();
    let (visible, _) = visibility.filter(ranked, &principal("bob", None)).await;
    assert_eq!(visible.len(), 3, "the resolver says member");
    assert_eq!(
        resolver.queries(),
        1,
        "one membership query for three facts"
    );
}
