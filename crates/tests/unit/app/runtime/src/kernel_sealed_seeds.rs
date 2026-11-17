//! The kernel state seals its secret-bearing and boot-only seeds: the
//! resolver a provide fn and a kernel command read serves them, the resolver
//! a member's command runs against refuses them as missing.

use std::sync::Arc;

use systemprompt_events::EventRouter;
use systemprompt_extension::Resolved;
use systemprompt_identifiers::InstanceId;
use systemprompt_manifest::Config;
use systemprompt_manifest::services::SystemAdmin;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_runtime::{AppContext, Kernel};
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_database_url};
use systemprompt_traits::{AppContext as AppContextTrait, BackgroundTasks, SecretStore};

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

#[tokio::test]
async fn the_kernel_state_seals_the_secret_and_boot_only_seeds() {
    let ctx = context().await;
    let state = ctx.kernel().state();

    assert!(state.is_sealed::<Arc<Config>>());
    assert!(state.is_sealed::<Arc<dyn SecretStore>>());
    assert!(state.is_sealed::<SystemAdmin>());
    assert!(state.is_sealed::<ActiveServicesRoot>());
    assert!(state.is_sealed::<Arc<EventRouter>>());
    assert!(!state.is_sealed::<InstanceId>(), "the instance id is open");
}

#[tokio::test]
async fn a_member_command_resolver_refuses_every_sealed_seed() {
    let ctx = context().await;
    let kernel = command_kernel(&ctx);
    let member = Resolved::__member_command(&kernel, kernel.state());

    assert!(member.get::<Arc<Config>>().is_err());
    assert!(member.get::<Arc<dyn SecretStore>>().is_err());
    assert!(member.get::<SystemAdmin>().is_err());
    assert!(member.get::<ActiveServicesRoot>().is_err());
    assert!(member.get::<Arc<EventRouter>>().is_err());
    assert!(member.optional::<Arc<Config>>().is_none());
    let missing = member.get::<SystemAdmin>().expect_err("sealed");
    assert_eq!(
        missing,
        systemprompt_extension::MissingDependency::of::<SystemAdmin>(),
        "a sealed seed is refused exactly as an absent one"
    );
    assert!(member.get::<InstanceId>().is_ok(), "an open seed is served");
}

#[tokio::test]
async fn the_provide_and_kernel_resolver_serves_every_sealed_seed() {
    let ctx = context().await;
    let kernel = command_kernel(&ctx);
    let provide = kernel.resolved();

    assert!(provide.get::<Arc<Config>>().is_ok());
    assert!(provide.get::<Arc<dyn SecretStore>>().is_ok());
    assert!(provide.get::<SystemAdmin>().is_ok());
    assert!(provide.get::<ActiveServicesRoot>().is_ok());
    assert!(provide.get::<Arc<EventRouter>>().is_ok());
}
