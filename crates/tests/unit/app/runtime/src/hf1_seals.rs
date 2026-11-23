//! The late-bound cells of the kernel state hold only their read halves; the
//! messaging dispatcher, the agent dispatch, the session store and the
//! tool-call intent claims are sealed; and a route set is refused every
//! host-sealed seed whatever its owner's standing.

use std::sync::Arc;

use systemprompt_extension::{LateRead, Resolved, StandingTag};
use systemprompt_identifiers::{ExtensionId, InstanceId};
use systemprompt_loader::subprocess::Spawner;
use systemprompt_models::a2a::dispatch::AgentDispatchCell;
use systemprompt_provider_contracts::DependencyProvider;
use systemprompt_runtime::{AppContext, Kernel};
use systemprompt_security::keys::SigningKeys;
use systemprompt_test_fixtures::{
    Probe, ensure_test_bootstrap, fixture_admitted_of, fixture_app_context_with_extensions,
    fixture_config, fixture_core_members, test_database_url,
};
use systemprompt_traits::{
    AppContext as AppContextTrait, BackgroundTasks, DynInventoryPublisher, DynSessionStore,
    DynSessionTelemetry, DynToolCallIntentClaims, MessagingDispatcher, SecretStore,
};

type MessagingCell = LateRead<dyn MessagingDispatcher>;

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
async fn no_bind_half_of_a_late_bound_cell_is_in_the_kernel_state() {
    let ctx = context().await;
    let rendered = format!("{:?}", ctx.kernel().state());
    assert!(
        !rendered.contains("late_bound::LateBound<"),
        "a bindable cell is in the kernel state: {rendered}"
    );
    assert!(
        !rendered.contains("late_bound::LateBinder<"),
        "a bind half is in the kernel state: {rendered}"
    );
}

#[tokio::test]
async fn the_messaging_dispatcher_is_sealed_from_member_code() {
    let ctx = context().await;
    let state = ctx.kernel().state();
    assert!(state.is_sealed::<MessagingCell>());
    assert!(
        state.find::<MessagingCell>().is_some(),
        "kernel code reads it"
    );
    let kernel = command_kernel(&ctx);
    let member = Resolved::__member_command(&kernel, kernel.state());
    assert!(member.optional::<MessagingCell>().is_none());
    assert!(kernel.runtime_state().find::<MessagingCell>().is_none());
    let forged = ExtensionId::new("forged-member");
    assert!(
        ctx.kernel()
            .route_scoped(&forged, StandingTag::Install, &[])
            .get::<MessagingCell>()
            .is_err(),
        "an install member's route state is refused it"
    );
}

#[tokio::test]
async fn a_member_cannot_provide_the_messaging_dispatcher_cell() {
    ensure_test_bootstrap();
    let mut members = fixture_core_members();
    members.push(Box::new(Probe::new("forged-dispatcher", |c| {
        c.provide(
            |_: &Resolved<'_>| -> Result<MessagingCell, systemprompt_extension::ProvideError> {
                Ok(MessagingCell::default())
            },
        );
    })));
    let refused = fixture_app_context_with_extensions(
        fixture_config(&test_database_url()),
        fixture_admitted_of(members),
    )
    .await;
    assert!(
        refused.is_err(),
        "a member providing the kernel's dispatcher cell fails the boot"
    );
}

#[tokio::test]
async fn a_route_set_of_either_standing_is_refused_every_host_sealed_seed() {
    let ctx = context().await;
    let kernel = ctx.kernel();
    let install = ExtensionId::new("an-install-member");
    let resolved = kernel.route_scoped(&install, StandingTag::Install, &[]);
    assert!(resolved.get::<Arc<Spawner>>().is_err());
    assert!(resolved.get::<SigningKeys>().is_err());
    assert!(resolved.get::<AgentDispatchCell>().is_err());
    assert!(resolved.get::<Arc<dyn SecretStore>>().is_err());
    assert!(
        resolved.get::<InstanceId>().is_ok(),
        "an open seed is served"
    );

    let core = ExtensionId::new("content");
    let resolved = kernel.route_scoped(&core, StandingTag::Core, &[]);
    assert!(resolved.get::<Arc<Spawner>>().is_err());
    assert!(resolved.get::<SigningKeys>().is_err());
    assert!(resolved.get::<AgentDispatchCell>().is_err());
    assert!(resolved.get::<Arc<dyn SecretStore>>().is_err());
    assert!(
        resolved.get::<InstanceId>().is_ok(),
        "an open seed is served"
    );
}

#[tokio::test]
async fn a_members_sealed_provide_is_refused_to_another_members_route_set_of_either_standing() {
    let ctx = context().await;
    let kernel = ctx.kernel();
    let other = ExtensionId::new("content");
    for standing in [StandingTag::Core, StandingTag::Install] {
        assert!(
            kernel
                .route_scoped(&other, standing, &[])
                .get::<DynInventoryPublisher>()
                .is_err(),
            "{standing:?}"
        );
    }
}

#[tokio::test]
async fn the_session_store_is_sealed_and_the_telemetry_view_is_open() {
    let ctx = context().await;
    let state = ctx.kernel().state();
    assert!(state.is_sealed::<DynSessionStore>());
    assert!(!state.is_sealed::<DynSessionTelemetry>());
    assert!(matches!(
        state.provider_of::<DynSessionStore>(),
        Some(DependencyProvider::Extension(id)) if id.as_str() == "users"
    ));
    let kernel = command_kernel(&ctx);
    let member = Resolved::__member_command(&kernel, kernel.state());
    assert!(member.optional::<DynSessionStore>().is_none());
    assert!(member.optional::<DynSessionTelemetry>().is_some());
    let runtime = kernel.runtime_state();
    assert!(runtime.find::<DynSessionStore>().is_none());
    assert!(runtime.find::<DynSessionTelemetry>().is_some());
}

#[tokio::test]
async fn the_tool_call_intent_claims_are_the_ai_members_sealed_provide() {
    let ctx = context().await;
    let state = ctx.kernel().state();
    assert!(state.is_sealed::<DynToolCallIntentClaims>());
    assert!(matches!(
        state.provider_of::<DynToolCallIntentClaims>(),
        Some(DependencyProvider::Extension(id)) if id.as_str() == "ai"
    ));
    let kernel = command_kernel(&ctx);
    let member = Resolved::__member_command(&kernel, kernel.state());
    assert!(member.optional::<DynToolCallIntentClaims>().is_none());
    assert!(
        kernel
            .runtime_state()
            .find::<DynToolCallIntentClaims>()
            .is_none()
    );
}
