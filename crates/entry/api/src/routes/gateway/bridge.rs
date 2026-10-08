//! Bridge profile endpoint: providers, hosts, and per-host protocol overrides.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::TenantId;
use systemprompt_loader::ServicesBootstrap;
use systemprompt_manifest::bridge_profile;
use systemprompt_models::providers::ApiSurface;

use systemprompt_security::manifest_signing;
use uuid::Uuid;

pub use systemprompt_manifest::bridge_profile::provider_health;
pub use systemprompt_models::bridge::profile::{BridgeProfileResponse, ProviderHealth};

use super::bridge_data;
use super::bridge_error::{BridgeError, authenticate_bridge};
use crate::error::ApiHttpError;
use crate::services::middleware::JwtContextExtractor;

use systemprompt_models::bridge::host::HostKind;

pub fn instance_enabled_hosts(
    services: &systemprompt_manifest::services::ServicesConfig,
) -> Vec<HostKind> {
    HostKind::ALL
        .into_iter()
        .filter(|host| {
            services
                .external_agents
                .iter()
                .find(|(id, _)| id.as_str().replace('_', "-") == host.as_str())
                .is_none_or(|(_, agent)| agent.enabled)
        })
        .collect()
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct EnabledHostsRequest {
    pub host_id: HostKind,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SetHostPrefResponse {
    pub host_id: HostKind,
    pub enabled: bool,
}

pub async fn set_enabled_host(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: systemprompt_runtime::AppContext,
    headers: HeaderMap,
    body: Result<Json<EnabledHostsRequest>, JsonRejection>,
) -> Result<Json<SetHostPrefResponse>, ApiHttpError> {
    let (claims, _user) = authenticate_bridge(&jwt_extractor, &headers).await?;
    let Json(body) = body.map_err(BridgeError::InvalidBody)?;

    if body.enabled {
        let services = bridge_data::load_services_config()
            .map_err(|e| BridgeError::internal("services config load failed", e))?;
        if !instance_enabled_hosts(&services).contains(&body.host_id) {
            return Err(BridgeError::HostDisabled(body.host_id).into());
        }
    }

    bridge_data::upsert_host_pref(&ctx, &claims.user_id, body.host_id, body.enabled)
        .await
        .map_err(BridgeError::from)?;

    Ok(Json(SetHostPrefResponse {
        host_id: body.host_id,
        enabled: body.enabled,
    }))
}

#[derive(Debug, Deserialize)]
pub struct HostModelFilterRequest {
    pub host_id: HostKind,
    #[serde(default)]
    pub model_protocols: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct HostModelFilterResponse {
    pub host_id: HostKind,
    pub model_protocols: Option<Vec<String>>,
}

pub async fn set_host_model_filter(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: systemprompt_runtime::AppContext,
    headers: HeaderMap,
    body: Result<Json<HostModelFilterRequest>, JsonRejection>,
) -> Result<Json<HostModelFilterResponse>, ApiHttpError> {
    let (claims, _user) = authenticate_bridge(&jwt_extractor, &headers).await?;
    let Json(body) = body.map_err(BridgeError::InvalidBody)?;

    let normalized = body
        .model_protocols
        .as_ref()
        .map(|tags| {
            tags.iter()
                .map(|tag| {
                    ApiSurface::from_tag(tag)
                        .map(|s| s.as_tag().to_owned())
                        .ok_or_else(|| BridgeError::UnknownSurface(tag.clone()))
                })
                .collect::<Result<Vec<String>, _>>()
        })
        .transpose()?;

    bridge_data::set_host_model_protocols(
        &ctx,
        &claims.user_id,
        body.host_id,
        normalized.as_deref(),
    )
    .await
    .map_err(BridgeError::from)?;

    Ok(Json(HostModelFilterResponse {
        host_id: body.host_id,
        model_protocols: normalized,
    }))
}

#[derive(Debug, Serialize)]
pub struct PubkeyResponse {
    pub pubkey: String,
}

pub async fn pubkey() -> Result<Json<PubkeyResponse>, ApiHttpError> {
    let pubkey = manifest_signing::pubkey_b64()
        .map_err(|e| BridgeError::internal("manifest signing key unavailable", e))?;
    Ok(Json(PubkeyResponse { pubkey }))
}

pub async fn profile() -> Result<Json<BridgeProfileResponse>, ApiHttpError> {
    let profile =
        ProfileBootstrap::get().map_err(|e| BridgeError::unavailable("profile not ready", e))?;
    let services = ServicesBootstrap::get()
        .map_err(|e| BridgeError::unavailable("services config not ready", e))?;
    let gateway = services
        .gateway_config()
        .filter(|g| g.enabled)
        .ok_or(BridgeError::GatewayDisabled)?;

    let base = profile.server.api_external_url.trim_end_matches('/');
    let prefix = gateway.inference_path_prefix.trim_end_matches('/');
    let inference_gateway_base_url = format!("{base}{prefix}");

    let organization_uuid = profile
        .cloud
        .as_ref()
        .and_then(|cloud| cloud.tenant_id.as_ref())
        .map(canonicalize_org_uuid);

    let secrets = systemprompt_config::SecretsBootstrap::get()
        .map_err(|e| BridgeError::unavailable("secrets not ready", e))?;
    let response = bridge_profile::build(
        bridge_profile::BridgeProfileParams {
            inference_gateway_base_url,
            auth_scheme: gateway.auth_scheme.clone(),
            organization_uuid,
            default_model: gateway.default_model.clone(),
            registry: &services.providers,
            gateway: Some(gateway),
        },
        |name| secrets.get(name).is_some_and(|k| !k.is_empty()),
    );

    Ok(Json(response))
}

pub fn canonicalize_org_uuid(tenant_id: &TenantId) -> String {
    let raw = tenant_id.as_str();
    let suffix = raw.strip_prefix("local_").unwrap_or(raw);
    if let Ok(parsed) = Uuid::parse_str(suffix) {
        return parsed.to_string();
    }
    Uuid::new_v5(&Uuid::NAMESPACE_OID, raw.as_bytes()).to_string()
}
