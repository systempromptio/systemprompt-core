use axum::http::StatusCode;
use systemprompt_identifiers::{JwtToken, TenantId, TraceId, UserId};
use systemprompt_runtime::AppContext;
use systemprompt_users::{API_KEY_PREFIX, ApiKeyService};

use crate::services::middleware::JwtContextExtractor;

pub(super) struct AuthedPrincipal {
    pub user_id: UserId,
    pub tenant_id: Option<TenantId>,
    pub trace_id: Option<TraceId>,
    pub roles: Vec<String>,
    pub department: String,
}

pub(super) async fn authenticate(
    credential: &str,
    jwt_extractor: &JwtContextExtractor,
    ctx: &AppContext,
    tenant_id: Option<TenantId>,
) -> Result<AuthedPrincipal, (StatusCode, String)> {
    if credential.starts_with(API_KEY_PREFIX) {
        return authenticate_api_key(credential, ctx, tenant_id).await;
    }
    authenticate_jwt(credential, jwt_extractor, tenant_id).await
}

async fn authenticate_api_key(
    credential: &str,
    ctx: &AppContext,
    tenant_id: Option<TenantId>,
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
            tenant_id,
            trace_id: Some(TraceId::generate()),
            roles: Vec::new(),
            department: String::new(),
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
    header_tenant_id: Option<TenantId>,
) -> Result<AuthedPrincipal, (StatusCode, String)> {
    let jwt_token = JwtToken::new(credential);
    let claims = jwt_extractor
        .decode_for_gateway(&jwt_token)
        .await
        .map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;

    Ok(AuthedPrincipal {
        user_id: claims.user_id,
        tenant_id: header_tenant_id,
        trace_id: Some(TraceId::generate()),
        roles: claims.roles,
        department: claims.department.unwrap_or_else(String::new),
    })
}
