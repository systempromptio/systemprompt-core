use axum::Json;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode};
use serde::Serialize;
use std::collections::HashMap;
use systemprompt_identifiers::headers;
use systemprompt_models::auth::BEARER_PREFIX;
use systemprompt_oauth::services::{CoworkAuthResult, issue_cowork_access};
use systemprompt_runtime::AppContext;
use systemprompt_users::ApiKeyService;

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
    Json(Capabilities { modes: vec!["pat"] })
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
    _ctx: AppContext,
    _request: Request,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    Err((
        StatusCode::NOT_IMPLEMENTED,
        "Dashboard-session auth not yet wired. Use PAT provider.".into(),
    ))
}

pub async fn mtls(
    _ctx: AppContext,
    _request: Request,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    Err((
        StatusCode::NOT_IMPLEMENTED,
        "mTLS device-cert auth not yet wired. Use PAT provider.".into(),
    ))
}

fn extract_bearer(hdrs: &HeaderMap) -> Option<String> {
    let auth = hdrs.get(headers::AUTHORIZATION)?.to_str().ok()?;
    auth.strip_prefix(BEARER_PREFIX)
        .map(|s| s.trim().to_string())
}
