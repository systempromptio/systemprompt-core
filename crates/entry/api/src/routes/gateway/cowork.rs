use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;
use serde_json::json;
use systemprompt_models::ProfileBootstrap;
use systemprompt_security::manifest_signing;

pub async fn pubkey() -> impl IntoResponse {
    match manifest_signing::pubkey_b64() {
        Ok(b64) => (StatusCode::OK, Json(json!({ "pubkey": b64 }))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response(),
    }
}

#[derive(Debug, Serialize)]
pub struct CoworkProfileResponse {
    pub inference_gateway_base_url: String,
    pub auth_scheme: String,
    pub models: Vec<String>,
    pub organization_uuid: Option<String>,
}

pub async fn profile() -> Result<Json<CoworkProfileResponse>, (StatusCode, String)> {
    let profile = ProfileBootstrap::get().map_err(|e| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("Profile not ready: {e}"),
        )
    })?;

    let gateway = profile
        .gateway
        .as_ref()
        .filter(|g| g.enabled)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Gateway not enabled".to_string()))?;

    let base = profile.server.api_external_url.trim_end_matches('/');
    let prefix = gateway.inference_path_prefix.trim_end_matches('/');
    let inference_gateway_base_url = format!("{base}{prefix}");

    let models: Vec<String> = gateway
        .catalog
        .as_ref()
        .map(|c| c.models.iter().map(|m| m.id.clone()).collect())
        .unwrap_or_default();

    let organization_uuid = profile
        .cloud
        .as_ref()
        .and_then(|c| c.tenant_id.clone());

    Ok(Json(CoworkProfileResponse {
        inference_gateway_base_url,
        auth_scheme: gateway.auth_scheme.clone(),
        models,
        organization_uuid,
    }))
}
