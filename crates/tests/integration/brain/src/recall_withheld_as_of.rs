//! Recall reporting and time: the lifecycle counts are reported, the
//! count cut is `max_facts`, an `as_of` read admits only facts observed by
//! then, and a block produced after the deadline is reused by the turn's
//! next request.

use chrono::{Duration, Utc};
use systemprompt_extension::brain::{
    MemoryRecall, RecallConversation, RecallOutcome, RecallReceipt, ReuseHint, SkipReason,
    WithheldReason,
};
use systemprompt_identifiers::{AiRequestId, GatewayConversationId};
use systemprompt_models::brain::RetrievalDoc;

use crate::harness::{Harness, Setup};
use crate::plane;
use crate::recall_seed::{HINTS, PROJECT, seed};
use crate::recall_support::{ids, injected, principal, request};

#[tokio::test]
async fn the_lifecycle_counts_and_the_count_cut_are_reported_honestly() {
    let h = Harness::with("brain_ws_withheld", Setup::confirming()).await;
    let seeded = seed(&h).await;
    let full = h
        .recall()
        .recall(&request(principal(&seeded.reader, Some(PROJECT)), HINTS))
        .await
        .expect("recall");
    let block = injected(&full).expect("block");
    for reason in [
        WithheldReason::Stale,
        WithheldReason::Disputed,
        WithheldReason::ReviewDue,
    ] {
        assert_eq!(
            block.withheld.by_reason.get(&reason),
            Some(&1),
            "{reason:?}"
        );
    }
    assert_eq!(block.withheld.by_reason.get(&WithheldReason::Scope), None);

    let mut declared = plane::declared();
    declared.retrieval = Some(RetrievalDoc {
        max_facts: 1,
        ..RetrievalDoc::default()
    });
    plane::apply(&h.pg, &declared).await;
    let capped = h
        .recall()
        .recall(&request(principal(&seeded.reader, Some(PROJECT)), HINTS))
        .await
        .expect("capped recall");
    let block = injected(&capped).expect("block");
    assert_eq!(block.facts.len(), 1);
    assert!(
        block
            .withheld
            .by_reason
            .get(&WithheldReason::MaxFacts)
            .is_some_and(|n| *n >= 3),
        "{:?}",
        block.withheld
    );
    assert_eq!(block.withheld.by_reason.get(&WithheldReason::Budget), None);
    h.finish().await;
}

#[tokio::test]
async fn an_as_of_read_admits_only_facts_observed_by_then() {
    let h = Harness::with("brain_ws_as_of", Setup::confirming()).await;
    let seeded = seed(&h).await;
    let two_days_ago = Utc::now() - Duration::days(2);
    sqlx::query("UPDATE brain_facts SET observed_at = $2 WHERE id = $1")
        .bind(seeded.subject.as_str())
        .bind(two_days_ago)
        .execute(&h.pg)
        .await
        .expect("age the observation");
    let as_of = Utc::now() - Duration::days(1);

    let then = h
        .recall()
        .recall(&request(principal(&seeded.reader, Some(PROJECT)), HINTS).with_as_of(Some(as_of)))
        .await
        .expect("as-of recall");
    assert_eq!(ids(&then), vec![seeded.subject.clone()]);
    assert_eq!(injected(&then).expect("block").as_of, as_of);

    let now = h
        .recall()
        .recall(&request(principal(&seeded.reader, Some(PROJECT)), HINTS))
        .await
        .expect("current recall");
    assert!(ids(&now).len() > 1);
    h.finish().await;
}

#[tokio::test]
async fn a_block_produced_after_the_deadline_is_reused_by_the_next_request_of_the_turn() {
    let h = Harness::with("brain_ws_late", Setup::confirming()).await;
    let seeded = seed(&h).await;
    let service = h.recall();
    let mut first = request(principal(&seeded.reader, Some(PROJECT)), HINTS);
    first.conversation = RecallConversation {
        gateway_conversation_id: Some(
            GatewayConversationId::try_new("ctx_0123456789abcdef").expect("conversation"),
        ),
        ..RecallConversation::default()
    };
    let late = injected(&service.recall(&first).await.expect("recall"))
        .expect("block")
        .clone();
    let receipt = RecallReceipt::new(
        &first,
        AiRequestId::generate(),
        RecallOutcome::Skipped {
            reason: SkipReason::Deadline,
        },
    )
    .with_late(late.clone());
    service.record(&receipt).await.expect("record late");
    let (skipped, rendered): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT skipped, rendered_block FROM brain_recalls")
            .fetch_one(&h.pg)
            .await
            .expect("one row");
    assert_eq!(skipped.as_deref(), Some("deadline"));
    assert_eq!(rendered.as_deref(), Some(late.render().as_str()));

    let next = service
        .recall(&first.with_reuse(ReuseHint::SameTurn))
        .await
        .expect("same turn");
    let reused = injected(&next).expect("reused");
    assert_eq!(reused.render(), late.render(), "identical bytes");
    assert!(reused.reused_after_deadline);
    assert!(
        next.descriptor().ends_with(" reused_after_deadline"),
        "{}",
        next.descriptor()
    );
    h.finish().await;
}
