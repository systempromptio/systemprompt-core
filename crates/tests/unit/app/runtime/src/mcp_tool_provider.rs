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
async fn the_provided_tool_provider_is_backed_by_the_mcp_registry() {
    let ctx = context().await;
    let provider = Arc::clone(
        ctx.kernel()
            .state()
            .find::<Arc<dyn ToolProvider>>()
            .expect("the mcp member provides the tool provider"),
    );
    let health = provider.health_check().await.expect("health check answers");
    let configured: Vec<String> = ctx
        .services_state()
        .view()
        .config()
        .mcp_servers
        .iter()
        .filter(|(_, deployment)| {
            deployment.enabled
                && matches!(
                    deployment.server_type,
                    systemprompt_models::mcp::McpServerType::Internal
                )
        })
        .map(|(name, _)| name.clone())
        .collect();
    assert!(
        !configured.is_empty(),
        "the fixture services configuration declares an enabled internal MCP server"
    );
    for name in configured {
        assert!(
            health.contains_key(&name),
            "the MCP provider reports {name}; the no-tools fallback reports nothing: {health:?}"
        );
    }
}
