use axum::http::StatusCode;
use systemprompt_identifiers::{Actor, JwtToken, SessionId, TraceId, UserId};
use systemprompt_runtime::AppContext;
use systemprompt_users::{API_KEY_PREFIX, ApiKeyService};

use crate::services::middleware::JwtContextExtractor;

pub(super) struct AuthedPrincipal {
    pub user_id: UserId,
    pub trace_id: Option<TraceId>,
    pub roles: Vec<String>,
    pub department: String,
    pub act_chain: Vec<Actor>,
    // None for API-key credentials (not session-bound). Gateways MUST refuse a
    // request whose X-Session-ID header disagrees with this value.
    pub jwt_session_id: Option<SessionId>,
}

pub(super) async fn authenticate(
    credential: &str,
    jwt_extractor: &JwtContextExtractor,
    ctx: &AppContext,
) -> Result<AuthedPrincipal, (StatusCode, String)> {
    if credential.starts_with(API_KEY_PREFIX) {
        return authenticate_api_key(credential, ctx).await;
    }
    authenticate_jwt(credential, jwt_extractor, ctx).await
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
            jwt_session_id: None,
        }),
        None => Err((
            StatusCode::UNAUTHORIZED,
            "Invalid or revoked API key".to_owned(),
        )),
    }
}

async fn authenticate_jwt(
    credential: &str,
    jwt_extractor: &JwtContextExtractor,
    ctx: &AppContext,
) -> Result<AuthedPrincipal, (StatusCode, String)> {
    let jwt_token = JwtToken::new(credential);
    let claims = jwt_extractor
        .decode_for_gateway(&jwt_token)
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    let repo = systemprompt_users::UserRepository::new(ctx.db_pool())
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let user = repo
        .find_by_id(&claims.user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                format!("User not found: {}", claims.user_id.as_str()),
            )
        })?;

    Ok(AuthedPrincipal {
        user_id: claims.user_id,
        trace_id: Some(TraceId::generate()),
        roles: user.roles,
        department: String::new(),
        act_chain: claims.act_chain,
        jwt_session_id: Some(claims.session_id),
    })
}
