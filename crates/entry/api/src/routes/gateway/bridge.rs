use std::sync::Arc;

use axum::Json;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use serde_json::json;
use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::{JwtToken, TenantId};

use systemprompt_security::manifest_signing;
use uuid::Uuid;

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

#[derive(Debug, Serialize)]
pub struct BridgeProfileResponse {
    pub inference_gateway_base_url: String,
    pub auth_scheme: String,
    pub models: Vec<String>,
    pub organization_uuid: Option<String>,
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

    let models: Vec<String> = profile
        .providers
        .providers
        .iter()
        .flat_map(|entry| {
            entry.models.iter().flat_map(|m| {
                std::iter::once(m.id.as_str().to_owned())
                    .chain(m.aliases.iter().map(|a| a.as_str().to_owned()))
            })
        })
        .collect();

    let organization_uuid = profile
        .cloud
        .as_ref()
        .and_then(|cloud| cloud.tenant_id.as_ref())
        .map(canonicalize_org_uuid);

    Ok(Json(BridgeProfileResponse {
        inference_gateway_base_url,
        auth_scheme: gateway.auth_scheme.clone(),
        models,
        organization_uuid,
    }))
}

// Why: Codex CLI threads this value into the `x-tenant` HTTP header on
// outbound inference requests, and downstream tenant-attribution consumers
// expect a canonical RFC-4122 UUID rather than the internal `local_`-
// prefixed form. Internal state keeps the prefix; only the bridge-facing
// handler peels it.
fn canonicalize_org_uuid(tenant_id: &TenantId) -> String {
    let raw = tenant_id.as_str();
    let suffix = raw.strip_prefix("local_").unwrap_or(raw);
    if let Ok(parsed) = Uuid::parse_str(suffix) {
        return parsed.to_string();
    }
    Uuid::new_v5(&Uuid::NAMESPACE_OID, raw.as_bytes()).to_string()
}
