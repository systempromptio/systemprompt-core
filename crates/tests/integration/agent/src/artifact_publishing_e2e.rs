use anyhow::Result;
use systemprompt_agent::models::a2a::{Artifact, Part, TextPart};
use systemprompt_agent::services::artifact_publishing::{
    ArtifactPublishingService, PublishFromMcpParams,
};
use systemprompt_identifiers::{Actor, AgentName, ArtifactId, SessionId, TraceId};
use systemprompt_models::a2a::{ArtifactMetadata, TaskState};
use systemprompt_models::execution::CallSource;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_test_fixtures::ensure_test_bootstrap;

use crate::common::Fixture;

fn make_artifact(
    ctx_id: &systemprompt_identifiers::ContextId,
    task_id: &systemprompt_identifiers::TaskId,
) -> Artifact {
    Artifact {
        id: ArtifactId::generate(),
        title: Some("Hello".to_string()),
        description: Some("Sample artifact".to_string()),
        parts: vec![Part::Text(TextPart {
            text: "body".to_string(),
        })],
        extensions: vec![],
        metadata: ArtifactMetadata::new("text".to_string(), ctx_id.clone(), task_id.clone()),
    }
}

#[tokio::test]
async fn artifact_publishing_publish_from_a2a_succeeds() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Working).await?;
    let svc = ArtifactPublishingService::new(&fx.db)?;

    let artifact = make_artifact(&fx.context_id, &task_id);
    svc.publish_from_a2a(&artifact, &task_id, &fx.context_id)
        .await?;

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn artifact_publishing_publish_from_mcp_agentic_skips_messages() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Working).await?;
    let svc = ArtifactPublishingService::new(&fx.db)?;

    let artifact = make_artifact(&fx.context_id, &task_id);
    let mut ctx = RequestContext::new(
        SessionId::new("art-pub-session"),
        TraceId::new("art-pub-trace"),
        fx.context_id.clone(),
        AgentName::new("test-agent"),
    );
    ctx.auth.actor = Actor::user(fx.user_id.clone());

    let args = serde_json::json!({"k": "v"});
    svc.publish_from_mcp(PublishFromMcpParams {
        artifact: &artifact,
        task_id: &task_id,
        context_id: &fx.context_id,
        tool_name: "my_tool",
        tool_args: &args,
        request_context: &ctx,
        call_source: CallSource::Agentic,
    })
    .await?;

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn artifact_publishing_publish_from_mcp_direct_creates_messages() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Working).await?;
    let svc = ArtifactPublishingService::new(&fx.db)?;

    let artifact = make_artifact(&fx.context_id, &task_id);
    let mut ctx = RequestContext::new(
        SessionId::new("art-pub-direct"),
        TraceId::new("art-pub-direct-trace"),
        fx.context_id.clone(),
        AgentName::new("test-agent"),
    );
    ctx.auth.actor = Actor::user(fx.user_id.clone());

    let args = serde_json::json!({});
    svc.publish_from_mcp(PublishFromMcpParams {
        artifact: &artifact,
        task_id: &task_id,
        context_id: &fx.context_id,
        tool_name: "direct_tool",
        tool_args: &args,
        request_context: &ctx,
        call_source: CallSource::Direct,
    })
    .await?;

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn artifact_publishing_debug_impl() -> Result<()> {
    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let svc = ArtifactPublishingService::new(&fx.db)?;
    let dbg = format!("{:?}", svc);
    assert!(dbg.contains("ArtifactPublishingService"));
    fx.cleanup().await?;
    Ok(())
}
