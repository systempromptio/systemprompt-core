//! Admin API-key issuance and listing endpoints.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_identifiers::{ApiKeyId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_models::attribution::ScopeBinding;
use systemprompt_runtime::AppContext;
use systemprompt_security::authz::{AuthzHookContext, NullAuditSink, SubjectProviderSet};
use systemprompt_users::{ApiKeyLimits, ApiKeyService, IssueApiKeyParams, UserApiKey};

use crate::error::ApiHttpError;

pub(super) fn router() -> Router<AppContext> {
    Router::new()
        .route("/", post(issue_key).get(list_keys))
        .route("/{key_id}", delete(revoke_key))
}

#[derive(Debug, Deserialize)]
pub(super) struct IssueApiKeyRequest {
    pub name: String,
    #[serde(default)]
    pub target_user_id: Option<String>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default, flatten)]
    pub limits: ApiKeyLimits,
    #[serde(default)]
    pub scopes: Vec<ScopeBinding>,
}

#[derive(Debug, Serialize)]
pub(super) struct IssueApiKeyResponse {
    pub id: ApiKeyId,
    pub name: String,
    pub key_prefix: String,
    pub secret: String,
    pub created_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(flatten)]
    pub limits: ApiKeyLimits,
    pub scopes: Vec<ScopeBinding>,
}

#[derive(Debug, Serialize)]
pub(super) struct ApiKeyView {
    pub id: ApiKeyId,
    pub name: String,
    pub key_prefix: String,
    pub created_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    #[serde(flatten)]
    pub limits: ApiKeyLimits,
    pub scopes: Vec<ScopeBinding>,
}

impl From<UserApiKey> for ApiKeyView {
    fn from(k: UserApiKey) -> Self {
        Self {
            id: k.id,
            name: k.name,
            key_prefix: k.key_prefix,
            created_at: k.created_at,
            last_used_at: k.last_used_at,
            expires_at: k.expires_at,
            revoked_at: k.revoked_at,
            limits: k.limits,
            scopes: k.scopes,
        }
    }
}

async fn issue_key(
    State(ctx): State<AppContext>,
    Extension(req_ctx): Extension<RequestContext>,
    Json(body): Json<IssueApiKeyRequest>,
) -> Result<impl IntoResponse, ApiHttpError> {
    let target_user = match body.target_user_id.as_deref() {
        Some(value) if !value.is_empty() => UserId::try_new(value).map_err(ApiError::from)?,
        _ => req_ctx.user_id().clone(),
    };
    verify_scope_bindings(&ctx, &target_user, &body.scopes).await?;
    let service = ApiKeyService::new(Arc::clone(ctx.user_repository()));

    let issued = service
        .issue(IssueApiKeyParams {
            user_id: &target_user,
            name: &body.name,
            expires_at: body.expires_at,
            limits: &body.limits,
            scopes: &body.scopes,
        })
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(IssueApiKeyResponse {
            id: issued.record.id,
            name: issued.record.name,
            key_prefix: issued.record.key_prefix,
            secret: issued.secret,
            created_at: issued.record.created_at,
            expires_at: issued.record.expires_at,
            limits: issued.record.limits,
            scopes: issued.record.scopes,
        }),
    ))
}

async fn verify_scope_bindings(
    ctx: &AppContext,
    owner: &UserId,
    scopes: &[ScopeBinding],
) -> Result<(), ApiHttpError> {
    if scopes.is_empty() {
        return Ok(());
    }
    let providers = SubjectProviderSet::discover(&AuthzHookContext {
        pool: ctx.db_pool().pool(),
        sink: Arc::new(NullAuditSink),
    });
    for scope in scopes {
        let Some(provider) = providers.find(scope.dimension.as_str()) else {
            return Err(ApiHttpError::bad_request(format!(
                "unknown scope dimension '{}': no subject attribute provider registers it",
                scope.dimension
            )));
        };
        let held = provider.values_for(owner).await?;
        if !held.iter().any(|value| value == &scope.value) {
            return Err(ApiHttpError::forbidden(format!(
                "the key owner is not a member of {} '{}'",
                scope.dimension, scope.value
            )));
        }
    }
    Ok(())
}

async fn list_keys(
    State(ctx): State<AppContext>,
    Extension(req_ctx): Extension<RequestContext>,
) -> Result<Json<Vec<ApiKeyView>>, ApiHttpError> {
    let service = ApiKeyService::new(Arc::clone(ctx.user_repository()));

    let keys = service.list_for_user(req_ctx.user_id()).await?;

    Ok(Json(keys.into_iter().map(ApiKeyView::from).collect()))
}

async fn revoke_key(
    State(ctx): State<AppContext>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(key_id): Path<String>,
) -> Result<StatusCode, ApiHttpError> {
    let service = ApiKeyService::new(Arc::clone(ctx.user_repository()));

    let id = ApiKeyId::try_new(key_id).map_err(ApiError::from)?;
    let revoked = service.revoke(&id, req_ctx.user_id()).await?;

    if revoked {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiHttpError::not_found("API key not found"))
    }
}
