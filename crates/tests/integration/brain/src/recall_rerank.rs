//! A registered reranker reorders the facts the reader can see before the
//! budget fill.
//!
//! The injected order is the order it gave and the
//! receipt names it with the rank it gave each injected fact. A reranker
//! that errors leaves the fused order untouched and the receipt records
//! no reranker.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::brain::{
    MemoryRecall, RecallReceipt, RerankCandidate, RerankError, RerankQuery, Reranker,
};
use systemprompt_identifiers::{AiRequestId, BrainFactId};
use systemprompt_models::brain::RecallRerank;

use crate::harness::{Harness, Setup};
use crate::recall_seed::{HINTS, PROJECT, seed};
use crate::recall_support::{ids, injected, principal, request};

struct Reversing;

#[async_trait]
impl Reranker for Reversing {
    fn name(&self) -> &'static str {
        "test.reverse"
    }

    async fn rerank(
        &self,
        _query: &RerankQuery,
        candidates: &[RerankCandidate],
    ) -> Result<Vec<BrainFactId>, RerankError> {
        Ok(candidates.iter().rev().map(|c| c.id.clone()).collect())
    }
}

struct Failing;

#[async_trait]
impl Reranker for Failing {
    fn name(&self) -> &'static str {
        "test.failing"
    }

    async fn rerank(
        &self,
        _query: &RerankQuery,
        _candidates: &[RerankCandidate],
    ) -> Result<Vec<BrainFactId>, RerankError> {
        Err(RerankError::Other {
            reranker: "test.failing",
            source: "no model".into(),
        })
    }
}

async fn recorded_rerank(h: &Harness, ai_request_id: &AiRequestId) -> Option<RecallRerank> {
    let row: Option<serde_json::Value> =
        sqlx::query_scalar("SELECT reranked FROM brain_recalls WHERE ai_request_id = $1")
            .bind(ai_request_id.as_str())
            .fetch_one(&h.pg)
            .await
            .expect("the recall row");
    row.map(|value| serde_json::from_value(value).expect("a rerank record"))
}

#[tokio::test]
async fn a_reranker_orders_the_fill_and_the_receipt_names_it() {
    let h = Harness::with("brain_recall_rerank", Setup::confirming()).await;
    let seeded = seed(&h).await;
    let req = request(principal(&seeded.reader, Some(PROJECT)), HINTS);

    let fused = h.recall().recall(&req).await.expect("fused recall");
    let fused_ids = ids(&fused);
    assert!(fused_ids.len() >= 2, "the seed injects several facts");
    assert!(
        injected(&fused).expect("block").reranked.is_none(),
        "no reranker, no record"
    );

    let service = h.recall_with_rerankers(vec![Arc::new(Reversing)]);
    let outcome = service.recall(&req).await.expect("reranked recall");
    let got = ids(&outcome);
    let mut reversed = fused_ids.clone();
    reversed.reverse();
    assert_eq!(got, reversed, "the fill follows the reranker's order");
    let block = injected(&outcome).expect("block");
    let reranked = block
        .reranked
        .clone()
        .expect("the block names the reranker");
    assert_eq!(reranked.reranker, "test.reverse");
    let ranks: Vec<(BrainFactId, u32)> = reranked
        .ranks
        .iter()
        .map(|r| (r.id.clone(), r.rank))
        .collect();
    let expected: Vec<(BrainFactId, u32)> = got
        .iter()
        .enumerate()
        .map(|(at, id)| (id.clone(), u32::try_from(at + 1).expect("rank")))
        .collect();
    assert_eq!(
        ranks, expected,
        "each injected fact carries the rank it was given"
    );

    let ai_request_id = AiRequestId::generate();
    service
        .record(&RecallReceipt::new(&req, ai_request_id.clone(), outcome))
        .await
        .expect("record");
    let recorded = recorded_rerank(&h, &ai_request_id)
        .await
        .expect("the receipt names the reranker");
    assert_eq!(recorded, reranked);
}

#[tokio::test]
async fn a_failing_reranker_leaves_the_fused_order_and_records_nothing() {
    let h = Harness::with("brain_recall_rerank_fail", Setup::confirming()).await;
    let seeded = seed(&h).await;
    let req = request(principal(&seeded.reader, Some(PROJECT)), HINTS);

    let fused = h.recall().recall(&req).await.expect("fused recall");
    let service = h.recall_with_rerankers(vec![Arc::new(Failing)]);
    let outcome = service
        .recall(&req)
        .await
        .expect("recall survives the error");
    assert_eq!(ids(&outcome), ids(&fused), "the fused order stands");
    assert!(injected(&outcome).expect("block").reranked.is_none());

    let ai_request_id = AiRequestId::generate();
    service
        .record(&RecallReceipt::new(&req, ai_request_id.clone(), outcome))
        .await
        .expect("record");
    assert!(recorded_rerank(&h, &ai_request_id).await.is_none());
}
