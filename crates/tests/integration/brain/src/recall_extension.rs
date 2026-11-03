//! An extension retriever introduces facts the core tiers did not find,
//! and an introduced fact goes through the same visibility check as any
//! other: a stranger's user-scoped fact is withheld as scope, never
//! injected (the leak test; the ownership topic is narrowed to a role
//! nobody holds, as its default `everyone` audience would widen the fact
//! to the org). At the default zero weight the tier is off.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::brain::{FactRetriever, MemoryRecall, RecallError, WithheldReason};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::{FactScope, RecallCtx, RecallQuery, RetrievalDoc, TierWeights};
use systemprompt_models::notifications::AudienceSubject;

use crate::events::entered;
use crate::harness::{Harness, Setup};
use crate::plane::{self, ENGINEERING, OWNERSHIP, kind};
use crate::recall_seed::{HINTS, PROJECT, seed};
use crate::recall_support::{ids, injected, principal, request};

struct FixedRetriever {
    ids: Vec<BrainFactId>,
}

#[async_trait]
impl FactRetriever for FixedRetriever {
    fn name(&self) -> &'static str {
        "test.fixed"
    }

    async fn retrieve(
        &self,
        _query: &RecallQuery,
        _ctx: &RecallCtx,
    ) -> Result<Vec<BrainFactId>, RecallError> {
        Ok(self.ids.clone())
    }
}

async fn enable_extension(h: &Harness) {
    let mut declared = plane::declared();
    let engineering = declared
        .topics
        .iter_mut()
        .find(|t| t.id.as_str() == ENGINEERING)
        .expect("engineering topic");
    engineering.audience = vec![AudienceSubject::Role("engineering_lead".to_owned())];
    declared.retrieval = Some(RetrievalDoc {
        tiers: TierWeights {
            extension: 1.0,
            ..TierWeights::default()
        },
        ..RetrievalDoc::default()
    });
    plane::apply(&h.pg, &declared).await;
}

async fn unseen_fact(h: &Harness, scope: FactScope) -> BrainFactId {
    let author = h.user("author", &[]).await;
    h.emit(&entered(
        &author,
        kind(OWNERSHIP),
        "kappa",
        serde_json::json!({ "team": "unrelated" }),
        scope,
    ))
    .await;
    h.drain().await;
    h.fact_ids("kappa")
        .await
        .into_iter()
        .next()
        .expect("the unseen fact")
}

fn retriever(id: &BrainFactId) -> Vec<Arc<dyn FactRetriever>> {
    vec![Arc::new(FixedRetriever {
        ids: vec![id.clone()],
    })]
}

#[tokio::test]
async fn an_extension_introduces_a_visible_fact_no_core_tier_found() {
    let h = Harness::with("brain_recall_ext_seen", Setup::confirming()).await;
    let seeded = seed(&h).await;
    let unseen = unseen_fact(&h, FactScope::Org).await;
    let req = request(principal(&seeded.reader, Some(PROJECT)), HINTS);

    let without = h.recall().recall(&req).await.expect("core recall");
    assert!(!ids(&without).contains(&unseen), "no core tier finds kappa");

    let off = h
        .recall_with_retrievers(Vec::new(), retriever(&unseen))
        .recall(&req)
        .await
        .expect("zero-weight recall");
    assert_eq!(
        ids(&off),
        ids(&without),
        "tiers.extension 0 is the core recall"
    );

    enable_extension(&h).await;
    let on = h
        .recall_with_retrievers(Vec::new(), retriever(&unseen))
        .recall(&req)
        .await
        .expect("extension recall");
    let got = ids(&on);
    assert!(got.contains(&unseen), "{unseen} missing from {got:?}");
    assert!(got.contains(&seeded.subject), "core hits stay");
}

#[tokio::test]
async fn an_introduced_fact_the_principal_may_not_see_is_withheld_as_scope() {
    let h = Harness::with("brain_recall_ext_leak", Setup::confirming()).await;
    let seeded = seed(&h).await;
    let stranger = h.user("stranger", &[]).await;
    let private = unseen_fact(&h, FactScope::User(stranger)).await;
    enable_extension(&h).await;

    let outcome = h
        .recall_with_retrievers(Vec::new(), retriever(&private))
        .recall(&request(principal(&seeded.reader, Some(PROJECT)), HINTS))
        .await
        .expect("recall");
    let block = injected(&outcome).expect("block");
    assert!(
        !ids(&outcome).contains(&private),
        "a stranger's fact leaked through the extension tier"
    );
    assert!(
        block
            .withheld
            .by_reason
            .get(&WithheldReason::Scope)
            .is_some_and(|n| *n >= 1),
        "the introduced fact is counted as withheld by scope: {:?}",
        block.withheld.by_reason
    );
}
