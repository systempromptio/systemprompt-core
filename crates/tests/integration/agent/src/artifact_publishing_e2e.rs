use std::sync::Arc;

use anyhow::Result;
use systemprompt_agent::models::a2a::{Artifact, Part};
use systemprompt_agent::repository::execution::ExecutionStepRepository;
use systemprompt_agent::services::SkillService;
use systemprompt_agent::services::artifact_publishing::ArtifactPublishingService;
use systemprompt_identifiers::ArtifactId;
use systemprompt_models::a2a::{ArtifactMetadata, TaskState};
use systemprompt_test_fixtures::ensure_test_bootstrap;

use crate::common::Fixture;

fn publishing_service(fx: &Fixture) -> Result<ArtifactPublishingService> {
    let repositories = systemprompt_test_fixtures::a2a_repositories(&fx.db);
    let steps = Arc::new(ExecutionStepRepository::new(&fx.db));
    let skills = Arc::new(SkillService::new(
        systemprompt_test_fixtures::test_skills_root(),
        systemprompt_test_fixtures::not_managed_skills(),
        steps,
        systemprompt_test_mocks::recording_webhooks(),
    ));
    Ok(ArtifactPublishingService::new(&repositories, skills))
}

fn make_artifact(
    ctx_id: &systemprompt_identifiers::ContextId,
    task_id: &systemprompt_identifiers::TaskId,
) -> Artifact {
    Artifact {
        id: ArtifactId::generate(),
        title: Some("Hello".to_string()),
        description: Some("Sample artifact".to_string()),
        parts: vec![Part::text("body".to_string())],
        extensions: vec![],
        metadata: ArtifactMetadata::new("text".to_string(), ctx_id.clone(), task_id.clone()),
    }
}

#[tokio::test]
async fn artifact_publishing_publish_from_a2a_succeeds() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let runtime = fx.insert_runtime_task(TaskState::Working).await?;
    let svc = publishing_service(&fx)?;

    let artifact = make_artifact(&fx.context_id, runtime.task_id());
    svc.publish_from_a2a(&artifact, &runtime, &fx.user_id)
        .await?;

    fx.cleanup().await?;
    Ok(())
}
