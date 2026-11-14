//! Extension routers on each node role through `setup_api_server`: an
//! install extension's routers are built, and refused, only on a role that
//! serves `Extensions`, as at the base, so an install declaration under a
//! reserved prefix or a router that fails to build refuses boot on the all
//! and api roles and leaves the gateway, worker and runner roles booting.

use std::sync::Arc;

use axum::Router;
use systemprompt_api::services::server::setup_api_server;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, Extension, ExtensionContext, ExtensionMetadata, ExtensionRegistry,
    ExtensionRouter, ExtensionStateError,
};
use systemprompt_manifest::profile::{NodeRole, RateLimitsConfig};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config, test_db_pool,
};

struct ReservedPrefixExt;

impl Extension for ReservedPrefixExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "covreserved",
            name: "Coverage Reserved Prefix Extension",
            version: "0.0.1",
        }
    }

    fn routers(
        &self,
        _ctx: &dyn ExtensionContext,
    ) -> Result<Vec<ExtensionRouter>, ExtensionStateError> {
        Ok(vec![ExtensionRouter::new(
            Router::new(),
            "/api/v1/admin/covreserved",
            AuthzPolicy::admin(),
            ContextKind::User,
        )])
    }
}

struct BrokenRouterExt;

impl Extension for BrokenRouterExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "covbroken",
            name: "Coverage Broken Router Extension",
            version: "0.0.1",
        }
    }

    fn routers(
        &self,
        _ctx: &dyn ExtensionContext,
    ) -> Result<Vec<ExtensionRouter>, ExtensionStateError> {
        Err(ExtensionStateError::NotReady)
    }
}

async fn boot(role: NodeRole, injected: Arc<dyn Extension>) -> anyhow::Result<Result<(), String>> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.rate_limits = RateLimitsConfig::disabled();
    config.cors_allowed_origins = vec!["http://127.0.0.1".to_owned()];
    config.role = role;

    let mut registry =
        ExtensionRegistry::discover().map_err(|e| anyhow::anyhow!("registry: {e}"))?;
    registry
        .merge(vec![injected])
        .map_err(|e| anyhow::anyhow!("registry: {e}"))?;
    registry
        .validate()
        .map_err(|e| anyhow::anyhow!("registry: {e}"))?;
    let ctx = fixture_app_context_with_extensions(&pool, config, registry, &[])?;

    Ok(setup_api_server(
        &ctx,
        None,
        &Arc::new(systemprompt_api::services::server::ReadinessState::new()),
    )
    .map(|_| ())
    .map_err(|e| format!("{e:#}")))
}

#[tokio::test]
async fn an_install_reserved_prefix_refuses_boot_only_where_extensions_are_built()
-> anyhow::Result<()> {
    for role in [NodeRole::All, NodeRole::Api] {
        let refused = boot(role, Arc::new(ReservedPrefixExt)).await?;
        assert!(
            refused
                .as_ref()
                .is_err_and(|e| e.contains("/api/v1/admin/covreserved")),
            "{role:?} must refuse the reserved prefix: {refused:?}"
        );
    }
    for role in [NodeRole::Gateway, NodeRole::Worker, NodeRole::Runner] {
        let booted = boot(role, Arc::new(ReservedPrefixExt)).await?;
        assert!(
            booted.is_ok(),
            "{role:?} never builds an install router, as at the base: {booted:?}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_router_build_failure_refuses_boot_only_where_extensions_are_built()
-> anyhow::Result<()> {
    let refused = boot(NodeRole::All, Arc::new(BrokenRouterExt)).await?;
    assert!(
        refused.as_ref().is_err_and(|e| e.contains("covbroken")),
        "{refused:?}"
    );
    for role in [NodeRole::Gateway, NodeRole::Worker, NodeRole::Runner] {
        let booted = boot(role, Arc::new(BrokenRouterExt)).await?;
        assert!(booted.is_ok(), "{role:?}: {booted:?}");
    }
    Ok(())
}
