//! DB-backed: the analysis aggregates, proposal replay attachment and an
//! empty dry run, each against a disposable database with the brain schema.

use std::sync::Arc;

use chrono::{Duration, Utc};
use systemprompt_brain::analyses::AnalysisRepository;
use systemprompt_brain::pipeline::PipelineSeams;
use systemprompt_brain::replay::ReplayService;
use systemprompt_brain::repository::{BrainRepositories, ProposalRepository};
use systemprompt_events::services::durable::OutboxConsumer;
use systemprompt_extension::brain::{AnalysisWindow, BrainModelClient, BrainModelError};
use systemprompt_identifiers::{BrainProposalId, PipelineVersion};
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_models::brain::{
    BrainModelRequest, BrainModelResponse, ProposalDraft, ProposalKind,
};
use systemprompt_test_fixtures::DisposableDb;

struct UnavailableModel;

#[async_trait::async_trait]
impl BrainModelClient for UnavailableModel {
    async fn complete(
        &self,
        _request: BrainModelRequest,
    ) -> Result<BrainModelResponse, BrainModelError> {
        Err(BrainModelError::Unavailable)
    }
}

fn last_day() -> AnalysisWindow {
    let until = Utc::now();
    AnalysisWindow {
        since: until - Duration::days(1),
        until,
    }
}

#[tokio::test]
async fn aggregates_over_an_empty_store_are_empty() {
    let db = DisposableDb::with_schema("brain_analyses").await;
    let pool = db.test_pool().await;
    let store = AnalysisRepository::new(pool.pool());
    assert!(
        store
            .kind_usage(last_day())
            .await
            .expect("usage")
            .is_empty()
    );
    assert!(
        store
            .client_recalls(last_day())
            .await
            .expect("recalls")
            .is_empty()
    );
    db.drop_now().await;
}

#[tokio::test]
async fn a_replay_report_attaches_to_an_existing_proposal_only() {
    let db = DisposableDb::with_schema("brain_replay_attach").await;
    let pool = db.test_pool().await;
    let repos = BrainRepositories::new(&pool);
    let draft = ProposalDraft {
        analysis: "bloat".to_owned(),
        kind: ProposalKind::ConfigDiff,
        summary: "s".to_owned(),
        body: serde_json::json!({"kind": "a"}),
    };
    let id = ProposalRepository::insert(repos.write_pool(), &draft)
        .await
        .expect("insert");
    let report = serde_json::json!({"events": 0});
    assert!(
        ProposalRepository::attach_replay(repos.write_pool(), &id, &report)
            .await
            .expect("attach")
    );
    assert!(
        !ProposalRepository::attach_replay(
            repos.write_pool(),
            &BrainProposalId::generate(),
            &report
        )
        .await
        .expect("attach missing")
    );
    db.drop_now().await;
}

#[tokio::test]
async fn a_dry_run_over_an_empty_window_reports_nothing_and_keeps_nothing() {
    let db = DisposableDb::with_schema("brain_replay_run").await;
    let pool = db.test_pool().await;
    let repos = Arc::new(BrainRepositories::new(&pool));
    let outbox = OutboxConsumer::new(repos.write_pool().clone());
    let service = ReplayService::new(
        Arc::clone(&repos),
        outbox,
        PipelineSeams::default(),
        BrainProfile::default(),
    );
    let version = PipelineVersion::try_new("0123456789abcdef").expect("version");
    let report = service
        .dry_run(last_day(), version, Arc::new(UnavailableModel))
        .await
        .expect("dry run");
    assert_eq!(report.events, 0);
    assert_eq!(report.would_create, 0);
    assert_eq!(report.tokens, 0);
    assert!(!report.truncated);
    db.drop_now().await;
}
