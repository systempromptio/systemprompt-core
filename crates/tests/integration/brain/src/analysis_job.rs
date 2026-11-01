use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_brain::jobs::BrainAnalysisJob;
use systemprompt_database::DbPool;
use systemprompt_extension::brain::{AnalysisWindow, BrainAnalysis, BrainAnalysisError};
use systemprompt_extension::{ExtensionContext, register_brain_analysis};
use systemprompt_identifiers::{Actor, UserId};
use systemprompt_manifest::profile::BrainProfile;
use systemprompt_models::brain::{ProposalDraft, ProposalKind};
use systemprompt_test_fixtures::{DisposableDb, test_app_context};
use systemprompt_traits::{Dependencies, Job, JobContext, JobResult, RepositoryError};

struct DraftingAnalysis;

#[async_trait]
impl BrainAnalysis for DraftingAnalysis {
    fn name(&self) -> &'static str {
        "test-drafting"
    }

    async fn analyse(
        &self,
        window: AnalysisWindow,
    ) -> Result<Vec<ProposalDraft>, BrainAnalysisError> {
        assert!(window.since < window.until);
        let draft = ProposalDraft {
            analysis: "test-drafting".to_owned(),
            kind: ProposalKind::FactActions,
            summary: "retire stale pricing".to_owned(),
            body: serde_json::json!({"actions": ["retire"]}),
        };
        Ok(vec![draft.clone(), draft])
    }
}

struct FailingAnalysis;

#[async_trait]
impl BrainAnalysis for FailingAnalysis {
    fn name(&self) -> &'static str {
        "test-failing"
    }

    async fn analyse(&self, _: AnalysisWindow) -> Result<Vec<ProposalDraft>, BrainAnalysisError> {
        Err(BrainAnalysisError::Repository(
            RepositoryError::invalid_argument("window", "refused"),
        ))
    }
}

register_brain_analysis!(
    factory = |_| Arc::new(DraftingAnalysis) as Arc<dyn BrainAnalysis>,
    priority = 0
);
register_brain_analysis!(
    factory = |_| Arc::new(FailingAnalysis) as Arc<dyn BrainAnalysis>,
    priority = 1
);

fn profile(enabled: bool) -> Arc<BrainProfile> {
    Arc::new(BrainProfile {
        enabled,
        ..BrainProfile::default()
    })
}

async fn execute(deps: Dependencies) -> JobResult {
    let ctx = JobContext::new(Actor::user(UserId::generate()), deps);
    BrainAnalysisJob.execute(&ctx).await.expect("job runs")
}

async fn proposals(pool: &DbPool) -> Vec<(String, String)> {
    sqlx::query_as::<_, (String, String)>(
        "SELECT analysis, status FROM brain_proposals ORDER BY created_at",
    )
    .fetch_all(pool.write_pool().as_ref())
    .await
    .expect("proposals")
}

#[test]
fn the_job_declares_a_daily_cluster_schedule() {
    assert_eq!(BrainAnalysisJob.name(), "brain_analysis");
    assert_eq!(BrainAnalysisJob.schedule(), "0 15 3 * * *");
    assert_eq!(
        BrainAnalysisJob.scope(),
        systemprompt_traits::JobScope::Cluster
    );
    assert!(BrainAnalysisJob.description().contains("proposals"));
}

#[tokio::test]
async fn a_disabled_brain_runs_no_analysis() {
    let db = DisposableDb::with_schema("brain_analysis_off").await;
    let pool = db.test_pool().await;

    let result = execute(
        Dependencies::new()
            .with(profile(false))
            .with(Arc::clone(&pool)),
    )
    .await;

    assert!(result.success, "{result:?}");
    assert_eq!(result.items_processed, Some(0));
    assert!(proposals(&pool).await.is_empty());
    pool.write_pool().close().await;
    db.drop_now().await;
}

#[tokio::test]
async fn core_analyses_over_an_empty_history_store_nothing_and_succeed() {
    let db = DisposableDb::with_schema("brain_analysis_core").await;
    let pool = db.test_pool().await;

    let result = execute(
        Dependencies::new()
            .with(profile(true))
            .with(Arc::clone(&pool)),
    )
    .await;

    assert!(result.success, "{result:?}");
    assert_eq!(result.items_processed, Some(0));
    assert_eq!(result.items_failed, Some(0));
    assert!(proposals(&pool).await.is_empty());
    pool.write_pool().close().await;
    db.drop_now().await;
}

#[tokio::test]
async fn extension_analyses_store_one_fresh_proposal_and_count_failures() {
    let db = DisposableDb::with_schema("brain_analysis_ext").await;
    let pool = db.test_pool().await;
    let app = test_app_context(&pool, db.url());
    let extensions: Arc<dyn ExtensionContext> = app;
    let deps = || {
        Dependencies::new()
            .with(profile(true))
            .with(Arc::clone(&pool))
            .with(Arc::clone(&extensions))
    };

    let first = execute(deps()).await;
    assert!(!first.success);
    assert_eq!(first.message.as_deref(), Some("1 brain analyses failed"));
    assert_eq!(first.items_processed, Some(1));
    assert_eq!(first.items_failed, Some(1));
    assert_eq!(
        proposals(&pool).await,
        vec![("test-drafting".to_owned(), "open".to_owned())]
    );

    let second = execute(deps()).await;
    assert_eq!(
        second.items_processed,
        Some(0),
        "an open proposal is not redrafted"
    );
    assert_eq!(proposals(&pool).await.len(), 1);
    pool.write_pool().close().await;
    db.drop_now().await;
}
