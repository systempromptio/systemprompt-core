//! The ai, gateway, brain and kernel credential seams keep their seal: each
//! value is in the API kernel state for kernel code, and neither a member
//! command resolver nor the runtime state every job, runner and plane context
//! is built from can obtain it.

use std::sync::Arc;

use systemprompt_ai::repository::AiRepositories;
use systemprompt_extension::Resolved;
use systemprompt_extension::brain::DynBrainConsumerStart;
use systemprompt_gateway::GatewayEngines;
use systemprompt_models::ai::DynAiProvider;
use systemprompt_runtime::{AppContext, Kernel};
use systemprompt_security::credential::TokenCache;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_database_url};
use systemprompt_traits::{
    AppContext as AppContextTrait, BackgroundTasks, DynAiSessionProvider, DynCatalogMigrationPort,
    DynSafetyFindingsReader, SecretLookup,
};

async fn context() -> Arc<AppContext> {
    ensure_test_bootstrap();
    systemprompt_test_fixtures::test_app_context(&test_database_url()).await
}

fn command_kernel(ctx: &AppContext) -> Kernel {
    Kernel::new(
        AppContextTrait::database_handle(ctx),
        BackgroundTasks::new(),
        ctx.kernel().state().clone(),
    )
}

fn assert_sealed<T: Clone + Send + Sync + 'static>(ctx: &AppContext) {
    let name = std::any::type_name::<T>();
    let state = ctx.kernel().state();
    assert!(state.is_sealed::<T>(), "{name} is sealed");
    assert!(state.find::<T>().is_some(), "kernel code reads {name}");
    let kernel = command_kernel(ctx);
    let member = Resolved::__member_command(&kernel, kernel.state());
    assert!(
        member.optional::<T>().is_none(),
        "a member command cannot read {name}"
    );
    assert!(
        kernel.runtime_state().find::<T>().is_none(),
        "no job, runner or plane context reads {name}"
    );
}

#[tokio::test]
async fn the_connector_tokens_and_the_provider_key_lookup_are_sealed_seeds() {
    let ctx = context().await;
    assert_sealed::<Arc<TokenCache>>(&ctx);
    assert_sealed::<Arc<dyn SecretLookup>>(&ctx);
}

#[tokio::test]
async fn the_ai_values_are_sealed() {
    let ctx = context().await;
    assert_sealed::<Arc<AiRepositories>>(&ctx);
    assert_sealed::<DynSafetyFindingsReader>(&ctx);
    assert_sealed::<DynCatalogMigrationPort>(&ctx);
    assert_sealed::<DynAiProvider>(&ctx);
}

#[tokio::test]
async fn the_ai_session_provider_the_users_member_provides_is_sealed() {
    let ctx = context().await;
    assert_sealed::<DynAiSessionProvider>(&ctx);
}

#[tokio::test]
async fn the_gateway_engines_and_the_brain_consumer_start_are_sealed() {
    let ctx = context().await;
    assert_sealed::<Arc<GatewayEngines>>(&ctx);
    assert_sealed::<DynBrainConsumerStart>(&ctx);
}
