use axum::http::StatusCode;
use systemprompt_identifiers::{Actor, JwtToken, TraceId, UserId};
use systemprompt_runtime::AppContext;
use systemprompt_users::{API_KEY_PREFIX, ApiKeyService};

use crate::services::middleware::JwtContextExtractor;

pub(super) struct AuthedPrincipal {
    pub user_id: UserId,
    pub trace_id: Option<TraceId>,
    pub roles: Vec<String>,
    pub department: String,
    pub act_chain: Vec<Actor>,
}

pub(super) async fn authenticate(
    credential: &str,
    jwt_extractor: &JwtContextExtractor,
    ctx: &AppContext,
) -> Result<AuthedPrincipal, (StatusCode, String)> {
    if credential.starts_with(API_KEY_PREFIX) {
        return authenticate_api_key(credential, ctx).await;
    }
    authenticate_jwt(credential, jwt_extractor).await
}

async fn authenticate_api_key(
    credential: &str,
    ctx: &AppContext,
) -> Result<AuthedPrincipal, (StatusCode, String)> {
    let service = ApiKeyService::new(ctx.db_pool()).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("API key service unavailable: {e}"),
        )
    })?;
    let record = service.verify(credential).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("API key verification failed: {e}"),
        )
    })?;
    match record {
        Some(rec) => Ok(AuthedPrincipal {
            user_id: rec.user_id,
            trace_id: Some(TraceId::generate()),
            roles: Vec::new(),
            department: String::new(),
            act_chain: Vec::new(),
        }),
        None => Err((
            StatusCode::UNAUTHORIZED,
            "Invalid or revoked API key".to_string(),
        )),
    }
}

async fn authenticate_jwt(
    credential: &str,
    jwt_extractor: &JwtContextExtractor,
) -> Result<AuthedPrincipal, (StatusCode, String)> {
    let jwt_token = JwtToken::new(credential);
    let claims = jwt_extractor
        .decode_for_gateway(&jwt_token)
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    Ok(AuthedPrincipal {
        user_id: claims.user_id,
        trace_id: Some(TraceId::generate()),
        roles: claims.roles,
        department: claims.department.unwrap_or_else(String::new),
        act_chain: claims.act_chain,
    })
}
