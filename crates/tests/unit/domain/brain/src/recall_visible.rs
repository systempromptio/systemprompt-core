use std::sync::Arc;
use std::time::Duration;

use systemprompt_brain::recall::visible::{Verdict, Visibility, lifecycle_withheld};
use systemprompt_extension::brain::SourceVerdict;
use systemprompt_identifiers::SessionId;
use systemprompt_models::brain::{
    FactScope, FactSource, FactStatus, Granularity, TopicDecl, WithheldReason,
};
use systemprompt_models::notifications::AudienceSubject;

use crate::fixtures::{at, subject, topic};
use crate::recall_fakes::{Access, FakeAccess, FakeResolver};
use crate::recall_fixtures::{
    brain_fact, principal, scored, uid, with_scope, with_source, with_topic,
};

fn topic_decl(id: &str, audience: Vec<AudienceSubject>, granularity: Granularity) -> TopicDecl {
    TopicDecl {
        id: topic(id),
        name: id.to_owned(),
        description: None,
        audience,
        granularity,
    }
}

fn visibility(access: Access) -> Visibility {
    Visibility::new(Arc::new(FakeAccess(access)), Duration::from_millis(20))
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
            topic_decl("people", vec![], Granularity::Aggregate),
        ])
}

const READABLE: Access = Access::Verdict(SourceVerdict::Readable);

async fn verdict(
    access: Access,
    fact: systemprompt_models::brain::BrainFact,
    who: &str,
    project: Option<&str>,
) -> Verdict {
    visibility(access)
        .check(&fact, &principal(who, project))
        .await
}

fn withheld(reason: WithheldReason) -> Verdict {
    Verdict::Withheld(reason)
}

#[tokio::test]
async fn scope_matrix() {
    let org = brain_fact(1, "a");
    assert_eq!(verdict(READABLE, org, "bob", None).await, Verdict::Visible);
    let mine = with_scope(brain_fact(2, "a"), FactScope::User(uid("bob")));
    assert_eq!(
        verdict(READABLE, mine.clone(), "bob", None).await,
        Verdict::Visible
    );
    assert_eq!(
        verdict(READABLE, mine, "carol", None).await,
        withheld(WithheldReason::Scope)
    );
    let project = with_scope(brain_fact(3, "a"), FactScope::Project("apollo".to_owned()));
    assert_eq!(
        verdict(READABLE, project.clone(), "bob", Some("apollo")).await,
        Verdict::Visible
    );
    assert_eq!(
        verdict(READABLE, project.clone(), "bob", Some("gemini")).await,
        withheld(WithheldReason::Scope)
    );
    assert_eq!(
        verdict(READABLE, project, "bob", None).await,
        withheld(WithheldReason::Scope)
    );
    let group = with_scope(brain_fact(4, "a"), FactScope::Group("platform".to_owned()));
    assert_eq!(
        verdict(READABLE, group.clone(), "alice", None).await,
        Verdict::Visible
    );
    assert_eq!(
        verdict(READABLE, group, "bob", None).await,
        withheld(WithheldReason::Scope)
    );
}

#[tokio::test]
async fn an_unresolvable_group_is_withheld() {
    let fact = with_scope(brain_fact(1, "a"), FactScope::Group("platform".to_owned()));
    let bare = Visibility::new(Arc::new(FakeAccess(READABLE)), Duration::from_millis(20));
    assert_eq!(
        bare.check(&fact, &principal("alice", None)).await,
        withheld(WithheldReason::Scope)
    );
}

#[tokio::test]
async fn topic_audiences_widen_scope() {
    let project = with_scope(brain_fact(1, "a"), FactScope::Project("apollo".to_owned()));
    let open = with_topic(project.clone(), "open");
    assert_eq!(
        verdict(READABLE, open, "bob", Some("gemini")).await,
        Verdict::Visible
    );
    let leads = with_topic(project.clone(), "leads");
    assert_eq!(
        verdict(READABLE, leads.clone(), "alice", None).await,
        Verdict::Visible
    );
    assert_eq!(
        verdict(READABLE, leads, "bob", None).await,
        withheld(WithheldReason::Scope)
    );
    let undeclared = with_topic(project, "unknown");
    assert_eq!(
        verdict(READABLE, undeclared, "bob", None).await,
        withheld(WithheldReason::Scope)
    );
}

#[tokio::test]
async fn a_topic_audience_never_bypasses_the_source_check() {
    let project = with_scope(brain_fact(1, "a"), FactScope::Project("apollo".to_owned()));
    let open = with_topic(project, "open");
    let hidden = Access::Verdict(SourceVerdict::Hidden);
    assert_eq!(
        verdict(hidden, open, "bob", None).await,
        withheld(WithheldReason::Source)
    );
}

#[tokio::test]
async fn source_verdicts_fail_closed() {
    let fact = brain_fact(1, "a");
    let hidden = Access::Verdict(SourceVerdict::Hidden);
    let capped = Access::Verdict(SourceVerdict::AgentCapped);
    assert_eq!(
        verdict(hidden, fact.clone(), "bob", None).await,
        withheld(WithheldReason::Source)
    );
    assert_eq!(
        verdict(capped, fact.clone(), "bob", None).await,
        withheld(WithheldReason::AgentCapped)
    );
    assert_eq!(
        verdict(Access::Fails, fact.clone(), "bob", None).await,
        withheld(WithheldReason::Source)
    );
    assert_eq!(
        verdict(Access::Hangs, fact, "bob", None).await,
        withheld(WithheldReason::Source)
    );
}

#[tokio::test]
async fn private_sources_stay_with_their_owner_until_confirmed() {
    let session = FactSource::Session {
        session_id: SessionId::new("s1"),
        owner: uid("bob"),
    };
    let mut fact = with_source(brain_fact(1, "a"), session);
    fact.status = FactStatus::Proposed;
    assert_eq!(
        verdict(Access::Fails, fact.clone(), "bob", None).await,
        Verdict::Visible
    );
    assert_eq!(
        verdict(Access::Fails, fact.clone(), "carol", None).await,
        withheld(WithheldReason::Source)
    );
    fact.status = FactStatus::Confirmed;
    assert_eq!(
        verdict(Access::Fails, fact, "carol", None).await,
        Verdict::Visible
    );
}

#[tokio::test]
async fn an_aggregate_topic_hides_a_person_from_everyone_else() {
    let mut fact = with_topic(brain_fact(1, "a"), "people");
    fact.subject = subject("user:bob");
    assert_eq!(
        verdict(READABLE, fact.clone(), "bob", None).await,
        Verdict::Visible
    );
    assert_eq!(
        verdict(READABLE, fact, "carol", None).await,
        withheld(WithheldReason::Scope)
    );
}

#[tokio::test]
async fn filter_counts_each_withheld_reason() {
    let ranked = vec![
        scored(brain_fact(1, "a"), 0.3),
        scored(
            with_scope(brain_fact(2, "a"), FactScope::User(uid("carol"))),
            0.2,
        ),
        scored(
            with_scope(brain_fact(3, "a"), FactScope::Project("x".to_owned())),
            0.1,
        ),
    ];
    let (visible, withheld) = visibility(READABLE)
        .filter(ranked, &principal("bob", None))
        .await;
    assert_eq!(visible.len(), 1);
    assert_eq!(withheld.by_reason.get(&WithheldReason::Scope), Some(&2));
    assert_eq!(withheld.total(), 2);
}

#[test]
fn lifecycle_withholds_stale_review_due_and_out_of_window_facts() {
    let now = at(1_800_000_000);
    let mut fact = brain_fact(1, "a");
    assert_eq!(lifecycle_withheld(&fact, now, false), None);
    fact.status = FactStatus::Stale;
    assert_eq!(
        lifecycle_withheld(&fact, now, false),
        Some(WithheldReason::Stale)
    );
    assert_eq!(lifecycle_withheld(&fact, now, true), None);
    fact.status = FactStatus::Confirmed;
    fact.review_after = Some(at(1_700_000_000));
    assert_eq!(
        lifecycle_withheld(&fact, now, false),
        Some(WithheldReason::ReviewDue)
    );
    fact.review_after = None;
    fact.valid_to = Some(at(1_750_000_000));
    assert_eq!(
        lifecycle_withheld(&fact, now, false),
        Some(WithheldReason::Scope)
    );
}
