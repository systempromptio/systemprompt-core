use std::sync::Arc;

use chrono::{Duration, Utc};
use systemprompt_brain::error::BrainError;
use systemprompt_brain::pipeline::PipelineSeams;
use systemprompt_brain::replay::{ProposalReplay, ReplayService};
use systemprompt_brain::repository::ProposalRepository;
use systemprompt_events::services::durable::OutboxConsumer;
use systemprompt_extension::brain::AnalysisWindow;
use systemprompt_identifiers::{BrainProposalId, UserId};
use systemprompt_models::brain::{ProposalDraft, ProposalKind, ProposalStatus};

use crate::harness::Harness;

const ADMIN: &[&str] = &["admin"];

async fn draft(h: &Harness, summary: &str) -> BrainProposalId {
    let draft = ProposalDraft {
        analysis: "bloat".to_owned(),
        kind: ProposalKind::ConfigDiff,
        summary: summary.to_owned(),
        body: serde_json::json!({ "drop": ["pricing"] }),
    };
    ProposalRepository::insert(h.repos.write_pool(), &draft)
        .await
        .expect("insert proposal")
}

async fn decision(h: &Harness, id: &BrainProposalId) -> (String, Option<String>, bool) {
    sqlx::query_as::<_, (String, Option<String>, bool)>(
        "SELECT status, decided_by, decided_at IS NOT NULL FROM brain_proposals WHERE id = $1",
    )
    .bind(id.as_str())
    .fetch_one(&h.pg)
    .await
    .expect("proposal row")
}

fn window() -> AnalysisWindow {
    let until = Utc::now();
    AnalysisWindow {
        since: until - Duration::days(1),
        until,
    }
}

async fn admin(h: &Harness) -> UserId {
    h.user("admin", ADMIN).await
}

#[tokio::test]
async fn an_admin_approves_an_open_proposal_once() {
    let h = Harness::new("brain_proposal_approve").await;
    let who = admin(&h).await;
    let caller = Harness::caller(&who, ADMIN);
    let id = draft(&h, "drop pricing").await;

    h.service
        .approve_proposal(&caller, &id)
        .await
        .expect("approve");

    assert_eq!(
        decision(&h, &id).await,
        ("approved".to_owned(), Some(who.as_str().to_owned()), true)
    );
    let again = h.service.approve_proposal(&caller, &id).await;
    assert!(
        matches!(again, Err(BrainError::Forbidden { ref reason }) if reason == "the proposal is not open"),
        "{again:?}"
    );
    let flip = h.service.reject_proposal(&caller, &id).await;
    assert!(
        matches!(flip, Err(BrainError::Forbidden { .. })),
        "{flip:?}"
    );
    assert_eq!(decision(&h, &id).await.0, "approved");
    h.finish().await;
}

#[tokio::test]
async fn an_admin_rejects_an_open_proposal_and_it_leaves_the_open_list() {
    let h = Harness::new("brain_proposal_reject").await;
    let who = admin(&h).await;
    let caller = Harness::caller(&who, ADMIN);
    let kept = draft(&h, "kept").await;
    let rejected = draft(&h, "rejected").await;

    let open = h.service.proposals(&caller, 10).await.expect("list");
    assert_eq!(open.len(), 2);
    assert!(open.iter().all(|p| p.status == ProposalStatus::Open));

    h.service
        .reject_proposal(&caller, &rejected)
        .await
        .expect("reject");

    assert_eq!(
        decision(&h, &rejected).await,
        ("rejected".to_owned(), Some(who.as_str().to_owned()), true)
    );
    let open = h.service.proposals(&caller, 10).await.expect("list");
    assert_eq!(
        open.iter().map(|p| p.id.clone()).collect::<Vec<_>>(),
        vec![kept.clone()]
    );
    assert_eq!(open[0].summary, "kept");
    assert_eq!(open[0].kind, ProposalKind::ConfigDiff);
    assert_eq!(open[0].decided_by, None);
    assert_eq!(
        h.service.proposals(&caller, 0).await.expect("list").len(),
        0
    );
    h.finish().await;
}

#[tokio::test]
async fn deciding_a_missing_proposal_reports_it_not_found() {
    let h = Harness::new("brain_proposal_missing").await;
    let who = admin(&h).await;
    let caller = Harness::caller(&who, ADMIN);
    let missing = BrainProposalId::generate();

    let approved = h.service.approve_proposal(&caller, &missing).await;
    assert!(
        matches!(approved, Err(BrainError::ProposalNotFound(ref id)) if *id == missing),
        "{approved:?}"
    );
    let rejected = h.service.reject_proposal(&caller, &missing).await;
    assert!(
        matches!(rejected, Err(BrainError::ProposalNotFound(_))),
        "{rejected:?}"
    );
    h.finish().await;
}

#[tokio::test]
async fn a_non_admin_may_not_list_decide_or_replay_proposals() {
    let h = Harness::new("brain_proposal_refused").await;
    let who = h.user("member", &["user"]).await;
    let caller = Harness::caller(&who, &["user"]);
    let id = draft(&h, "drop pricing").await;

    assert!(matches!(
        h.service.proposals(&caller, 10).await,
        Err(BrainError::Forbidden { .. })
    ));
    assert!(matches!(
        h.service.approve_proposal(&caller, &id).await,
        Err(BrainError::Forbidden { .. })
    ));
    assert!(matches!(
        h.service.reject_proposal(&caller, &id).await,
        Err(BrainError::Forbidden { .. })
    ));
    assert!(matches!(
        h.service.replay_proposal(&caller, &id, window()).await,
        Err(BrainError::Forbidden { .. })
    ));
    assert_eq!(decision(&h, &id).await, ("open".to_owned(), None, false));
    h.finish().await;
}

#[tokio::test]
async fn replay_without_a_configured_replay_is_refused() {
    let h = Harness::new("brain_proposal_no_replay").await;
    let who = admin(&h).await;
    let caller = Harness::caller(&who, ADMIN);
    let id = draft(&h, "drop pricing").await;

    let refused = h.service.replay_proposal(&caller, &id, window()).await;

    assert!(
        matches!(refused, Err(BrainError::Forbidden { ref reason }) if reason == "replay is not configured"),
        "{refused:?}"
    );
    h.finish().await;
}

fn attach(h: &Harness) {
    let service = ReplayService::new(
        Arc::clone(&h.repos),
        OutboxConsumer::new(h.pg.clone()),
        PipelineSeams::default(),
        h.profile.clone(),
    );
    assert!(h.service.attach_replay(ProposalReplay {
        service: Arc::new(service),
        model: Arc::clone(&h.model) as _,
    }));
}

#[tokio::test]
async fn replay_stores_its_report_on_the_proposal_and_refuses_a_missing_one() {
    let h = Harness::new("brain_proposal_replay").await;
    let who = admin(&h).await;
    let caller = Harness::caller(&who, ADMIN);
    let id = draft(&h, "drop pricing").await;
    attach(&h);

    let report = h
        .service
        .replay_proposal(&caller, &id, window())
        .await
        .expect("replay");

    assert_eq!(report.events, 0);
    assert_eq!(report.would_create, 0);
    let stored: Option<serde_json::Value> =
        sqlx::query_scalar("SELECT replay FROM brain_proposals WHERE id = $1")
            .bind(id.as_str())
            .fetch_one(&h.pg)
            .await
            .expect("replay column");
    let stored = stored.expect("the report is attached");
    assert_eq!(stored["events"], serde_json::json!(0));
    assert_eq!(
        stored["pipeline_version"],
        serde_json::json!(report.pipeline_version.as_str())
    );
    assert_eq!(decision(&h, &id).await.0, "open", "replay decides nothing");

    let missing = BrainProposalId::generate();
    let refused = h.service.replay_proposal(&caller, &missing, window()).await;
    assert!(
        matches!(refused, Err(BrainError::ProposalNotFound(ref m)) if *m == missing),
        "{refused:?}"
    );
    h.finish().await;
}
