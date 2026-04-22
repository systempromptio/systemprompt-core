use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ApiKeyId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_runtime::AppContext;
use systemprompt_users::{ApiKeyService, IssueApiKeyParams, UserApiKey};

pub fn router() -> Router<AppContext> {
    Router::new()
        .route("/", post(issue_key).get(list_keys))
        .route("/{key_id}", delete(revoke_key))
}

#[derive(Debug, Deserialize)]
pub struct IssueApiKeyRequest {
    pub name: String,
    #[serde(default)]
    pub target_user_id: Option<String>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct IssueApiKeyResponse {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    pub secret: String,
    pub created_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct ApiKeyView {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    pub created_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl From<UserApiKey> for ApiKeyView {
    fn from(k: UserApiKey) -> Self {
        Self {
            id: k.id.as_str().to_string(),
            name: k.name,
            key_prefix: k.key_prefix,
            created_at: k.created_at,
            last_used_at: k.last_used_at,
            expires_at: k.expires_at,
            revoked_at: k.revoked_at,
        }
    }
}

async fn issue_key(
    State(ctx): State<AppContext>,
    Extension(req_ctx): Extension<RequestContext>,
    Json(body): Json<IssueApiKeyRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let target_user = resolve_target_user(&req_ctx, body.target_user_id.as_deref())?;
    let service = ApiKeyService::new(ctx.db_pool())
        .map_err(|e| ApiError::internal_error(format!("API key service error: {e}")))?;

    let issued = service
        .issue(IssueApiKeyParams {
            user_id: &target_user,
            name: &body.name,
            expires_at: body.expires_at,
        })
        .await
        .map_err(|e| ApiError::internal_error(format!("Failed to issue API key: {e}")))?;

    Ok((
        StatusCode::CREATED,
        Json(IssueApiKeyResponse {
            id: issued.record.id.as_str().to_string(),
            name: issued.record.name,
            key_prefix: issued.record.key_prefix,
            secret: issued.secret,
            created_at: issued.record.created_at,
            expires_at: issued.record.expires_at,
        }),
    ))
}

async fn list_keys(
    State(ctx): State<AppContext>,
    Extension(req_ctx): Extension<RequestContext>,
) -> Result<Json<Vec<ApiKeyView>>, ApiError> {
    let service = ApiKeyService::new(ctx.db_pool())
        .map_err(|e| ApiError::internal_error(format!("API key service error: {e}")))?;

    let keys = service
        .list_for_user(req_ctx.user_id())
        .await
        .map_err(|e| ApiError::internal_error(format!("Failed to list API keys: {e}")))?;

    Ok(Json(keys.into_iter().map(ApiKeyView::from).collect()))
}

async fn revoke_key(
    State(ctx): State<AppContext>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(key_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let service = ApiKeyService::new(ctx.db_pool())
        .map_err(|e| ApiError::internal_error(format!("API key service error: {e}")))?;

    let id = ApiKeyId::new(key_id);
    let revoked = service
        .revoke(&id, req_ctx.user_id())
        .await
        .map_err(|e| ApiError::internal_error(format!("Failed to revoke API key: {e}")))?;

    if revoked {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("API key not found"))
    }
}

#[allow(clippy::result_large_err)]
fn resolve_target_user(
    req_ctx: &RequestContext,
    override_user_id: Option<&str>,
) -> Result<UserId, ApiError> {
    match override_user_id {
        Some(value) if !value.is_empty() => {
            if req_ctx.user_type() != systemprompt_models::auth::UserType::Admin {
                return Err(ApiError::forbidden(
                    "Only admins can issue keys for other users",
                ));
            }
            Ok(UserId::new(value.to_string()))
        },
        _ => Ok(req_ctx.user_id().clone()),
    }
}
