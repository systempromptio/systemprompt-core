use axum::Json;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use systemprompt_identifiers::headers;
use systemprompt_models::auth::BEARER_PREFIX;
use systemprompt_oauth::services::{
    CoworkAuthResult, exchange_cowork_session_code, issue_cowork_access,
};
use systemprompt_runtime::AppContext;
use systemprompt_users::{ApiKeyService, DeviceCertService};

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub ttl: u64,
    pub headers: HashMap<String, String>,
}

impl From<CoworkAuthResult> for AuthResponse {
    fn from(r: CoworkAuthResult) -> Self {
        Self {
            token: r.token,
            ttl: r.ttl,
            headers: r.headers,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Capabilities {
    pub modes: Vec<&'static str>,
}

pub async fn capabilities() -> Json<Capabilities> {
    Json(Capabilities {
        modes: vec!["pat", "session", "mtls"],
    })
}

#[derive(Debug, Deserialize)]
pub struct MtlsRequestBody {
    pub device_cert_fingerprint: String,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SessionExchangeBody {
    pub code: String,
    #[serde(default)]
    pub session_id: Option<String>,
}

pub async fn pat(
    ctx: AppContext,
    request: Request,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    let pat_token = extract_bearer(request.headers()).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Missing Authorization: Bearer <pat>".into(),
        )
    })?;

    let service = ApiKeyService::new(ctx.db_pool())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let record = service
        .verify(&pat_token)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "Invalid PAT".into()))?;

    let result = issue_cowork_access(ctx.db_pool(), &record.user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(result.into()))
}

pub async fn session(
    ctx: AppContext,
    Json(body): Json<SessionExchangeBody>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    if body.code.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "missing exchange code".into()));
    }

    let result = exchange_cowork_session_code(ctx.db_pool(), body.code.trim())
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                "exchange code invalid, expired, or already consumed".into(),
            )
        })?;

    Ok(Json(result.into()))
}

pub async fn mtls(
    ctx: AppContext,
    Json(body): Json<MtlsRequestBody>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    let fingerprint = body.device_cert_fingerprint.trim();
    if fingerprint.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "missing device_cert_fingerprint".into(),
        ));
    }

    let service = DeviceCertService::new(ctx.db_pool())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let record = service
        .verify(fingerprint)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                "device certificate not enrolled or revoked".into(),
            )
        })?;

    let result = issue_cowork_access(ctx.db_pool(), &record.user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(result.into()))
}

fn extract_bearer(hdrs: &HeaderMap) -> Option<String> {
    let auth = hdrs.get(headers::AUTHORIZATION)?.to_str().ok()?;
    auth.strip_prefix(BEARER_PREFIX)
        .map(|s| s.trim().to_string())
}
