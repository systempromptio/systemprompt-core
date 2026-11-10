use std::sync::Arc;
use std::time::Duration;

use systemprompt_brain::recall::visible::{Verdict, Visibility, topic_widens};
use systemprompt_models::brain::{FactScope, FactStatus, Granularity, WithheldReason};
use systemprompt_models::notifications::AudienceSubject;

use crate::recall_fakes::{FakeAccess, FakeResolver};
use crate::recall_fixtures::{brain_fact, principal, scored, uid, with_scope, with_topic};
use crate::recall_visible::{READABLE, topic_decl};

fn visibility() -> Visibility {
    Visibility::new(Arc::new(FakeAccess(READABLE)), Duration::from_millis(20))
        .with_audiences(vec![Arc::new(FakeResolver {
            member: uid("alice"),
        })])
        .with_topics(vec![
            topic_decl(
                "open",
                vec![AudienceSubject::Everyone],
                Granularity::Individual,
            ),
            topic_decl(
                "leads",
                vec![AudienceSubject::Role("lead".to_owned())],
                Granularity::Individual,
            ),
        ])
}

fn confirmed_personal(topic: &str) -> systemprompt_models::brain::BrainFact {
    let mut fact = with_topic(
        with_scope(brain_fact(1, "a"), FactScope::User(uid("bob"))),
        topic,
    );
    fact.status = FactStatus::Confirmed;
    fact
}

#[tokio::test]
async fn a_user_fact_under_an_everyone_topic_stays_its_owners() {
    let fact = confirmed_personal("open");
    let v = visibility();
    assert_eq!(
        v.check(&fact, &principal("bob", None)).await,
        Verdict::Visible
    );
    assert_eq!(
        v.check(&fact, &principal("carol", None)).await,
        Verdict::Withheld(WithheldReason::Scope)
    );
}

#[tokio::test]
async fn a_user_fact_under_a_role_topic_is_hidden_from_role_members() {
    let fact = confirmed_personal("leads");
    let v = visibility();
    assert_eq!(
        v.check(&fact, &principal("alice", None)).await,
        Verdict::Withheld(WithheldReason::Scope)
    );
    let (visible, withheld) = v
        .filter(vec![scored(fact, 1.0)], &principal("alice", None))
        .await;
    assert!(visible.is_empty());
    assert_eq!(withheld.by_reason.get(&WithheldReason::Scope), Some(&1));
}

#[test]
fn only_a_user_scope_refuses_topic_widening() {
    assert!(!topic_widens(&FactScope::User(uid("bob"))));
    assert!(topic_widens(&FactScope::Org));
    assert!(topic_widens(&FactScope::Project("apollo".to_owned())));
    assert!(topic_widens(&FactScope::Group("platform".to_owned())));
}
