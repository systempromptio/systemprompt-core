//! `/.well-known/systemprompt-bridge`, the unauthenticated discovery document.
//!
//! A shared bridge binary reads it to find this instance's sign-in and
//! identity routes, its branding and the hosts it serves. It carries no trust
//! material.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use systemprompt_manifest::services::ServicesConfig;
use systemprompt_models::api::ApiError;
use systemprompt_models::bridge::instance::{BridgeDiscovery, DISCOVERY_CONTRACT};
use systemprompt_models::modules::ApiPaths;
use systemprompt_runtime::AppContext;

use crate::error::ApiHttpError;
use crate::services::middleware::{RateLimitState, RouterExt};
use systemprompt_extension::LoaderError;

const DISCOVERY_RATE_LIMIT_PER_SECOND: u64 = 5;

pub fn bridge_discovery_router(ctx: &AppContext) -> Result<Router, LoaderError> {
    let limits = RateLimitState::from_context(ctx);
    Router::new()
        .route(ApiPaths::WELLKNOWN_BRIDGE, get(handle_discovery))
        .with_state(ctx.clone())
        .with_rate_limit(&limits, DISCOVERY_RATE_LIMIT_PER_SECOND, "wellknown_bridge")
}

async fn handle_discovery(
    State(ctx): State<AppContext>,
) -> Result<impl IntoResponse, ApiHttpError> {
    discovery_document(&ctx.services_config())
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
        enabled_hosts: services.instance_enabled_hosts(),
    })
}
