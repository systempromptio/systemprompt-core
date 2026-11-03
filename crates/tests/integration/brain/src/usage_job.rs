//! `brain_usage` writes each fact's Laplace-smoothed rates from the
//! recalls and citations of the results window, counts a same-turn reuse
//! as no injection, and records no revision.

use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::Utc;
use systemprompt_brain::NewRecall;
use systemprompt_brain::jobs::BrainUsageJob;
use systemprompt_brain::outcomes::OutcomeService;
use systemprompt_brain::schema::BRAIN_USAGE_COLUMNS;
use systemprompt_extension::brain::RecallConversation;
use systemprompt_identifiers::{AiRequestId, BrainFactId, BrainRecallId, UserId};
use systemprompt_models::brain::{FactRef, RecallAudit};
use systemprompt_models::origin::ClientKind;

use crate::events::{Landed, price};
use crate::harness::Harness;
use crate::jobs_support::run;

fn close(left: f64, right: f64) -> bool {
    (left - right).abs() < 1e-9
}

async fn capture(h: &Harness, products: &[&str]) {
    let author = h.user("author", &[]).await;
    for product in products {
        h.model
            .push(serde_json::json!([price(product, "emea", "1200")]));
        h.emit(&Landed::new().event(&author)).await;
    }
    h.drain().await;
}

async fn recall(h: &Harness, user: &UserId, facts: &[&BrainFactId], tokens: u32) -> AiRequestId {
    let ai_request_id = AiRequestId::generate();
    let audit = RecallAudit {
        recall_id: BrainRecallId::generate(),
        injected: facts
            .iter()
            .map(|id| FactRef {
                id: (*id).clone(),
                revision: 1,
            })
            .collect(),
        withheld: BTreeMap::new(),
        tokens,
        as_of: Utc::now(),
    };
    let recall = NewRecall::builder(user.clone(), ClientKind::ClaudeCode, audit)
        .conversation(RecallConversation {
            ai_request_id: Some(ai_request_id.clone()),
            ..RecallConversation::default()
        })
        .rendered_block("<team_memory/>")
        .build();
    h.repos
        .recalls
        .record_detached(&recall)
        .await
        .expect("recall");
    ai_request_id
}

#[tokio::test]
async fn usage_rates_follow_injections_and_citations_without_a_revision() {
    let h = Harness::new("brain_usage_job").await;
    capture(&h, &["cited", "ignored", "unshown"]).await;
    let cited = h.only_fact("cited").await;
    let ignored = h.only_fact("ignored").await;
    let unshown = h.only_fact("unshown").await;
    let reader = h.user("reader", &[]).await;

    let shown = recall(&h, &reader, &[&cited.id, &ignored.id], 40).await;
    recall(&h, &reader, &[&cited.id, &ignored.id], 0).await;
    let service = OutcomeService::new(
        Arc::clone(&h.repos.recalls),
        Arc::clone(&h.repos.outcomes),
        Vec::new(),
    );
    let text = format!("Per {}, the price stands.", cited.id);
    assert_eq!(
        service
            .observe_response(&shown, &text)
            .await
            .expect("observe"),
        1
    );

    let result = run(&h, &BrainUsageJob, h.profile.clone(), None).await;
    assert!(result.success, "{result:?}");
    assert_eq!(result.items_processed, Some(3));

    let after_cited = h.fact(&cited.id).await;
    assert!(
        close(after_cited.usage_cited_rate, 2.0 / 3.0),
        "{after_cited:?}"
    );
    assert!(close(after_cited.usage_ignored_rate, 1.0 / 3.0));
    let after_ignored = h.fact(&ignored.id).await;
    assert!(
        close(after_ignored.usage_cited_rate, 1.0 / 3.0),
        "{after_ignored:?}"
    );
    assert!(close(after_ignored.usage_ignored_rate, 2.0 / 3.0));
    let after_unshown = h.fact(&unshown.id).await;
    assert!(close(after_unshown.usage_cited_rate, 0.5));
    assert!(close(after_unshown.usage_ignored_rate, 0.5));

    for (before, after) in [
        (&cited, &after_cited),
        (&ignored, &after_ignored),
        (&unshown, &after_unshown),
    ] {
        assert_eq!(after.revision, before.revision, "no revision bump");
        assert_eq!(after.updated_at, before.updated_at, "updated_at untouched");
    }
    assert_eq!(h.total("brain_fact_revisions").await, 3);
    assert_eq!(
        h.count(
            "SELECT COUNT(*) FROM brain_facts WHERE usage_updated_at IS NOT NULL AND $1 <> ''",
            "x"
        )
        .await,
        3
    );
    h.finish().await;
}

#[tokio::test]
async fn the_usage_columns_exist_on_a_fresh_schema() {
    let h = Harness::new("brain_usage_columns").await;
    for column in BRAIN_USAGE_COLUMNS {
        assert_eq!(
            h.count(
                "SELECT COUNT(*) FROM information_schema.columns \
                 WHERE table_name = 'brain_facts' AND column_name = $1",
                column
            )
            .await,
            1,
            "{column}"
        );
    }
    h.finish().await;
}
