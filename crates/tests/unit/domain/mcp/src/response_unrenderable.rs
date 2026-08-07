// `McpResponseBuilder::build` when the artifact type has no registered
// renderer. Rendering is presentational, so the failure must degrade to a
// result without the embedded ui:// resource rather than failing the tool call
// — the branch the renderable-artifact tests in `response_ui` never take.

use schemars::JsonSchema;
use serde::Serialize;
use systemprompt_identifiers::{AgentName, ContextId, McpExecutionId, SessionId, TraceId, UserId};
use systemprompt_mcp::repository::McpArtifactRepository;
use systemprompt_mcp::{
    ClientProfile, McpOutputSchema, McpResponseBuilder, ToolIdentity, UI_RESOURCE_URI_META_KEY,
};
use systemprompt_models::RequestContext;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

#[derive(Serialize, JsonSchema)]
struct UnrenderableArtifact {
    detail: String,
}

impl McpOutputSchema for UnrenderableArtifact {
    fn artifact_type() -> &'static str {
        "no_renderer_is_registered_for_this"
    }
}

fn ui_client() -> ClientProfile {
    ClientProfile {
        protocol_version: Some(rmcp::model::ProtocolVersion::V_2025_06_18),
        client_name: Some("test-host".to_owned()),
        extensions: [systemprompt_models::mcp::EXTENSION_ID.to_owned()].into(),
    }
}

fn test_ctx() -> RequestContext {
    RequestContext::new(
        SessionId::new("s-unrender"),
        TraceId::new("t-unrender"),
        ContextId::generate(),
        AgentName::new("agent-unrender"),
    )
    .with_actor(systemprompt_identifiers::Actor::user(UserId::new(
        "user-unrender",
    )))
}

#[tokio::test]
async fn an_artifact_with_no_renderer_still_produces_a_successful_result() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };
    let repo = McpArtifactRepository::new(&db).expect("artifact repo");
    let ctx = test_ctx();
    let exec_id = McpExecutionId::generate();

    let result = McpResponseBuilder::new(
        UnrenderableArtifact {
            detail: "payload the UI layer cannot draw".to_owned(),
        },
        ToolIdentity::new("unrenderable-server", "unrenderable-tool"),
        &ctx,
        &exec_id,
        &ui_client(),
    )
    .build(
        "summary for the model",
        &repo,
        UnrenderableArtifact::artifact_type(),
        Some("Unrenderable".to_owned()),
    )
    .await
    .expect("a renderer failure must not fail the tool call");

    assert_ne!(
        result.is_error,
        Some(true),
        "the call succeeded despite the rendering failure"
    );
    assert!(
        result.structured_content.is_some(),
        "programmatic consumers still get the artifact"
    );

    let serialized = serde_json::to_string(&result).expect("serialize");
    assert!(
        serialized.contains("summary for the model"),
        "the text summary for the model survives: {serialized}"
    );
    assert!(
        !serialized.contains("text/html"),
        "no HTML resource is embedded when rendering failed: {serialized}"
    );
}

#[tokio::test]
async fn the_result_meta_names_the_ui_resource_even_when_rendering_failed() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };
    let repo = McpArtifactRepository::new(&db).expect("artifact repo");
    let ctx = test_ctx();
    let exec_id = McpExecutionId::generate();

    let result = McpResponseBuilder::new(
        UnrenderableArtifact {
            detail: "another payload".to_owned(),
        },
        ToolIdentity::new("unrenderable-server", "unrenderable-tool"),
        &ctx,
        &exec_id,
        &ui_client(),
    )
    .build(
        "summary",
        &repo,
        UnrenderableArtifact::artifact_type(),
        None,
    )
    .await
    .expect("build succeeds");

    let uri = result
        .meta
        .as_ref()
        .and_then(|meta| meta.0.get(UI_RESOURCE_URI_META_KEY))
        .and_then(serde_json::Value::as_str)
        .expect("the ui resource uri is always advertised on _meta");

    assert!(
        uri.starts_with("ui://"),
        "hosts that prefer resources/read are given a ui:// uri: {uri}"
    );
    assert!(
        uri.contains("unrenderable-tool"),
        "the uri is scoped to the server that produced it: {uri}"
    );
}
