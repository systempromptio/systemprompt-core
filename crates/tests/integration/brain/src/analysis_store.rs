use std::sync::Arc;

use chrono::{Duration, Utc};
use systemprompt_brain::analyses::AnalysisRepository;
use systemprompt_extension::brain::{AnalysisWindow, MemoryRecall, RecallReceipt};
use systemprompt_identifiers::AiRequestId;

use crate::events::{Landed, price};
use crate::harness::{Harness, Setup};
use crate::plane::{PRICING, kind};
use crate::recall_support::{ids, principal, request};

fn around_now() -> AnalysisWindow {
    let now = Utc::now();
    AnalysisWindow {
        since: now - Duration::hours(1),
        until: now + Duration::hours(1),
    }
}

fn long_ago() -> AnalysisWindow {
    let now = Utc::now();
    AnalysisWindow {
        since: now - Duration::days(30),
        until: now - Duration::days(29),
    }
}

#[tokio::test]
async fn an_empty_brain_has_no_usage_in_any_window() {
    let h = Harness::new("brain_analysis_empty").await;
    let store = AnalysisRepository::new(Arc::new(h.pg.clone()));
    assert!(
        store
            .kind_usage(around_now())
            .await
            .expect("kinds")
            .is_empty()
    );
    assert!(
        store
            .workflow_kind_usage(around_now())
            .await
            .expect("workflows")
            .is_empty()
    );
    assert!(
        store
            .project_recalls(around_now())
            .await
            .expect("projects")
            .is_empty()
    );
    h.finish().await;
}

#[tokio::test]
async fn usage_counts_live_facts_injections_and_recalls_per_project() {
    let h = Harness::with("brain_analysis_usage", Setup::confirming()).await;
    let owner = h.user("owner", &[]).await;
    h.model.push(serde_json::json!([
        price("acme", "emea", "1200"),
        price("globex", "emea", "700"),
    ]));
    h.emit(&Landed::new().event(&owner)).await;
    h.drain().await;
    let store = AnalysisRepository::new(Arc::new(h.pg.clone()));

    let before = store.kind_usage(around_now()).await.expect("kinds");
    assert_eq!(before.len(), 1, "{before:?}");
    assert_eq!(before[0].kind, kind(PRICING));
    assert_eq!(before[0].facts, 2);
    assert_eq!(before[0].injections, 0);
    assert_eq!(before[0].citations, 0);

    let service = h.recall();
    let alpha_req = request(principal(&owner, Some("alpha")), "acme globex price");
    let hit = service.recall(&alpha_req).await.expect("recall");
    let injected = ids(&hit).len() as i64;
    assert!(injected > 0, "{hit:?}");
    service
        .record(&RecallReceipt::new(
            &alpha_req,
            AiRequestId::generate(),
            hit.clone(),
        ))
        .await
        .expect("record alpha");
    let beta_req = request(principal(&owner, Some("beta")), "zzzz qqqq");
    let miss = service.recall(&beta_req).await.expect("second recall");
    service
        .record(&RecallReceipt::new(
            &beta_req,
            AiRequestId::generate(),
            miss,
        ))
        .await
        .expect("record beta");
    assert_eq!(store.kind_usage(around_now()).await.expect("k")[0].facts, 2);

    let after = store.kind_usage(around_now()).await.expect("kinds");
    assert_eq!(after[0].facts, 2);
    assert_eq!(after[0].injections, injected);
    assert!(
        store.kind_usage(long_ago()).await.expect("old")[0].injections == 0,
        "an old window sees no injection"
    );

    let workflows = store
        .workflow_kind_usage(around_now())
        .await
        .expect("workflows");
    assert_eq!(workflows.len(), 1, "{workflows:?}");
    assert_eq!(workflows[0].workflow, None);
    assert_eq!(workflows[0].kind, kind(PRICING));
    assert_eq!(workflows[0].injections, injected);
    assert_eq!(workflows[0].citations, 0);
    assert!(
        store
            .workflow_kind_usage(long_ago())
            .await
            .expect("old")
            .is_empty()
    );

    let projects = store.project_recalls(around_now()).await.expect("projects");
    let summary: Vec<(Option<String>, i64, i64)> = projects
        .into_iter()
        .map(|p| (p.project, p.recalls, p.empty))
        .collect();
    let alpha = summary
        .iter()
        .find(|(p, _, _)| p.as_deref() == Some("alpha"))
        .expect("alpha");
    assert_eq!((alpha.1, alpha.2), (1, 0));
    if let Some(beta) = summary
        .iter()
        .find(|(p, _, _)| p.as_deref() == Some("beta"))
    {
        assert_eq!((beta.1, beta.2), (1, 1));
    }
    assert!(
        store
            .project_recalls(long_ago())
            .await
            .expect("old")
            .is_empty()
    );
    h.finish().await;
}
