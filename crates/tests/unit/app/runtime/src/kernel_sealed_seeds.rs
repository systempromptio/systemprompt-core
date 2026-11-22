//! The kernel state seals its secret-bearing and boot-only seeds: the
//! resolver a provide fn and a kernel command read serves them, the resolver
//! a member's command runs against refuses them as missing. A member value
//! provided sealed (one that acts with standing authority) is sealed the same
//! way, and a route set's resolver serves it only to its provider. A seal
//! covers the capability, not one type name: no open value of the runtime
//! state is a member-sealed value's implementor, wraps one, or shares its
//! allocation.

use std::any::{TypeId, type_name};
use std::collections::{BTreeSet, HashSet};
use std::sync::Arc;

use systemprompt_events::EventRouter;
use systemprompt_extension::Resolved;
use systemprompt_identifiers::{ExtensionId, InstanceId};
use systemprompt_manifest::Config;
use systemprompt_manifest::services::SystemAdmin;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_marketplace::inventory::InventoryRefresher;
use systemprompt_marketplace::managed::{OrganizationSkillResolver, OwnerSkillView};
use systemprompt_runtime::{AppContext, Kernel};
use systemprompt_security::keys::SigningKeys;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_database_url};
use systemprompt_traits::{
    AppContext as AppContextTrait, BackgroundTasks, DynInventoryPublisher, DynManagedSkillResolver,
    SecretStore,
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
            .route_scoped(&other, &[])
            .get::<DynInventoryPublisher>()
            .is_err(),
        "another member's route state is refused it"
    );
    let provider = ExtensionId::new("managed_resources");
    assert!(
        kernel
            .route_scoped(&provider, &[])
            .get::<DynInventoryPublisher>()
            .is_ok(),
        "its provider's own route state reads it"
    );
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

    let probes: [(&str, [(TypeId, &str); 2]); 4] = [
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
            carrier::<systemprompt_agent::services::agent_orchestration::process_set::AgentProcessSet>(),
        ),
        (
            type_name::<Arc<systemprompt_extension::service_process::ServiceProcessSets>>(),
            carrier::<systemprompt_mcp::services::process_set::McpProcessSet>(),
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
