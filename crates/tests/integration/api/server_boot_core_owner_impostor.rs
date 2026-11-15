//! The core owner check on the full boot path: an injected extension that
//! takes the `slack` core row id and declares that row's exact shape (base
//! path, handler-verified Slack signature, handler-resolved context,
//! messaging group, agents budget) is refused by `setup_api_server` with
//! `MountError::CoreOwnerImpostor` before any extension factory runs. Its own
//! router factory is never called, and neither is a gateway guard factory
//! linked into this binary: the guard factory counter stays at zero. The
//! negative control builds the gateway engines from the same discovered
//! registry without the impostor and observes the guard factory run, so the
//! counter is proved to see a guard build. Counters are per process; nextest
//! runs each test in its own process.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use systemprompt_api::routes::gateway::GatewayBootError;
use systemprompt_api::services::middleware::MountError;
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_extension::{
    AuthzPolicy, ContextKind, Extension, ExtensionContext, ExtensionMetadata, ExtensionRegistry,
    ExtensionRouter, ExtensionStateError, GatewayDenyReason, GatewayGuardRequest,
    GatewayRequestGuard, HandlerVerifier, RateBudget, RateLimit, RouteGroup,
};
use systemprompt_models::modules::ApiPaths;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config, test_db_pool,
};

static GUARD_FACTORY_CALLS: AtomicUsize = AtomicUsize::new(0);
static IMPOSTOR_ROUTER_CALLS: AtomicUsize = AtomicUsize::new(0);

struct CountedGuard;

#[async_trait::async_trait]
impl GatewayRequestGuard for CountedGuard {
    async fn check(
        &self,
        _db: &dyn systemprompt_traits::DatabaseHandle,
        _request: &GatewayGuardRequest<'_>,
    ) -> Result<(), GatewayDenyReason> {
        Ok(())
    }
}

fn counted_guard(_ctx: &dyn ExtensionContext) -> Arc<dyn GatewayRequestGuard> {
    GUARD_FACTORY_CALLS.fetch_add(1, Ordering::SeqCst);
    Arc::new(CountedGuard)
}

systemprompt_extension::register_gateway_guard!("gateway", factory = counted_guard);

struct SlackImpostor;

impl Extension for SlackImpostor {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "slack",
            name: "Slack Impostor",
            version: "0.0.1",
        }
    }

    fn routers(
        &self,
        _ctx: &dyn ExtensionContext,
    ) -> Result<Vec<ExtensionRouter>, ExtensionStateError> {
        IMPOSTOR_ROUTER_CALLS.fetch_add(1, Ordering::SeqCst);
        Ok(vec![
            ExtensionRouter::new(
                Router::new(),
                ApiPaths::SLACK_BASE,
                AuthzPolicy::handler_verified(HandlerVerifier::SlackSignature),
                ContextKind::HandlerResolved,
            )
            .in_group(RouteGroup::Messaging)
            .with_rate_limit(RateLimit::new("agents", RateBudget::Agents)),
        ])
    }
}

async fn context(
    injected: Vec<Arc<dyn Extension>>,
) -> anyhow::Result<Arc<systemprompt_runtime::AppContext>> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let config = fixture_config(&bootstrap.database_url);
    let mut registry =
        ExtensionRegistry::discover().map_err(|e| anyhow::anyhow!("registry: {e}"))?;
    registry
        .merge(injected)
        .map_err(|e| anyhow::anyhow!("registry: {e}"))?;
    registry
        .validate()
        .map_err(|e| anyhow::anyhow!("registry: {e}"))?;
    fixture_app_context_with_extensions(&pool, config, registry, &[])
}

#[tokio::test]
async fn an_impostor_with_a_core_rows_exact_shape_is_refused_before_any_factory_runs()
-> anyhow::Result<()> {
    let ctx = context(vec![Arc::new(SlackImpostor)]).await?;

    let err = setup_api_server(&ctx, None, &Arc::new(ReadinessState::new()))
        .expect_err("an impostor of a core row must refuse boot");
    let mount = err
        .downcast_ref::<MountError>()
        .unwrap_or_else(|| panic!("the refusal is a MountError: {err:?}"));
    assert!(
        matches!(
            mount,
            MountError::CoreOwnerImpostor {
                extension: "slack",
                owner: "systemprompt_slack_entry::extension::SlackExtension",
                ..
            }
        ),
        "{mount:?}"
    );
    assert_eq!(IMPOSTOR_ROUTER_CALLS.load(Ordering::SeqCst), 0);
    assert_eq!(GUARD_FACTORY_CALLS.load(Ordering::SeqCst), 0);

    let err = systemprompt_api::routes::gateway::gateway_router(&ctx)
        .expect_err("the public gateway router runs the owner check");
    assert!(
        matches!(
            err.downcast_ref::<GatewayBootError>(),
            Some(GatewayBootError::CoreOwner(MountError::CoreOwnerImpostor { .. }))
        ),
        "{err:?}"
    );
    assert_eq!(GUARD_FACTORY_CALLS.load(Ordering::SeqCst), 0);
    Ok(())
}

#[tokio::test]
async fn without_the_impostor_the_guard_factory_is_observed() -> anyhow::Result<()> {
    let ctx = context(Vec::new()).await?;
    systemprompt_api::routes::gateway::gateway_engines(&ctx)?;
    assert!(
        GUARD_FACTORY_CALLS.load(Ordering::SeqCst) > 0,
        "the guard factory counter must see a guard build"
    );
    Ok(())
}
