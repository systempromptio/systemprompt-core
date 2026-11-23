//! The MCP health hop stays with kernel code: the signing keys it is minted
//! from are a sealed seed no member command or runtime context reads, and the
//! read-only observer the `mcp` member provides openly checks health by port
//! reachability only, so member code can never present a health hop to a
//! hosted server.

use std::sync::Arc;

use systemprompt_extension::Resolved;
use systemprompt_mcp::services::McpServiceObserver;
use systemprompt_mcp::services::health_probe::HealthProbe;
use systemprompt_runtime::Kernel;
use systemprompt_security::keys::SigningKeys;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_database_url};
use systemprompt_traits::{AppContext as AppContextTrait, BackgroundTasks};

#[tokio::test]
async fn member_code_holds_no_health_hop() {
    ensure_test_bootstrap();
    let ctx = systemprompt_test_fixtures::test_app_context(&test_database_url()).await;
    let state = ctx.kernel().state();
    assert!(
        state.is_sealed::<SigningKeys>(),
        "the signing keys are sealed"
    );

    let kernel = Kernel::new(
        AppContextTrait::database_handle(&*ctx),
        BackgroundTasks::new(),
        state.clone(),
    );
    let member = Resolved::__member_command(&kernel, kernel.state());
    assert!(
        member.optional::<SigningKeys>().is_none(),
        "a member command cannot mint a health hop"
    );
    assert!(kernel.runtime_state().find::<SigningKeys>().is_none());

    let observer = member
        .optional::<Arc<McpServiceObserver>>()
        .expect("member commands read the open observer");
    assert!(
        matches!(observer.probe(), HealthProbe::Listening),
        "the open observer checks port reachability only"
    );
}
