use std::sync::Arc;

use systemprompt_database::{CoordinationRepositories, ServiceRepository};
use systemprompt_events::EventBridgeHandle;
use systemprompt_extension::StandingTag;
use systemprompt_governance::authz::SharedAuthzHook;
use systemprompt_governance::policy::GovernanceEngine;
use systemprompt_identifiers::ExtensionId;
use systemprompt_loader::{BootContext, ServicesState};
use systemprompt_logging::LoggingRuntime;
use systemprompt_models::{BuildIdentity, RouteClassifier};
use systemprompt_provider_contracts::Dependencies;
use systemprompt_runtime::{
    AppContext, ConfigPlane, ContextPlanes, DataPlane, Kernel, PendingBinders, RuntimeError,
    SchemaState, ShutdownRequest, Subsystems,
};
use systemprompt_test_fixtures::{AppContextSeeds, test_app_context, test_database_url};
use systemprompt_traits::{BackgroundTasks, ConfigWatch};

fn planes_of(ctx: &AppContext) -> ContextPlanes {
    ContextPlanes {
        data: DataPlane {
            service_repository: Arc::clone(ctx.service_repository()),
            coordination: Arc::clone(ctx.coordination_repositories()),
        },
        cfg: ConfigPlane {
            boot: ctx.boot().clone(),
            route_classifier: Arc::clone(ctx.route_classifier()),
            build: ctx.build_identity().clone(),
        },
        subsystems: Subsystems {
            logging: ctx.logging().clone(),
            authz_hook: Arc::clone(ctx.authz_hook()),
            governance: ctx.governance_watch().clone(),
            services: ctx.services_state().clone(),
            schema_install: Arc::clone(ctx.schema_state()),
            event_bridge: Arc::clone(ctx.event_bridge()),
            shutdown: ctx.shutdown_request().clone(),
            late_binders: PendingBinders::default(),
        },
    }
}

#[tokio::test]
async fn a_kernel_without_a_host_seed_refuses_the_context() {
    let ctx = test_app_context(&test_database_url()).await;
    let bare = Kernel::detached(BackgroundTasks::default(), Dependencies::new());
    let refused = AppContext::from_parts(planes_of(&ctx), bare, Arc::clone(ctx.admitted()));
    assert!(
        matches!(refused, Err(RuntimeError::Missing(_))),
        "a missing host seed is a typed refusal: {refused:?}"
    );
}

#[tokio::test]
async fn every_context_built_seed_is_sealed_and_withheld_from_member_contexts() {
    let ctx = test_app_context(&test_database_url()).await;
    let state = ctx.kernel().state();
    assert!(state.is_sealed::<Arc<ServiceRepository>>());
    assert!(state.is_sealed::<Arc<CoordinationRepositories>>());
    assert!(state.is_sealed::<BootContext>());
    assert!(state.is_sealed::<Arc<RouteClassifier>>());
    assert!(state.is_sealed::<BuildIdentity>());
    assert!(state.is_sealed::<LoggingRuntime>());
    assert!(state.is_sealed::<SharedAuthzHook>());
    assert!(state.is_sealed::<ConfigWatch<GovernanceEngine>>());
    assert!(state.is_sealed::<ServicesState>());
    assert!(state.is_sealed::<Arc<SchemaState>>());
    assert!(state.is_sealed::<Arc<EventBridgeHandle>>());
    assert!(state.is_sealed::<ShutdownRequest>());
    assert!(state.is_sealed::<PendingBinders>());

    let runtime = ctx.kernel().runtime_state();
    assert!(runtime.find::<BootContext>().is_none());
    assert!(runtime.find::<ConfigWatch<GovernanceEngine>>().is_none());
    assert!(runtime.find::<Arc<ServiceRepository>>().is_none());
    assert!(runtime.find::<ShutdownRequest>().is_none());

    let probe = ExtensionId::new("install-probe");
    let install = ctx.kernel().route_scoped(&probe, StandingTag::Install, &[]);
    assert!(install.get::<BootContext>().is_err());
    assert!(install.get::<ShutdownRequest>().is_err());
    assert!(install.get::<Arc<CoordinationRepositories>>().is_err());
}
