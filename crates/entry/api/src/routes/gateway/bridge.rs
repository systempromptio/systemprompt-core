//! Bridge profile endpoint: providers, hosts, and per-host protocol overrides.
//!
//! The profile is always built for a verified user: a missing, invalid or
//! stale bearer is 401, never an anonymous profile, and its model list is
//! what dispatch would serve that user (see `bridge_admission`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::TenantId;
use systemprompt_loader::ServicesBootstrap;
use systemprompt_manifest::bridge_profile;
use systemprompt_models::providers::ClientFamily;
use systemprompt_oauth::repository::HostPrefWrite;

use systemprompt_security::manifest_signing;

pub use systemprompt_manifest::bridge_profile::provider_health;
pub use systemprompt_models::bridge::profile::{BridgeProfileResponse, ProviderHealth};

use super::bridge_admission::{AdmissionPrincipal, ModelAdmission};
use super::bridge_data;
use super::bridge_error::{BridgeError, authenticate_bridge};
use crate::error::ApiHttpError;
use crate::services::middleware::JwtContextExtractor;

use systemprompt_models::bridge::host::HostKind;

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

    let services = bridge_data::load_services_config()
        .map_err(|e| BridgeError::internal("services config load failed", e))?;
    let instance_hosts = services.instance_enabled_hosts();
    if body.enabled && !instance_hosts.contains(&body.host_id) {
        return Err(BridgeError::HostDisabled(body.host_id).into());
    }

    let written = bridge_data::set_host_enabled(
        &ctx,
        &claims.user_id,
        body.host_id,
        body.enabled,
        &instance_hosts,
    )
    .await
    .map_err(BridgeError::from)?;
    if written == HostPrefWrite::WouldDisableAll {
        return Err(BridgeError::LastHostDisabled(body.host_id).into());
    }

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
                    ClientFamily::from_tag(tag)
                        .map(|family| family.as_tag().to_owned())
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

pub async fn profile(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: systemprompt_runtime::AppContext,
    headers: HeaderMap,
) -> Result<Json<BridgeProfileResponse>, ApiHttpError> {
    let (claims, user) = authenticate_bridge(&jwt_extractor, &headers).await?;
    let profile =
        ProfileBootstrap::get().map_err(|e| BridgeError::unavailable("profile not ready", e))?;
    let services = ServicesBootstrap::get()
        .map_err(|e| BridgeError::unavailable("services config not ready", e))?;
    let catalog = ServicesBootstrap::catalog()
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
        .map(TenantId::organization_uuid);

    let secrets = systemprompt_config::SecretsBootstrap::get()
        .map_err(|e| BridgeError::unavailable("secrets not ready", e))?;
    let personal_keys = personal_keys(&ctx, &claims.user_id).await?;
    let secret_present = |name: &str| secrets.get(name).is_some_and(|k| !k.is_empty());
    let mut response = bridge_profile::build(
        bridge_profile::BridgeProfileParams {
            inference_gateway_base_url,
            auth_scheme: gateway.auth_scheme.clone(),
            organization_uuid,
            catalog,
        },
        &bridge_profile::CredentialFacts {
            secret_present: &secret_present,
            personal_keys: &personal_keys,
        },
    );

    let principal = AdmissionPrincipal {
        user_id: claims.user_id,
        roles: user.roles,
        client_id: claims.client_id,
        session_id: claims.session_id,
    };
    let admission = ModelAdmission {
        catalog,
        hook: ctx.authz_hook(),
        principal: &principal,
    };
    response.models = admission.retain(std::mem::take(&mut response.models)).await;
    response
        .model_limits
        .retain(|model, _| response.models.contains(model));

    Ok(Json(response))
}

async fn personal_keys(
    ctx: &systemprompt_runtime::AppContext,
    user_id: &systemprompt_identifiers::UserId,
) -> Result<BTreeSet<String>, ApiHttpError> {
    let stored = ctx
        .ai_repositories()
        .user_provider_credentials
        .list_metadata(user_id)
        .await
        .map_err(|e| BridgeError::internal("personal key lookup failed", e))?;
    Ok(stored.into_iter().map(|meta| meta.credential).collect())
}
