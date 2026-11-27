//! Bridge profile endpoint: providers, hosts, and per-host protocol overrides.
//!
//! The profile is always built for a verified user: a missing, invalid or
//! stale bearer is 401, never an anonymous profile, and its model list is
//! what dispatch would serve that user (see `bridge_admission`).
//!
//! Both preference routes refuse a host the instance does not serve
//! (`host_disabled`) whatever the write is: switching on, switching off,
//! storing a protocol filter or clearing one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::TenantId;
use systemprompt_manifest::bridge_profile;
use systemprompt_models::providers::ClientFamily;
use systemprompt_oauth::repository::HostPrefWrite;

pub use systemprompt_manifest::bridge_profile::provider_health;
pub use systemprompt_models::bridge::profile::{BridgeProfileResponse, ProviderHealth};

use super::admission::{AdmissionPrincipal, ModelAdmission};
use super::error::{BridgeError, authenticate_bridge};
use super::{BridgeRouteState, data};
use systemprompt_extension::router::{ApiHttpError, GatewayPrincipal};
use systemprompt_gateway::surface::views::GatewayManifestSigner;

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
    state: BridgeRouteState,
    headers: HeaderMap,
    body: Result<Json<EnabledHostsRequest>, JsonRejection>,
) -> Result<Json<SetHostPrefResponse>, ApiHttpError> {
    let (claims, _user) = authenticate_bridge(&state.authenticator, &headers).await?;
    let Json(body) = body.map_err(BridgeError::InvalidBody)?;

    let services = state.host.services.view();
    let instance = services.config().instance_hosts();
    if !instance.enabled.contains(&body.host_id) {
        return Err(BridgeError::HostDisabled(body.host_id).into());
    }

    let written = data::set_host_enabled(
        &state,
        &claims.user_id,
        body.host_id,
        body.enabled,
        &instance,
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
    state: BridgeRouteState,
    headers: HeaderMap,
    body: Result<Json<HostModelFilterRequest>, JsonRejection>,
) -> Result<Json<HostModelFilterResponse>, ApiHttpError> {
    let (claims, _user) = authenticate_bridge(&state.authenticator, &headers).await?;
    let Json(body) = body.map_err(BridgeError::InvalidBody)?;

    let instance = state.host.services.view().config().instance_hosts();
    if !instance.enabled.contains(&body.host_id) {
        return Err(BridgeError::HostDisabled(body.host_id).into());
    }

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

    data::set_host_model_protocols(&state, &claims.user_id, body.host_id, normalized.as_deref())
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

pub fn pubkey(signer: &GatewayManifestSigner) -> Json<PubkeyResponse> {
    Json(PubkeyResponse {
        pubkey: signer.public_key_b64(),
    })
}

pub async fn profile(
    state: BridgeRouteState,
    caller: GatewayPrincipal,
    headers: HeaderMap,
) -> Result<Json<BridgeProfileResponse>, ApiHttpError> {
    let (claims, user) = authenticate_bridge(&state.authenticator, &headers).await?;
    let view = state.host.services.view();
    let catalog = &view.catalog;
    let services = view.config();
    let gateway = services
        .gateway_config()
        .filter(|g| g.enabled)
        .ok_or(BridgeError::GatewayDisabled)?;

    let base = state.settings.api_external_url().trim_end_matches('/');
    let prefix = gateway.inference_path_prefix.trim_end_matches('/');
    let inference_gateway_base_url = format!("{base}{prefix}");

    let organization_uuid = state
        .host
        .tenant_id
        .as_ref()
        .map(TenantId::organization_uuid);

    let personal_keys = personal_keys(&state, &caller).await?;
    let secret_present = |name: &str| state.settings.credential_configured(name);
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
            presents_own_credential: false,
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
        hook: &state.host.authz_hook,
        principal: &principal,
    };
    response.models = admission.retain(std::mem::take(&mut response.models)).await;
    response
        .model_limits
        .retain(|model, _| response.models.contains(model));

    Ok(Json(response))
}

async fn personal_keys(
    state: &BridgeRouteState,
    caller: &GatewayPrincipal,
) -> Result<BTreeSet<String>, ApiHttpError> {
    let names = state.personal_credentials.as_ref().ok_or_else(|| {
        BridgeError::internal(
            "the personal credential names are unavailable",
            "no member provides the personal credential names",
        )
    })?;
    Ok(names
        .credential_names(caller)
        .await
        .map_err(|e| BridgeError::internal("personal key lookup failed", e))?)
}
