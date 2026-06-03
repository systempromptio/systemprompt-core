use std::sync::Arc;

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use serde_json::json;
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::{JwtToken, TenantId};
use systemprompt_models::bridge::profile as bridge_profile;
use systemprompt_models::profile::ApiSurface;

use systemprompt_security::manifest_signing;
use uuid::Uuid;

pub use systemprompt_models::bridge::profile::{
    BridgeProfileResponse, ProviderHealth, provider_health,
};

use super::bridge_data;
use super::messages::extract_credential;
use crate::services::middleware::JwtContextExtractor;

pub(super) const KNOWN_HOSTS: &[&str] = &["claude-code", "claude-desktop", "cowork", "codex-cli"];

#[derive(Debug, Deserialize)]
pub struct EnabledHostsRequest {
    pub host_id: String,
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct SetHostPrefResponse {
    pub host_id: String,
    pub enabled: bool,
}

pub async fn set_enabled_host(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: systemprompt_runtime::AppContext,
    headers: HeaderMap,
    Json(body): Json<EnabledHostsRequest>,
) -> Result<Json<SetHostPrefResponse>, (StatusCode, String)> {
    let credential = extract_credential(&headers).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization or x-api-key credential".to_owned(),
        )
    })?;
    let (claims, _user) = jwt_extractor
        .decode_for_gateway(&JwtToken::new(credential))
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    if !KNOWN_HOSTS.iter().any(|h| *h == body.host_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("unknown host: {}", body.host_id),
        ));
    }

    bridge_data::upsert_host_pref(&ctx, &claims.user_id, &body.host_id, body.enabled)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(SetHostPrefResponse {
        host_id: body.host_id,
        enabled: body.enabled,
    }))
}

#[derive(Debug, Deserialize)]
pub struct HostModelFilterRequest {
    pub host_id: String,
    /// API-surface tags the host should advertise. `None` clears the override
    /// (host falls back to its built-in default); `Some(empty)` means "all
    /// models" (no restriction).
    #[serde(default)]
    pub model_protocols: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct HostModelFilterResponse {
    pub host_id: String,
    pub model_protocols: Option<Vec<String>>,
}

pub async fn set_host_model_filter(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: systemprompt_runtime::AppContext,
    headers: HeaderMap,
    Json(body): Json<HostModelFilterRequest>,
) -> Result<Json<HostModelFilterResponse>, (StatusCode, String)> {
    let credential = extract_credential(&headers).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization or x-api-key credential".to_owned(),
        )
    })?;
    let (claims, _user) = jwt_extractor
        .decode_for_gateway(&JwtToken::new(credential))
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    if !KNOWN_HOSTS.iter().any(|h| *h == body.host_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("unknown host: {}", body.host_id),
        ));
    }

    let normalized = body
        .model_protocols
        .as_ref()
        .map(|tags| {
            tags.iter()
                .map(|tag| {
                    ApiSurface::from_tag(tag)
                        .map(|s| s.as_tag().to_owned())
                        .ok_or_else(|| {
                            (
                                StatusCode::BAD_REQUEST,
                                format!("unknown API surface: {tag}"),
                            )
                        })
                })
                .collect::<Result<Vec<String>, _>>()
        })
        .transpose()?;

    bridge_data::set_host_model_protocols(
        &ctx,
        &claims.user_id,
        &body.host_id,
        normalized.as_deref(),
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(HostModelFilterResponse {
        host_id: body.host_id,
        model_protocols: normalized,
    }))
}

pub async fn pubkey() -> impl IntoResponse {
    match manifest_signing::pubkey_b64() {
        Ok(b64) => (StatusCode::OK, Json(json!({ "pubkey": b64 }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn profile() -> Result<Json<BridgeProfileResponse>, (StatusCode, String)> {
    let profile = ProfileBootstrap::get().map_err(|e| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("Profile not ready: {e}"),
        )
    })?;

    let gateway = profile
        .gateway
        .as_ref()
        .and_then(systemprompt_models::profile::GatewayState::resolved)
        .filter(|g| g.enabled)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Gateway not enabled".to_owned()))?;

    let base = profile.server.api_external_url.trim_end_matches('/');
    let prefix = gateway.inference_path_prefix.trim_end_matches('/');
    let inference_gateway_base_url = format!("{base}{prefix}");

    let organization_uuid = profile
        .cloud
        .as_ref()
        .and_then(|cloud| cloud.tenant_id.as_ref())
        .map(canonicalize_org_uuid);

    let secrets = systemprompt_config::SecretsBootstrap::get().ok();
    let response = bridge_profile::build(
        inference_gateway_base_url,
        gateway.auth_scheme.clone(),
        organization_uuid,
        &profile.providers,
        |name| {
            secrets
                .and_then(|s| s.get(name))
                .is_some_and(|k| !k.is_empty())
        },
    );

    Ok(Json(response))
}

/// Codex CLI threads this into the outbound `x-tenant` header, where downstream
/// tenant attribution expects a canonical RFC-4122 UUID, not the internal
/// `local_`-prefixed form. Only this bridge-facing handler peels the prefix.
fn canonicalize_org_uuid(tenant_id: &TenantId) -> String {
    let raw = tenant_id.as_str();
    let suffix = raw.strip_prefix("local_").unwrap_or(raw);
    if let Ok(parsed) = Uuid::parse_str(suffix) {
        return parsed.to_string();
    }
    Uuid::new_v5(&Uuid::NAMESPACE_OID, raw.as_bytes()).to_string()
}
