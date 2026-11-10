use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use systemprompt_agent_runs::credentials::RunCredentialService;
use systemprompt_agent_runs::execute::harness::{HarnessError, HarnessSource};
use systemprompt_agent_runs::repository::{AgentRunRow, NewAgentRun, RunCredentialRepositories};
use systemprompt_agent_runs::runner::{A2aRunHandler, A2aRunnerConfig};
use systemprompt_agent_runs::service::{ClaimedRun, RunHandler};
use systemprompt_agent_runs::{HarnessRef, RunRequest};
use systemprompt_identifiers::{AgentRunId, ResourceRevisionId, UserId};
use systemprompt_models::managed::RevisionBundle;
use systemprompt_test_fixtures::test_db_pool;
use tokio_util::sync::CancellationToken;

use crate::execute_support::ShHost;
use crate::harness::{bundle, harness_ref};
use crate::repository::{Fixture, fixture, request};

pub(crate) enum Source {
    Bundle(RevisionBundle),
    Fails,
}

#[async_trait]
impl HarnessSource for Source {
    async fn revision_bundle(
        &self,
        _owner: &UserId,
        _revision: &ResourceRevisionId,
    ) -> Result<RevisionBundle, HarnessError> {
        match self {
            Self::Bundle(bundle) => Ok(bundle.clone()),
            Self::Fails => Err(HarnessError::MissingAsset("AGENTS.md".to_owned())),
        }
    }
}

pub(crate) struct Rig {
    pub(crate) f: Fixture,
    data: tempfile::TempDir,
    pub(crate) workspace: tempfile::TempDir,
}

pub(crate) async fn rig() -> Rig {
    Rig {
        f: fixture().await,
        data: tempfile::tempdir().unwrap(),
        workspace: tempfile::tempdir().unwrap(),
    }
}

pub(crate) async fn handler(r: &Rig, script: &str, source: Option<Source>) -> A2aRunHandler {
    let pool = test_db_pool().await;
    let config = A2aRunnerConfig::new("http://127.0.0.1:9", "https://issuer.test", r.data.path());
    let handler = A2aRunHandler::new(
        RunCredentialService::new(
            RunCredentialRepositories::new(
                &pool,
                systemprompt_test_fixtures::oauth_fixture_repository(&pool),
            ),
            systemprompt_users::ApiKeyLifetime::unbounded(),
            systemprompt_test_fixtures::test_token_authority("https://issuer.test"),
            systemprompt_test_fixtures::test_config(),
        ),
        config,
    )
    .with_host(Arc::new(ShHost {
        script: script.to_owned(),
        keep: &[],
    }));
    match source {
        Some(source) => handler.with_harness_source(Arc::new(source)),
        None => handler,
    }
}

pub(crate) async fn claim(r: &Rig, harness: Option<HarnessRef>) -> ClaimedRun {
    let id = AgentRunId::generate();
    let request = RunRequest {
        workspace: r.workspace.path().to_path_buf(),
        harness,
        ..request()
    };
    r.f.repo
        .insert_run(NewAgentRun {
            id: &id,
            origin_kind: &r.f.origin_kind,
            origin_id: "origin-handler",
            user_id: &r.f.user_id,
            request: &request,
            not_before: None,
            task_id: None,
            context_id: None,
        })
        .await
        .expect("insert run");
    let claimed =
        r.f.repo
            .claim_queued(&r.f.origin_kind, "handler-test", Duration::from_secs(90))
            .await
            .expect("claim")
            .expect("claimed row");
    assert_eq!(claimed.row.id, id);
    ClaimedRun::new(claimed, r.f.repo.clone(), &CancellationToken::new(), 3)
}

async fn finished(r: &Rig, id: &AgentRunId) -> AgentRunRow {
    let row = r.f.repo.find_run(id).await.expect("find").expect("row");
    assert!(row.finished_at.is_some(), "run {id} not finished");
    row
}

#[tokio::test]
async fn a_run_cancelled_before_start_finishes_without_executing() {
    let r = rig().await;
    let claimed = claim(&r, None).await;
    let id = claimed.row.id.clone();
    r.f.repo.update_cancel_requested(&id).await.expect("cancel");

    handler(&r, "touch spawned", None)
        .await
        .run(claimed)
        .await
        .expect("run");

    let row = finished(&r, &id).await;
    assert_eq!(row.status, "cancelled");
    assert_eq!(row.error.as_deref(), Some("cancelled before start"));
    assert!(!r.workspace.path().join("spawned").exists());
}

#[tokio::test]
async fn a_harness_without_a_source_is_unavailable() {
    let r = rig().await;
    let bundle = bundle();
    let claimed = claim(&r, Some(harness_ref(&bundle))).await;
    let id = claimed.row.id.clone();

    handler(&r, "echo ran", None)
        .await
        .run(claimed)
        .await
        .expect("run");

    let row = finished(&r, &id).await;
    assert_eq!(row.status, "failed");
    assert_eq!(row.error_class.as_deref(), Some("harness_unavailable"));
    assert_eq!(
        row.error.as_deref(),
        Some("no harness source is configured for this runner")
    );
    assert_eq!(row.harness_digest, None);
}

#[tokio::test]
async fn a_harness_that_fails_to_load_is_unavailable() {
    let r = rig().await;
    let bundle = bundle();
    let claimed = claim(&r, Some(harness_ref(&bundle))).await;
    let id = claimed.row.id.clone();

    handler(&r, "echo ran", Some(Source::Fails))
        .await
        .run(claimed)
        .await
        .expect("run");

    let row = finished(&r, &id).await;
    assert_eq!(row.error_class.as_deref(), Some("harness_unavailable"));
    assert_eq!(
        row.error.as_deref(),
        Some("harness path AGENTS.md has no bytes in the bundle")
    );
}

#[tokio::test]
async fn a_loaded_harness_records_its_digest_and_runs() {
    let r = rig().await;
    let bundle = bundle();
    let harness = harness_ref(&bundle);
    let digest = harness.bundle_digest.as_str().to_owned();
    let claimed = claim(&r, Some(harness)).await;
    let id = claimed.row.id.clone();

    handler(&r, "true", Some(Source::Bundle(bundle)))
        .await
        .run(claimed)
        .await
        .expect("run");

    let row = finished(&r, &id).await;
    assert_eq!(row.status, "succeeded");
    assert_eq!(row.harness_digest.as_deref(), Some(digest.as_str()));
    assert!(r.workspace.path().join("AGENTS.md").exists());
}

#[tokio::test]
async fn an_undecodable_request_finishes_as_host_crashed() {
    let r = rig().await;
    let mut claimed = claim(&r, None).await;
    let id = claimed.row.id.clone();
    claimed.row.request = serde_json::json!({ "not": "a request" });

    handler(&r, "echo ran", None)
        .await
        .run(claimed)
        .await
        .expect("run");

    let row = finished(&r, &id).await;
    assert_eq!(row.status, "failed");
    assert_eq!(row.error_class.as_deref(), Some("host_crashed"));
    assert!(row.error.is_some());
}
