//! `/.well-known/systemprompt-bridge`, the unauthenticated discovery document.
//!
//! A shared bridge binary reads it to find this instance's sign-in and
//! identity routes, its branding and the hosts it serves. It carries no trust
//! material. The bridge is its only reader and manages no host that configures
//! itself, so `enabled_hosts` lists the [`ConfigOwner::Bridge`] hosts only: a
//! bridge of any version parses the document whatever the instance enables.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use systemprompt_loader::ServicesState;
use systemprompt_manifest::services::ServicesConfig;
use systemprompt_models::api::ApiError;
use systemprompt_models::bridge::host::ConfigOwner;
use systemprompt_models::bridge::instance::{BridgeDiscovery, DISCOVERY_CONTRACT};
use systemprompt_models::modules::ApiPaths;
use systemprompt_runtime::AppContext;

use crate::kernel::{ApiHttpError, RateLimitState, RouterExt};
use systemprompt_extension::LoaderError;

const DISCOVERY_RATE_LIMIT_PER_SECOND: u64 = 5;

pub fn bridge_discovery_router(ctx: &AppContext) -> Result<Router, LoaderError> {
    let limits = RateLimitState::from_context(ctx)?;
    Router::new()
        .route(ApiPaths::WELLKNOWN_BRIDGE, get(handle_discovery))
        .with_state(ctx.clone())
        .with_rate_limit(&limits, DISCOVERY_RATE_LIMIT_PER_SECOND, "wellknown_bridge")
}

async fn handle_discovery(
    State(ctx): State<AppContext>,
) -> Result<impl IntoResponse, ApiHttpError> {
    discovery_document(&ctx.kernel().state().get::<ServicesState>()?.config())
        .map(Json)
        .ok_or_else(|| {
            ApiError::not_found("bridge discovery is not configured on this instance")
                .with_error_key("bridge_instance_not_configured")
                .into()
        })
}

#[must_use]
pub fn discovery_document(services: &ServicesConfig) -> Option<BridgeDiscovery> {
    let instance = services.gateway_config()?.bridge_instance.as_ref()?;
    Some(BridgeDiscovery {
        contract: DISCOVERY_CONTRACT.to_owned(),
        device_link_path: instance.device_link_path.clone(),
        identity_path: instance.identity_path.clone(),
        branding: instance.branding.to_branding(),
        enabled_hosts: services
            .instance_enabled_hosts()
            .into_iter()
            .filter(|host| host.config_owner() == ConfigOwner::Bridge)
            .collect(),
    })
}
