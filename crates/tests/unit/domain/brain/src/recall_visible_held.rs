use std::sync::Arc;
use std::time::Duration;

use systemprompt_brain::recall::HeldKeys;
use systemprompt_brain::recall::visible::{Verdict, Visibility};
use systemprompt_models::brain::{FactScope, Granularity, WithheldReason};
use systemprompt_models::notifications::AudienceSubject;

use crate::recall_fakes::{EnumeratingResolver, FakeAccess};
use crate::recall_fixtures::{brain_fact, principal, with_scope, with_topic};
use crate::recall_visible::{READABLE, topic_decl};

fn withheld(reason: WithheldReason) -> Verdict {
    Verdict::Withheld(reason)
}

fn role_topic_fact() -> systemprompt_models::brain::BrainFact {
    with_topic(
        with_scope(brain_fact(1, "a"), FactScope::Project("apollo".to_owned())),
        "leads",
    )
}

#[tokio::test]
async fn a_held_role_key_admits_a_topic_fact_without_a_membership_query() {
    let resolver = Arc::new(EnumeratingResolver::new("roles", Some(Vec::new())));
    let held = HeldKeys {
        keys: vec!["org".to_owned(), "role:lead".to_owned()],
        enumerated: vec!["roles"],
    };
    let visibility = Visibility::new(Arc::new(FakeAccess(READABLE)), Duration::from_millis(20))
        .with_audiences(vec![Arc::clone(&resolver) as _])
        .with_topics(vec![topic_decl(
            "leads",
            vec![AudienceSubject::Role("lead".to_owned())],
            Granularity::Individual,
        )])
        .with_held(held);
    assert_eq!(
        visibility
            .check(&role_topic_fact(), &principal("bob", None))
            .await,
        Verdict::Visible
    );
    assert_eq!(resolver.queries(), 0, "answered from the held keys");
}

#[tokio::test]
async fn a_role_an_enumerating_resolver_did_not_list_is_not_held_and_not_queried() {
    let resolver = Arc::new(EnumeratingResolver::new("roles", Some(Vec::new())));
    let held = HeldKeys {
        keys: vec!["org".to_owned()],
        enumerated: vec!["roles"],
    };
    let visibility = Visibility::new(Arc::new(FakeAccess(READABLE)), Duration::from_millis(20))
        .with_audiences(vec![Arc::clone(&resolver) as _])
        .with_topics(vec![topic_decl(
            "leads",
            vec![AudienceSubject::Role("lead".to_owned())],
            Granularity::Individual,
        )])
        .with_held(held);
    assert_eq!(
        visibility
            .check(&role_topic_fact(), &principal("bob", None))
            .await,
        withheld(WithheldReason::Scope)
    );
    assert_eq!(resolver.queries(), 0);
}

#[tokio::test]
async fn a_resolver_that_cannot_enumerate_is_still_asked_per_fact() {
    let resolver = Arc::new(EnumeratingResolver::new("roles", None));
    let held = HeldKeys {
        keys: vec!["org".to_owned()],
        enumerated: Vec::new(),
    };
    let visibility = Visibility::new(Arc::new(FakeAccess(READABLE)), Duration::from_millis(20))
        .with_audiences(vec![Arc::clone(&resolver) as _])
        .with_topics(vec![topic_decl(
            "leads",
            vec![AudienceSubject::Role("lead".to_owned())],
            Granularity::Individual,
        )])
        .with_held(held);
    assert_eq!(
        visibility
            .check(&role_topic_fact(), &principal("bob", None))
            .await,
        Verdict::Visible,
        "defence in depth: the per-fact check still decides"
    );
    assert_eq!(resolver.queries(), 1);
}
