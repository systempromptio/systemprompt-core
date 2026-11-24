//! The inference provider the `ai` member builds holds the `mcp` member's
//! tool provider when `mcp` is admitted, and the typed no-tools fallback when
//! it is not: a tool call through the built inference service reaches the MCP
//! registry in the first composition and refuses with
//! `ToolProviderError::NoTools` in the second.

use std::sync::Arc;

use systemprompt_identifiers::{
    Actor, AgentName, AiToolCallId, ContextId, ExtensionId, McpServerId, SessionId, TraceId, UserId,
};
use systemprompt_models::ai::{DynAiProvider, McpTool, ToolCall};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    TestBootstrap, fixture_admitted, fixture_admitted_without, init_services_bootstrap,
    test_app_context_on_with,
};

const NO_TOOLS: &str = "no tool provider is composed";
const MODEL: &str = "claude-inference-tools-fixture";

fn inference_bootstrap() -> TestBootstrap {
    // SAFETY: set before the process-global bootstrap initialises the secrets
    // it reads; each nextest test runs in its own process.
    unsafe {
        std::env::set_var("SYSTEMPROMPT_CUSTOM_SECRETS", "anthropic_api_key");
        std::env::set_var("anthropic_api_key", "test-provider-key");
    }
    init_services_bootstrap(&format!(
        r#"
credentials:
  anthropic:
    kind: platform
    secret: anthropic_api_key
providers:
  anthropic:
    credential: anthropic
    endpoints:
      api:
        wire: anthropic
        url: http://127.0.0.1:9
models:
  {MODEL}:
    clients:
    - claude
    pricing:
      input_per_million: 3.0
      output_per_million: 15.0
      cache_read_per_million: 0.0
deployments:
  {MODEL}:
  - anthropic/api
gateway:
  default_model: {MODEL}
"#
    ))
}

async fn tool_call_error(ctx: &AppContext) -> String {
    let inference: DynAiProvider = Arc::clone(
        ctx.kernel()
            .state()
            .find::<DynAiProvider>()
            .expect("the ai member provides the inference provider"),
    );
    let server = McpServerId::new("hf2_no_such_server");
    let call = ToolCall {
        ai_tool_call_id: AiToolCallId::new(format!("inference-tools-{}", uuid::Uuid::new_v4())),
        name: "echo".to_owned(),
        arguments: serde_json::json!({}),
    };
    let request = RequestContext::new(
        SessionId::new("inference-tools"),
        TraceId::new("inference-tools"),
        ContextId::generate(),
        AgentName::system(),
        Actor::anonymous(UserId::new("inference-tools")),
    );
    let (_, results) = inference
        .execute_tools(vec![call], &[McpTool::new("echo", server)], &request, None)
        .await;
    assert_eq!(
        results.len(),
        1,
        "the fixture configures inference, so the built service runs the tool call"
    );
    serde_json::to_string(&results[0]).expect("tool result serialises")
}

#[tokio::test]
async fn with_mcp_admitted_the_inference_service_calls_tools_through_the_mcp_provider() {
    let boot = inference_bootstrap();
    let ctx = test_app_context_on_with(&boot, fixture_admitted()).await;
    let error = tool_call_error(&ctx).await;
    assert!(
        !error.contains(NO_TOOLS),
        "the inference service holds the MCP tool provider: {error}"
    );
}

#[tokio::test]
async fn with_mcp_disabled_the_inference_service_answers_no_tools() {
    let boot = inference_bootstrap();
    let ctx = test_app_context_on_with(
        &boot,
        fixture_admitted_without(&[ExtensionId::new("mcp"), ExtensionId::new("mcp-http")]),
    )
    .await;
    let error = tool_call_error(&ctx).await;
    assert!(
        error.contains(NO_TOOLS),
        "without mcp the inference service answers the typed no-tools refusal: {error}"
    );
}
