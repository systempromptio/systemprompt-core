use chrono::Utc;
use systemprompt_brain::recall::BrainRecallStore;
use systemprompt_brain::recall::fusion::{TierHits, fuse};
use systemprompt_extension::brain::{
    FactRecallSource, MemoryRecall, RecallConversation, RecallOutcome, RecallReceipt, ReuseHint,
    SkipReason,
};
use systemprompt_identifiers::{AiRequestId, BrainFactId, GatewayConversationId};
use systemprompt_models::auth::RequestActor;
use systemprompt_models::brain::{
    BrainFact, FactStatus, RecallCtx, RecallQuery, RetrievalDoc, RetrievalTier, ScopeLevel,
};

use crate::harness::{Harness, Setup};
use crate::recall_seed::{HINTS, PROJECT, Seeded, seed};
use crate::recall_support::{ids, injected, principal, request};

fn idset(facts: &[BrainFact]) -> Vec<BrainFactId> {
    facts.iter().map(|f| f.id.clone()).collect()
}

fn assert_excludes(tier: &str, facts: &[BrainFact], seeded: &Seeded) {
    for id in &seeded.excluded {
        assert!(
            !idset(facts).contains(id),
            "{tier} returned excluded fact {id}"
        );
    }
}

#[tokio::test]
async fn each_tier_returns_its_fact_and_fusion_ranks_the_fact_two_tiers_found_first() {
    let h = Harness::with("brain_recall_tiers", Setup::confirming()).await;
    let seeded = seed(&h).await;
    let store = BrainRecallStore::new((*h.repos).clone(), h.profile.recall.stale);
    let ctx = RecallCtx::new(seeded.reader.clone(), RequestActor::Human, Utc::now())
        .with_project(PROJECT)
        .with_scopes(ScopeLevel::ALL.to_vec());
    let query = RecallQuery {
        subject_hints: HINTS.to_owned(),
        limit: 20,
    };

    let pinned = store.pinned(&ctx, 20).await.expect("pinned");
    let subject = store.by_subject(&query, &ctx).await.expect("subject");
    let context = store.by_context(&query, &ctx).await.expect("context");
    let text = store.full_text(&query, &ctx).await.expect("full text");

    assert_eq!(idset(&pinned), vec![seeded.pinned.clone()]);
    assert!(idset(&subject).contains(&seeded.subject), "{subject:?}");
    assert_eq!(idset(&context), vec![seeded.context.clone()]);
    assert!(idset(&text).contains(&seeded.full_text), "{text:?}");
    for (tier, facts) in [
        ("subject", &subject),
        ("full text", &text),
        ("pinned", &pinned),
    ] {
        assert_excludes(tier, facts, &seeded);
    }

    let retrieval = RetrievalDoc::default();
    let fused = fuse(
        vec![
            TierHits::new(RetrievalTier::Pinned, pinned),
            TierHits::new(RetrievalTier::Subject, subject),
            TierHits::new(RetrievalTier::Context, context),
            TierHits::new(RetrievalTier::FullText, text),
        ],
        &retrieval.tiers,
        retrieval.rrf_k,
    );
    assert_eq!(
        fused[0].fact.id, seeded.subject,
        "subject + full text outranks one tier"
    );
    assert_eq!(fused[0].ranks.len(), 2);
    h.finish().await;
}

#[tokio::test]
async fn the_recall_service_injects_every_tier_and_nothing_stale_due_retired_or_disputed() {
    let h = Harness::with("brain_recall_service", Setup::confirming()).await;
    let seeded = seed(&h).await;

    let outcome = h
        .recall()
        .recall(&request(principal(&seeded.reader, Some(PROJECT)), HINTS))
        .await
        .expect("recall");

    let got = ids(&outcome);
    for id in [
        &seeded.pinned,
        &seeded.subject,
        &seeded.context,
        &seeded.full_text,
    ] {
        assert!(got.contains(id), "{id} missing from {got:?}");
    }
    for id in &seeded.excluded {
        assert!(!got.contains(id), "excluded {id} was injected");
    }
    assert_eq!(
        h.fact(&seeded.subject).await.status,
        FactStatus::Confirmed,
        "the seeded facts were confirmed on capture"
    );
    h.finish().await;
}

#[tokio::test]
async fn a_same_turn_recall_returns_the_stored_block_byte_for_byte() {
    let h = Harness::with("brain_recall_same_turn", Setup::confirming()).await;
    let seeded = seed(&h).await;
    let service = h.recall();
    let conversation = RecallConversation {
        gateway_conversation_id: Some(
            GatewayConversationId::try_new("ctx_0123456789abcdef").expect("conversation"),
        ),
        ..RecallConversation::default()
    };
    let mut first = request(principal(&seeded.reader, Some(PROJECT)), HINTS);
    first.conversation = conversation.clone();
    let outcome = service.recall(&first).await.expect("new turn");
    let block = injected(&outcome).expect("injected").clone();
    service
        .record(&RecallReceipt::new(
            &first,
            AiRequestId::generate(),
            outcome,
        ))
        .await
        .expect("record");

    let again = service
        .recall(&first.clone().with_reuse(ReuseHint::SameTurn))
        .await
        .expect("same turn");
    let reused = injected(&again).expect("the stored block is reused");
    assert_eq!(reused.render(), block.render(), "identical bytes");
    assert_eq!(reused.ids(), block.ids());
    assert!(
        reused.ids().contains('@'),
        "ids carry their revision: {}",
        reused.ids()
    );

    let mut stranger = first.with_reuse(ReuseHint::SameTurn);
    stranger.conversation.gateway_conversation_id =
        Some(GatewayConversationId::try_new("ctx_fedcba9876543210").expect("conversation"));
    assert_eq!(
        service.recall(&stranger).await.expect("other conversation"),
        RecallOutcome::Skipped {
            reason: SkipReason::NotNewTurn
        },
        "negative control: another conversation has no stored block"
    );
    h.finish().await;
}
