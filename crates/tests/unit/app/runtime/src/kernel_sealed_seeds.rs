//! The kernel state seals its secret-bearing and boot-only seeds: the
//! resolver a provide fn and a kernel command read serves them, the resolver
//! a member's command runs against refuses them as missing. A member value
//! provided sealed (one that acts with standing authority) is sealed the same
//! way, and a route set's resolver serves it only to its provider. A seal
//! covers the capability, not one type name: no open value of the runtime
//! state is a member-sealed value's implementor, wraps one, or shares its
//! allocation, and no open type names a host- or member-sealed capability, a
//! capability a sealed value is a subset of (the session store over the AI
//! session provider), or the `AppContext`. That pin matches types by name, so
//! a member newtype laundering a sealed capability carries no marker it can
//! see; the laundering source is closed instead: an install member's provide
//! fn is refused every host-sealed capability, so the newtype is never built.

use std::any::{TypeId, type_name};
use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;

use std::any::Any;

use systemprompt_config::DynChildSecretEnv;
use systemprompt_events::EventRouter;
use systemprompt_extension::{LateRead, ProvideError, Resolved, StandingTag};
use systemprompt_identifiers::{ExtensionId, InstanceId};
use systemprompt_loader::subprocess::Spawner;
use systemprompt_manifest::Config;
use systemprompt_manifest::services::SystemAdmin;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_marketplace::inventory::InventoryRefresher;
use systemprompt_marketplace::managed::{OrganizationSkillResolver, OwnerSkillView};
use systemprompt_models::a2a::dispatch::AgentDispatchCell;
use systemprompt_runtime::{AppContext, Kernel, KernelError};
use systemprompt_security::keys::SigningKeys;
use systemprompt_test_fixtures::{
    ComposedParts, Probe, ensure_test_bootstrap, fixture_admitted_of, probe_admitted,
    test_database_url,
};
use systemprompt_traits::{
    AppContext as AppContextTrait, BackgroundTasks, DynInventoryPublisher, DynManagedSkillResolver,
    MessagingDispatcher, SecretStore,
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

#[tokio::test]
async fn the_kernel_state_seals_the_secret_and_boot_only_seeds() {
    let ctx = context().await;
    let state = ctx.kernel().state();

    assert!(state.is_sealed::<Arc<Config>>());
    assert!(state.is_sealed::<Arc<dyn SecretStore>>());
    assert!(state.is_sealed::<SystemAdmin>());
    assert!(state.is_sealed::<ActiveServicesRoot>());
    assert!(state.is_sealed::<Arc<EventRouter>>());
    assert!(
        state.is_sealed::<SigningKeys>(),
        "the JWT signing keys are sealed"
    );
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
    assert!(member.get::<SigningKeys>().is_err());
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
    assert!(provide.get::<SigningKeys>().is_ok());
}

#[tokio::test]
async fn the_runtime_state_withholds_every_sealed_seed() {
    let ctx = context().await;
    let state = ctx.kernel().state();
    let runtime = ctx.kernel().runtime_state();

    let reachable: Vec<&str> = state
        .sealed_types()
        .filter(|(id, _)| runtime.contains_type(*id))
        .map(|(_, name)| name)
        .collect();
    assert!(
        reachable.is_empty(),
        "sealed seeds in the runtime state: {reachable:?}"
    );
    assert!(runtime.get::<SigningKeys>().is_err());
    assert!(runtime.get::<InstanceId>().is_ok(), "an open seed stays");
}

#[tokio::test]
async fn a_sealed_provide_is_withheld_from_every_member_runtime_context() {
    let ctx = context().await;
    let kernel: &Kernel = ctx.kernel();
    let state = kernel.state();

    assert!(state.is_sealed::<DynInventoryPublisher>());
    assert!(state.is_sealed::<DynManagedSkillResolver>());
    assert!(
        state.get::<DynInventoryPublisher>().is_ok(),
        "the kernel reads it"
    );
    assert!(kernel.resolved().get::<DynInventoryPublisher>().is_ok());

    let runtime = kernel.runtime_state();
    assert!(runtime.get::<DynInventoryPublisher>().is_err());
    assert!(runtime.get::<DynManagedSkillResolver>().is_err());

    let member = Resolved::__member_command(kernel, state);
    assert!(member.get::<DynInventoryPublisher>().is_err());

    let other = ExtensionId::new("content");
    assert!(
        kernel
            .route_scoped(&other, StandingTag::Core, &[])
            .get::<DynInventoryPublisher>()
            .is_err(),
        "another member's route state is refused it"
    );
    let provider = ExtensionId::new("managed_resources");
    assert!(
        kernel
            .route_scoped(&provider, StandingTag::Core, &[])
            .get::<DynInventoryPublisher>()
            .is_ok(),
        "its provider's own route state reads it"
    );
}

// A capability that wraps or exceeds a sealed one: the session store does
// everything the sealed AI session provider does and more, and the messaging
// dispatcher mints with the signing keys and sends through the sealed agent
// dispatch. Neither may be open.
const SUPERSET_OF_A_SEALED_CAPABILITY: &[&str] = &[
    "dyn systemprompt_traits::session_store::SessionStore",
    "dyn systemprompt_traits::messaging::MessagingDispatcher",
];

const CELL_WRAPPERS: &[&str] = &[
    "alloc::sync::Arc<",
    "systemprompt_models::late_bound::LateRead<",
    "systemprompt_models::late_bound::LateBound<",
];

fn open_types_of(rendered: &str) -> &str {
    let start = rendered
        .find("types: [")
        .map_or(0, |at| at + "types: [".len());
    let end = rendered[start..]
        .find("], reserved")
        .map_or(rendered.len(), |at| start + at);
    &rendered[start..end]
}

fn names_capability(open_types: &str, marker: &str) -> bool {
    open_types.match_indices(marker).any(|(at, _)| {
        open_types[at + marker.len()..]
            .chars()
            .next()
            .is_none_or(|next| !next.is_alphanumeric() && next != '_')
    })
}

fn capability_of(sealed: &'static str) -> &'static str {
    let mut name = sealed;
    while let Some(inner) = CELL_WRAPPERS
        .iter()
        .find_map(|wrapper| name.strip_prefix(wrapper))
        .and_then(|inner| inner.strip_suffix('>'))
    {
        name = inner;
    }
    name
}

fn carrier<T: 'static>() -> [(TypeId, &'static str); 2] {
    [
        (TypeId::of::<T>(), type_name::<T>()),
        (TypeId::of::<Arc<T>>(), type_name::<Arc<T>>()),
    ]
}

#[tokio::test]
async fn no_open_value_carries_a_member_sealed_capability() {
    let ctx = context().await;
    let state = ctx.kernel().state();
    let runtime = ctx.kernel().runtime_state();

    let probes: Vec<(&str, [(TypeId, &str); 2])> = vec![
        (
            type_name::<DynManagedSkillResolver>(),
            carrier::<OrganizationSkillResolver>(),
        ),
        (
            type_name::<DynInventoryPublisher>(),
            carrier::<InventoryRefresher>(),
        ),
        (
            type_name::<Arc<systemprompt_extension::service_process::ServiceProcessSets>>(),
            carrier::<
                systemprompt_agent::services::agent_orchestration::process_set::AgentProcessSet,
            >(),
        ),
        (
            type_name::<Arc<systemprompt_extension::service_process::ServiceProcessSets>>(),
            carrier::<systemprompt_mcp::services::process_set::McpProcessSet>(),
        ),
        (
            type_name::<Arc<systemprompt_ai::repository::AiRepositories>>(),
            carrier::<systemprompt_ai::repository::UserProviderCredentialRepository>(),
        ),
        (
            type_name::<systemprompt_traits::DynSafetyFindingsReader>(),
            carrier::<systemprompt_ai::AiSafetyFindingRepository>(),
        ),
        (
            type_name::<systemprompt_traits::DynCatalogMigrationPort>(),
            carrier::<systemprompt_ai::repository::CatalogMigrationRepository>(),
        ),
        (
            type_name::<systemprompt_models::ai::DynAiProvider>(),
            carrier::<systemprompt_ai::AiService>(),
        ),
        (
            type_name::<systemprompt_traits::DynAiSessionProvider>(),
            carrier::<systemprompt_users::UsersAiSessionProvider>(),
        ),
        (
            type_name::<Arc<systemprompt_gateway::GatewayEngines>>(),
            carrier::<systemprompt_gateway::client::GatewayClient>(),
        ),
        (
            type_name::<systemprompt_traits::DynAgentServer>(),
            carrier::<systemprompt_agent::services::a2a_server::serve::HostedAgentServer>(),
        ),
        (
            type_name::<systemprompt_extension::mcp::DynMcpSessionStore>(),
            carrier::<systemprompt_mcp::repository::McpSessionRepository>(),
        ),
        (
            type_name::<Arc<dyn systemprompt_traits::ToolProvider>>(),
            carrier::<systemprompt_mcp::McpToolProvider>(),
        ),
        (
            type_name::<systemprompt_extension::brain::DynBrainConsumerStart>(),
            carrier::<systemprompt_brain::consumer::BrainConsumer>(),
        ),
        (
            type_name::<systemprompt_traits::DynSessionStore>(),
            carrier::<systemprompt_users::SessionRepository>(),
        ),
        (
            type_name::<systemprompt_traits::DynToolCallIntentClaims>(),
            carrier::<systemprompt_ai::repository::AiRequestRepository>(),
        ),
        (
            type_name::<systemprompt_traits::DynGatewaySessionMint>(),
            carrier::<systemprompt_users::UsersGatewaySessionMint>(),
        ),
        (
            type_name::<systemprompt_traits::DynSignInSessions>(),
            carrier::<systemprompt_users::SessionRepository>(),
        ),
        (
            type_name::<Arc<systemprompt_ai::PersonalProviderCredentials>>(),
            carrier::<systemprompt_ai::repository::UserProviderCredentialRepository>(),
        ),
    ];
    let host: HashSet<TypeId> = state.host_types().map(|(id, _)| id).collect();
    let member_sealed: BTreeSet<&str> = state
        .sealed_types()
        .filter(|(id, _)| !host.contains(id))
        .map(|(_, name)| name)
        .collect();
    let probed: BTreeSet<&str> = probes.iter().map(|(sealed, _)| *sealed).collect();
    assert_eq!(
        member_sealed, probed,
        "every member-sealed provide names the implementors no open value may carry"
    );

    let open_names = format!("{runtime:?}");
    for (sealed, implementors) in &probes {
        for (id, name) in implementors {
            assert!(
                !runtime.contains_type(*id),
                "{name} is open in the runtime state while {sealed} is sealed"
            );
            assert!(
                !open_names.contains(name),
                "an open runtime value wraps {name} while {sealed} is sealed"
            );
        }
    }

    let markers: Vec<&str> = state
        .sealed_types()
        .map(|(_, name)| capability_of(name))
        .chain(SUPERSET_OF_A_SEALED_CAPABILITY.iter().copied())
        .chain(std::iter::once(type_name::<AppContext>()))
        .collect();
    let open_types = open_types_of(&open_names);
    for marker in &markers {
        assert!(
            !names_capability(open_types, marker),
            "an open runtime value carries the sealed capability {marker}: {open_types}"
        );
    }

    let sealed = state
        .get::<DynManagedSkillResolver>()
        .expect("the kernel reads the sealed resolver");
    let view = runtime
        .get::<Arc<OwnerSkillView>>()
        .expect("the owner-only view is open");
    assert_ne!(
        Arc::as_ptr(sealed).cast::<()>(),
        Arc::as_ptr(view).cast::<()>(),
        "the open view is not the sealed resolver's allocation"
    );
}

#[derive(Clone)]
struct Laundered(Arc<dyn Any + Send + Sync>);

fn launder<T: Clone + Send + Sync + 'static>(id: &'static str) -> Probe {
    Probe::new(id, |c| {
        c.provide(|r: &Resolved<'_>| -> Result<Laundered, ProvideError> {
            Ok(Laundered(Arc::new(r.get::<T>()?)))
        });
    })
}

fn resolve_over(
    ctx: &AppContext,
    admitted: &systemprompt_extension::Admitted,
) -> Result<Kernel, KernelError> {
    let mut kernel = Kernel::detached(BackgroundTasks::new(), ctx.kernel().state().clone());
    kernel.resolve(admitted)?;
    Ok(kernel)
}

fn installed(probe: Probe) -> systemprompt_extension::Admitted {
    probe_admitted(ComposedParts {
        probes: vec![probe],
        ..ComposedParts::default()
    })
}

fn install_cannot_launder<T: Clone + Send + Sync + 'static>(ctx: &AppContext) {
    let error = resolve_over(ctx, &installed(launder::<T>("forged-install")))
        .err()
        .unwrap_or_else(|| panic!("an install launders {}", type_name::<T>()));
    assert!(
        matches!(
            &error,
            KernelError::Provide { owner, source: ProvideError::Missing(missing), .. }
                if owner.as_str() == "forged-install" && missing.type_name() == type_name::<T>()
        ),
        "unexpected error laundering {}: {error:?}",
        type_name::<T>()
    );
}

#[tokio::test]
async fn a_forged_install_provide_cannot_launder_a_sealed_capability_into_an_open_value() {
    let ctx = context().await;

    let core = resolve_over(
        &ctx,
        &fixture_admitted_of(vec![Box::new(launder::<Arc<Spawner>>("core-launderer"))]),
    )
    .expect("a core provide reads the spawner");
    let laundered = core
        .state()
        .find::<Laundered>()
        .expect("the newtype is provided open");
    assert!(laundered.0.downcast_ref::<Arc<Spawner>>().is_some());
    let open_names = format!("{:?}", core.runtime_state());
    assert!(
        open_names.contains(type_name::<Laundered>())
            && !names_capability(
                open_types_of(&open_names),
                capability_of(type_name::<Arc<Spawner>>())
            ),
        "the name pin cannot see a member newtype: {open_names}"
    );

    install_cannot_launder::<Arc<Spawner>>(&ctx);
    install_cannot_launder::<SigningKeys>(&ctx);
    install_cannot_launder::<DynChildSecretEnv>(&ctx);
    install_cannot_launder::<AgentDispatchCell>(&ctx);
    install_cannot_launder::<LateRead<dyn MessagingDispatcher>>(&ctx);
    install_cannot_launder::<Arc<dyn SecretStore>>(&ctx);
    install_cannot_launder::<Arc<EventRouter>>(&ctx);
    install_cannot_launder::<DynManagedSkillResolver>(&ctx);
}
