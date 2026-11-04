//! Recall over the working set: candidates are the facts indexed under
//! the keys the principal holds (a role audience from the user's roles, a
//! group from the resolver's enumeration), and a subject reached through
//! one link hop scores below a direct hit.

use std::sync::Arc;

use chrono::Utc;
use systemprompt_brain::plane::DeclaredBrain;
use systemprompt_brain::recall::BrainRecallStore;
use systemprompt_extension::brain::{FactRecallSource, MemoryRecall, RecallOutcome, SkipReason};
use systemprompt_extension::notifications::{
    Membership, NotificationAudienceError, NotificationAudienceResolver,
};
use systemprompt_identifiers::{BrainFactId, UserId};
use systemprompt_models::auth::RequestActor;
use systemprompt_models::brain::{
    EnteredFact, EnteredStatus, FactScope, Granularity, RecallCtx, RecallQuery, SubjectKey,
    TopicDecl,
};
use systemprompt_models::notifications::AudienceSubject;
use systemprompt_runtime::notifications::UserRoleAudienceResolver;
use systemprompt_users::UserRepository;

use crate::events::{PRODUCT, entered};
use crate::harness::{Harness, Setup};
use crate::plane::{self, ENGINEERING, FINANCE_LEAD, OWNERSHIP, kind, topic};
use crate::recall_seed::{PROJECT, refresh_working_set, seed};
use crate::recall_support::{ids, principal, request};

struct GroupResolver {
    group: &'static str,
}

#[async_trait::async_trait]
impl NotificationAudienceResolver for GroupResolver {
    fn name(&self) -> &'static str {
        "test-groups"
    }

    fn handles(&self, subject: &AudienceSubject) -> bool {
        matches!(subject, AudienceSubject::Dimension { kind, .. } if kind == "group")
    }

    async fn members(
        &self,
        _subject: &AudienceSubject,
        _limit: usize,
    ) -> Result<Vec<UserId>, NotificationAudienceError> {
        Ok(Vec::new())
    }

    async fn is_member(
        &self,
        _subject: &AudienceSubject,
        _user: &UserId,
    ) -> Result<Membership, NotificationAudienceError> {
        panic!("the group is answered from the held keys, never per fact");
    }

    async fn held(&self, _user: &UserId) -> Option<Vec<AudienceSubject>> {
        Some(vec![AudienceSubject::Dimension {
            kind: "group".to_owned(),
            value: self.group.to_owned(),
        }])
    }
}

/// The declared plane with the ownership kind's topic narrowed to a role
/// nobody holds: its default `everyone` audience widens every ownership
/// fact to the org, which would make scope keys moot.
fn scoped_plane() -> DeclaredBrain {
    let mut declared = plane::declared();
    let engineering = declared
        .topics
        .iter_mut()
        .find(|t| t.id.as_str() == ENGINEERING)
        .expect("engineering topic");
    engineering.audience = vec![AudienceSubject::Role("engineering_lead".to_owned())];
    declared
}

async fn owned(h: &Harness, who: &UserId, product: &str, scope: FactScope) {
    h.emit(&entered(
        who,
        kind(OWNERSHIP),
        product,
        serde_json::json!({ "team": "ops" }),
        scope,
    ))
    .await;
}

#[tokio::test]
async fn candidates_are_the_facts_under_the_keys_the_principal_holds() {
    let h = Harness::with("brain_ws_candidates", Setup::confirming()).await;
    plane::apply(&h.pg, &scoped_plane()).await;
    let reader = h.user("reader", &[]).await;
    let other = h.user("other", &[]).await;
    owned(&h, &reader, "nimbus", FactScope::Org).await;
    owned(&h, &other, "cumulus", FactScope::User(other.clone())).await;
    owned(
        &h,
        &reader,
        "stratus",
        FactScope::Project("elsewhere".to_owned()),
    )
    .await;
    owned(
        &h,
        &reader,
        "cirrus",
        FactScope::Group("platform".to_owned()),
    )
    .await;
    h.drain().await;
    refresh_working_set(&h).await;
    let hints = "nimbus cumulus stratus cirrus";

    let got = ids(&h
        .recall()
        .recall(&request(principal(&reader, Some(PROJECT)), hints))
        .await
        .expect("recall"));
    assert_eq!(got, h.fact_ids("nimbus").await, "{got:?}");

    let store = BrainRecallStore::new((*h.repos).clone(), h.profile.recall.stale);
    let ctx = RecallCtx::new(reader.clone(), RequestActor::Human, Utc::now())
        .with_held_keys(vec!["org".to_owned(), format!("user:{other}")]);
    let text = store
        .full_text(&RecallQuery::new(hints.to_owned(), 20), &ctx)
        .await
        .expect("full text");
    let text_ids: Vec<BrainFactId> = text.iter().map(|f| f.id.clone()).collect();
    assert_eq!(
        text_ids.len(),
        2,
        "org plus the held user key: {text_ids:?}"
    );
    assert!(text_ids.contains(&h.fact_ids("cumulus").await[0]));

    let with_group = h
        .recall_with(vec![Arc::new(GroupResolver { group: "platform" }) as _])
        .recall(&request(principal(&reader, Some(PROJECT)), hints))
        .await
        .expect("recall with group");
    let got = ids(&with_group);
    assert!(got.contains(&h.fact_ids("cirrus").await[0]), "{got:?}");
    assert!(got.contains(&h.fact_ids("nimbus").await[0]), "{got:?}");
    assert_eq!(got.len(), 2);
    h.finish().await;
}

#[tokio::test]
async fn a_role_audience_is_answered_from_the_users_roles_once_per_recall() {
    let h = Harness::with("brain_ws_roles", Setup::confirming()).await;
    let mut declared = scoped_plane();
    declared.topics.push(TopicDecl {
        id: topic("leads"),
        name: "leads".to_owned(),
        description: None,
        audience: vec![AudienceSubject::Role(FINANCE_LEAD.to_owned())],
        granularity: Granularity::Individual,
    });
    plane::apply(&h.pg, &declared).await;
    let lead = h.user("lead", &[FINANCE_LEAD]).await;
    let intern = h.user("intern", &[]).await;
    let fact = EnteredFact {
        kind: kind(OWNERSHIP),
        subject: SubjectKey::new(PRODUCT, "argon"),
        fields: serde_json::json!({ "team": "finance" })
            .as_object()
            .cloned()
            .expect("object"),
        scope: FactScope::Project("elsewhere".to_owned()),
        topics: vec![topic("leads")],
        status: EnteredStatus::Proposed,
        observed_at: None,
    };
    h.emit(&fact.to_event(lead.clone(), Utc::now()).expect("event"))
        .await;
    h.drain().await;
    refresh_working_set(&h).await;
    let resolver: Arc<dyn NotificationAudienceResolver> = Arc::new(UserRoleAudienceResolver::new(
        Arc::new(UserRepository::new(&h.pool)),
    ));

    let seen = h
        .recall_with(vec![Arc::clone(&resolver)])
        .recall(&request(principal(&lead, None), "argon"))
        .await
        .expect("lead recall");
    assert_eq!(ids(&seen), h.fact_ids("argon").await);
    let unseen = h
        .recall_with(vec![resolver])
        .recall(&request(principal(&intern, None), "argon"))
        .await
        .expect("intern recall");
    assert_eq!(
        unseen,
        RecallOutcome::Skipped {
            reason: SkipReason::NoFacts
        }
    );
    h.finish().await;
}

#[tokio::test]
async fn a_subject_reached_through_one_link_hop_ranks_below_the_direct_hit() {
    let h = Harness::with("brain_ws_links", Setup::confirming()).await;
    let seeded = seed(&h).await;
    for (fact, subject) in [(&seeded.full_text, "acme"), (&seeded.subject, "omega")] {
        sqlx::query(
            "INSERT INTO brain_fact_links (fact_id, subject_kind, subject_id, via_field)
             VALUES ($1, $2, $3, 'partner')",
        )
        .bind(fact.as_str())
        .bind(PRODUCT)
        .bind(subject)
        .execute(&h.pg)
        .await
        .expect("link");
    }
    let store = BrainRecallStore::new((*h.repos).clone(), h.profile.recall.stale);
    let ctx = RecallCtx::new(seeded.reader.clone(), RequestActor::Human, Utc::now())
        .with_project(PROJECT)
        .with_held_keys(vec![
            "org".to_owned(),
            format!("project:{PROJECT}"),
            format!("user:{}", seeded.reader),
        ]);
    let query = RecallQuery::new("acme".to_owned(), 20);
    let direct = store.by_subject(&query, &ctx).await.expect("subject tier");
    let got: Vec<BrainFactId> = direct.iter().map(|f| f.id.clone()).collect();
    assert_eq!(
        got,
        vec![seeded.subject.clone()],
        "only the named subject's fact"
    );

    let linked = store.by_linked(&query, &ctx).await.expect("linked tier");
    let got: Vec<BrainFactId> = linked.iter().map(|f| f.id.clone()).collect();
    assert_eq!(
        got,
        vec![seeded.full_text.clone()],
        "the fact that links to acme"
    );
    assert!(
        !got.contains(&seeded.context),
        "nothing is reached through acme's own fact: {got:?}"
    );

    let mut project_only = ctx.clone();
    project_only.held_keys = vec![format!("project:{PROJECT}")];
    let still = store
        .by_linked(&query, &project_only)
        .await
        .expect("linked");
    assert_eq!(
        still.len(),
        0,
        "project candidates only; the linked org fact is not one"
    );
    h.finish().await;
}
