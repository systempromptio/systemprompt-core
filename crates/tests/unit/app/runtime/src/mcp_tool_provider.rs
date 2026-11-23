//! The tool provider the ai member's inference path reads is the `mcp`
//! member's: provided sealed before governance (so the ai member's
//! after-governance provide sees it in any composition order), readable by
//! kernel code and provide fns only, and backed by the MCP registry rather
//! than the no-tools fallback.

use std::sync::Arc;

use systemprompt_extension::Resolved;
use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::DependencyProvider;
use systemprompt_runtime::{AppContext, Kernel};
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_database_url};
use systemprompt_traits::{AppContext as AppContextTrait, BackgroundTasks, ToolProvider};

async fn context() -> Arc<AppContext> {
    ensure_test_bootstrap();
    systemprompt_test_fixtures::test_app_context(&test_database_url()).await
}

#[tokio::test]
async fn the_mcp_member_provides_the_tool_provider_sealed() {
    let ctx = context().await;
    let state = ctx.kernel().state();
    assert!(state.is_sealed::<Arc<dyn ToolProvider>>());
    assert!(matches!(
        state.provider_of::<Arc<dyn ToolProvider>>(),
        Some(DependencyProvider::Extension(owner)) if *owner == ExtensionId::new("mcp")
    ));
    let kernel = Kernel::new(
        AppContextTrait::database_handle(&*ctx),
        BackgroundTasks::new(),
        state.clone(),
    );
    let member = Resolved::__member_command(&kernel, kernel.state());
    assert!(member.optional::<Arc<dyn ToolProvider>>().is_none());
    assert!(kernel.runtime_state().find::<Arc<dyn ToolProvider>>().is_none());
}

#[tokio::test]
async fn the_provided_tool_provider_is_the_mcp_registry_not_the_no_tools_fallback() {
    let ctx = context().await;
    let provider = Arc::clone(
        ctx.kernel()
            .state()
            .find::<Arc<dyn ToolProvider>>()
            .expect("the mcp member provides the tool provider"),
    );
    let request = systemprompt_provider_contracts::ToolCallRequest {
        tool_call_id: systemprompt_identifiers::AiToolCallId::new("call-1"),
        name: "echo".to_owned(),
        arguments: serde_json::json!({}),
    };
    let error = provider
        .call_tool(
            &request,
            &systemprompt_identifiers::McpServerId::new("no_such_server"),
            &systemprompt_provider_contracts::ToolContext::new(systemprompt_identifiers::Actor::anonymous(
                systemprompt_identifiers::UserId::new("tool-provider-probe"),
            )),
        )
        .await
        .expect_err("an undeclared server is refused");
    assert!(
        !error.to_string().contains("NoopToolProvider"),
        "the inference path reads the MCP provider, not the no-tools fallback: {error}"
    );
}
