//! An introduced fact is admitted by the tier SQL's own rule before it
//! is fused, so the extension tier cannot inject what no core tier could
//! have returned: another user's proposed fact, a fact whose scope level
//! the recall does not read, or a fact observed after `as_of`; each is
//! counted as withheld. A loader that fails introduces nothing and the
//! recall goes on over the core tiers.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use systemprompt_brain::recall::{
    BrainRecallService, BrainRecallStore, FactLoader, PlaneDeclaredSource,
};
use systemprompt_extension::brain::{MemoryRecall, RecallError, RecallOutcome, WithheldReason};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::{BrainFact, FactScope, ScopeLevel};

use crate::harness::{Harness, Setup};
use crate::recall_extension::{enable_extension, retriever, unseen_fact};
use crate::recall_seed::{HINTS, PROJECT, seed};
use crate::recall_support::{ids, injected, principal, request};

struct FailingLoader;

#[async_trait]
impl FactLoader for FailingLoader {
    async fn load(&self, _ids: &[BrainFactId]) -> Result<Vec<BrainFact>, RecallError> {
        Err(RecallError::Other {
            component: "test.loader",
            source: "the fact store is unreachable".into(),
        })
    }
}

fn scope_withheld(outcome: &RecallOutcome) -> u32 {
    injected(outcome)
        .expect("block")
        .withheld
        .by_reason
        .get(&WithheldReason::Scope)
        .copied()
        .unwrap_or(0)
}

#[tokio::test]
async fn a_strangers_proposed_org_fact_is_withheld_not_injected() {
    let h = Harness::with("brain_recall_ext_proposed", Setup::confirming()).await;
    let seeded = seed(&h).await;
    let stranger = h.user("stranger", &[]).await;
    let proposed = unseen_fact(&h, FactScope::Org).await;
    sqlx::query("UPDATE brain_facts SET status = 'proposed', owner_user_id = $2 WHERE id = $1")
        .bind(proposed.as_str())
        .bind(stranger.as_str())
        .execute(&h.pg)
        .await
        .expect("propose the fact for the stranger");
    enable_extension(&h).await;

    let outcome = h
        .recall_with_retrievers(Vec::new(), retriever(&proposed))
        .recall(&request(principal(&seeded.reader, Some(PROJECT)), HINTS))
        .await
        .expect("recall");
    assert!(
        !ids(&outcome).contains(&proposed),
        "another user's proposed fact leaked through the extension tier"
    );
    assert!(scope_withheld(&outcome) >= 1, "{outcome:?}");
    assert!(ids(&outcome).contains(&seeded.subject), "core hits stay");
    h.finish().await;
}

#[tokio::test]
async fn a_fact_outside_the_recall_scopes_is_withheld_even_when_the_principal_holds_it() {
    let mut setup = Setup::confirming();
    setup.profile.recall.scopes = vec![ScopeLevel::User, ScopeLevel::Group, ScopeLevel::Org];
    let h = Harness::with("brain_recall_ext_scope_level", setup).await;
    let seeded = seed(&h).await;
    let project = unseen_fact(&h, FactScope::Project(PROJECT.to_owned())).await;
    enable_extension(&h).await;

    let outcome = h
        .recall_with_retrievers(Vec::new(), retriever(&project))
        .recall(&request(principal(&seeded.reader, Some(PROJECT)), HINTS))
        .await
        .expect("recall");
    let got = ids(&outcome);
    assert!(
        !got.contains(&project),
        "a project fact was introduced although the recall reads no project scope"
    );
    assert!(
        !got.contains(&seeded.context),
        "the context tier reads no project scope either"
    );
    assert!(scope_withheld(&outcome) >= 1, "{outcome:?}");
    h.finish().await;
}

#[tokio::test]
async fn a_fact_observed_after_as_of_is_withheld_on_an_as_of_recall() {
    let h = Harness::with("brain_recall_ext_as_of", Setup::confirming()).await;
    let seeded = seed(&h).await;
    sqlx::query("UPDATE brain_facts SET observed_at = $2 WHERE id = $1")
        .bind(seeded.subject.as_str())
        .bind(Utc::now() - Duration::days(2))
        .execute(&h.pg)
        .await
        .expect("age the observation");
    let later = unseen_fact(&h, FactScope::Org).await;
    enable_extension(&h).await;
    let as_of = Utc::now() - Duration::days(1);

    let then = h
        .recall_with_retrievers(Vec::new(), retriever(&later))
        .recall(&request(principal(&seeded.reader, Some(PROJECT)), HINTS).with_as_of(Some(as_of)))
        .await
        .expect("as-of recall");
    assert_eq!(
        ids(&then),
        vec![seeded.subject.clone()],
        "only the fact observed by as_of"
    );
    assert!(scope_withheld(&then) >= 1, "{then:?}");

    let now = h
        .recall_with_retrievers(Vec::new(), retriever(&later))
        .recall(&request(principal(&seeded.reader, Some(PROJECT)), HINTS))
        .await
        .expect("current recall");
    assert!(ids(&now).contains(&later), "observed by now: {now:?}");
    h.finish().await;
}

#[tokio::test]
async fn a_failing_loader_introduces_nothing_and_the_recall_goes_on() {
    let h = Harness::with("brain_recall_ext_loader_fails", Setup::confirming()).await;
    let seeded = seed(&h).await;
    let unseen = unseen_fact(&h, FactScope::Org).await;
    enable_extension(&h).await;
    let req = request(principal(&seeded.reader, Some(PROJECT)), HINTS);
    let core = h.recall().recall(&req).await.expect("core recall");

    let store = Arc::new(
        BrainRecallStore::new((*h.repos).clone(), h.profile.recall.stale)
            .with_working_set(h.profile.recall.working_set),
    );
    let service = BrainRecallService::builder(
        store as _,
        Arc::clone(&h.access) as _,
        Arc::new(PlaneDeclaredSource::new((*h.repos).clone())),
        Arc::new(h.profile.clone()),
    )
    .loader(Arc::new(FailingLoader))
    .retrievers(retriever(&unseen))
    .build();

    let outcome = service
        .recall(&req)
        .await
        .expect("the recall survives the loader");
    assert_eq!(
        ids(&outcome),
        ids(&core),
        "the core recall, nothing introduced"
    );
    h.finish().await;
}
