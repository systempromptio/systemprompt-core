// Exercises the McpToolHandler trait (default description, input/output schema)
// and the full McpToolExecutor::execute path (start/complete tracking, input
// parse, response build) via a real in-test handler. DB-backed parts run
// behind the fixture skip-guard; the schema/trait parts are pure.

use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::model::CallToolRequestParams;
use schemars::JsonSchema;
use serde::Deserialize;
use systemprompt_identifiers::{AgentName, ContextId, McpExecutionId, SessionId, TraceId, UserId};
use systemprompt_mcp::repository::{McpArtifactRepository, ToolUsageRepository};
use systemprompt_mcp::{McpToolExecutor, McpToolHandler};
use systemprompt_models::RequestContext;
use systemprompt_models::artifacts::TextArtifact;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

#[derive(Debug, Deserialize, JsonSchema)]
struct EchoInput {
    message: String,
}

// The handler Output must implement McpOutputSchema; TextArtifact already does,
// so we use it directly rather than wiring a bespoke output type.

struct EchoHandler;

impl McpToolHandler for EchoHandler {
    type Input = EchoInput;
    type Output = TextArtifact;

    fn tool_name(&self) -> &'static str {
        "echo"
    }

    fn description(&self) -> &'static str {
        "Echoes its input"
    }

    async fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(Self::Output, String), McpError> {
        Ok((
            TextArtifact::new(format!("echo: {}", input.message)),
            "echoed input".to_owned(),
        ))
    }
}

struct FailingHandler;

impl McpToolHandler for FailingHandler {
    type Input = EchoInput;
    type Output = TextArtifact;

    fn tool_name(&self) -> &'static str {
        "always-fails"
    }

    // No description override -> exercises the default ("").

    async fn handle(
        &self,
        _input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(Self::Output, String), McpError> {
        Err(McpError::internal_error("handler boom", None))
    }
}

fn test_ctx() -> RequestContext {
    RequestContext::new(
        SessionId::new("s-tool"),
        TraceId::new("t-tool"),
        ContextId::generate(),
        AgentName::new("agent-tool"),
    )
    .with_actor(systemprompt_identifiers::Actor::user(UserId::new(
        "user-tool",
    )))
}

fn echo_request(message: &str) -> CallToolRequestParams {
    let mut map = serde_json::Map::new();
    map.insert(
        "message".to_owned(),
        serde_json::Value::String(message.to_owned()),
    );
    CallToolRequestParams::new("echo".to_owned()).with_arguments(map)
}

// ---- pure trait-surface tests (no DB) ----

#[test]
fn handler_tool_name() {
    assert_eq!(EchoHandler.tool_name(), "echo");
}

#[test]
fn handler_description_override() {
    assert_eq!(EchoHandler.description(), "Echoes its input");
}

#[test]
fn handler_description_default_empty() {
    assert_eq!(FailingHandler.description(), "");
}

#[test]
fn handler_input_schema_is_object_with_message() {
    let schema = EchoHandler.input_schema();
    assert!(schema.is_object());
    let s = serde_json::to_string(&schema).unwrap();
    assert!(s.contains("message"));
}

#[test]
fn handler_output_schema_tags_artifact_type() {
    let schema = EchoHandler.output_schema();
    assert!(schema.is_object());
    let s = serde_json::to_string(&schema).unwrap();
    // TextArtifact's validated_schema injects the x-artifact-type marker.
    assert!(s.contains("x-artifact-type"));
    assert!(s.contains("text"));
}

// ---- full execute round-trips (DB-backed; skip-guarded) ----

#[tokio::test]
async fn execute_success_records_and_returns_result() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };
    let tool_repo = Arc::new(ToolUsageRepository::new(&db).unwrap());
    let art_repo = Arc::new(McpArtifactRepository::new(&db).unwrap());
    let exec = McpToolExecutor::new(tool_repo, art_repo, "srv-echo");

    let ctx = test_ctx();
    let request = echo_request("hi there");
    let result = exec.execute(&EchoHandler, &request, &ctx).await;

    let call_result = result.expect("execute should succeed");
    assert_ne!(call_result.is_error, Some(true));
    let serialized = serde_json::to_string(&call_result).unwrap();
    assert!(serialized.contains("echo: hi there") || serialized.contains("echoed input"));
}

#[tokio::test]
async fn execute_handler_error_propagates() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };
    let tool_repo = Arc::new(ToolUsageRepository::new(&db).unwrap());
    let art_repo = Arc::new(McpArtifactRepository::new(&db).unwrap());
    let exec = McpToolExecutor::new(tool_repo, art_repo, "srv-fail");

    let ctx = test_ctx();
    let mut map = serde_json::Map::new();
    map.insert(
        "message".to_owned(),
        serde_json::Value::String("x".to_owned()),
    );
    let request = CallToolRequestParams::new("always-fails".to_owned()).with_arguments(map);

    let result = exec.execute(&FailingHandler, &request, &ctx).await;
    let err = result.expect_err("handler returns Err");
    assert!(err.message.contains("boom"));
}

#[tokio::test]
async fn execute_input_parse_error_returns_invalid_params() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    let Ok(db) = fixture_db_pool(&url).await else {
        return;
    };
    let tool_repo = Arc::new(ToolUsageRepository::new(&db).unwrap());
    let art_repo = Arc::new(McpArtifactRepository::new(&db).unwrap());
    let exec = McpToolExecutor::new(tool_repo, art_repo, "srv-bad");

    let ctx = test_ctx();
    // Missing required "message" field -> parse_input fails.
    let request = CallToolRequestParams::new("echo".to_owned());
    let result = exec.execute(&EchoHandler, &request, &ctx).await;
    let err = result.expect_err("parse should fail");
    assert!(err.message.to_lowercase().contains("invalid"));
}
