//! `/v1/me/provider-credentials` — the caller's personal credential keys.
//!
//! A personal catalog credential (`services/ai/credentials.yaml`,
//! `kind: personal`) is served only from a key the calling user stored for
//! themselves, one per credential, covering every endpoint that credential
//! authenticates. These routes list the personal credentials the catalog
//! serves with whether the caller has a key for each, store one (after one
//! cheap upstream call to every endpoint proves it works), and remove it. The
//! key is sealed at rest and never returned, logged or echoed in an error;
//! every store and removal is recorded with the user and credential only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::extract::Path;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, put};
use axum::{Json, Router};
use systemprompt_ai::repository::{UserProviderCredentialMeta, UserProviderCredentialRepository};
use systemprompt_ai::{
    PersonalKeyError, PersonalKeyRequest, personal_credential, personal_credentials,
    store_personal_key,
};
use systemprompt_manifest::services::catalog::{ResolvedCatalog, ResolvedCredential};
use systemprompt_models::bridge::profile::{
    ProviderCredentialStatus, SetProviderCredentialRequest,
};
use systemprompt_runtime::AppContext;

use super::bridge_error::{BridgeError, authenticate_bridge};
use crate::error::ApiHttpError;
use crate::services::middleware::JwtContextExtractor;

pub async fn list(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
) -> Result<Json<Vec<ProviderCredentialStatus>>, ApiHttpError> {
    let (claims, _user) = authenticate_bridge(&jwt_extractor, &headers).await?;
    let stored = repository(&ctx)
        .list_metadata(&claims.user_id)
        .await
        .map_err(|e| BridgeError::internal("personal key listing failed", e))?;
    let catalog = ctx.services_state().catalog();
    let rows = personal_credentials(catalog)
        .into_iter()
        .map(|credential| {
            let meta = stored
                .iter()
                .find(|m| m.credential == credential.id.as_str());
            status_row(catalog, credential, meta)
        })
        .collect();
    Ok(Json(rows))
}

pub async fn set(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
    Path(credential): Path<String>,
    body: Result<Json<SetProviderCredentialRequest>, JsonRejection>,
) -> Result<Json<ProviderCredentialStatus>, ApiHttpError> {
    let (claims, _user) = authenticate_bridge(&jwt_extractor, &headers).await?;
    let Json(body) = body.map_err(BridgeError::InvalidBody)?;
    let catalog = ctx.services_state().catalog();
    let resolved = personal_credential(catalog, &credential).map_err(key_error)?;
    let meta = store_personal_key(
        repository(&ctx),
        catalog,
        PersonalKeyRequest {
            user_id: &claims.user_id,
            credential: &credential,
            key: &body.api_key,
            label: body.label.as_deref(),
        },
    )
    .await
    .map_err(key_error)?;
    Ok(Json(status_row(catalog, resolved, Some(&meta))))
}

pub async fn delete(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
    Path(credential): Path<String>,
) -> Result<StatusCode, ApiHttpError> {
    let (claims, _user) = authenticate_bridge(&jwt_extractor, &headers).await?;
    let resolved =
        personal_credential(ctx.services_state().catalog(), &credential).map_err(key_error)?;
    let removed = repository(&ctx)
        .delete_key(&claims.user_id, &resolved.id)
        .await
        .map_err(|e| BridgeError::internal("personal key could not be removed", e))?;
    if removed {
        tracing::info!(
            user_id = %claims.user_id,
            credential = %resolved.id.as_str(),
            "Personal credential key removed"
        );
    }
    Ok(StatusCode::NO_CONTENT)
}

fn repository(ctx: &AppContext) -> &UserProviderCredentialRepository {
    &ctx.ai_repositories().user_provider_credentials
}

fn key_error(error: PersonalKeyError) -> BridgeError {
    match error {
        PersonalKeyError::NotPersonalCredential(id) => BridgeError::NotPersonalCredential(id),
        PersonalKeyError::Rejected(credential) => BridgeError::PersonalKeyRejected(credential),
        PersonalKeyError::Unverified { credential, status } => {
            BridgeError::PersonalKeyUnverified { credential, status }
        },
        PersonalKeyError::Internal(source) => {
            BridgeError::internal("personal key could not be stored", source)
        },
    }
}

fn status_row(
    catalog: &ResolvedCatalog,
    credential: &ResolvedCredential,
    meta: Option<&UserProviderCredentialMeta>,
) -> ProviderCredentialStatus {
    let display_name = catalog
        .providers()
        .find(|provider| provider.credential == credential.id)
        .and_then(|provider| provider.display_name.clone())
        .or_else(|| credential.label.clone());
    ProviderCredentialStatus {
        credential: credential.id.as_str().to_owned(),
        display_name,
        configured: meta.is_some(),
        label: meta.and_then(|m| m.label.clone()),
        created_at: meta.map(|m| m.created_at),
        verified_at: meta.and_then(|m| m.verified_at),
        last_used_at: meta.and_then(|m| m.last_used_at),
    }
}

pub(super) fn routes(ctx: &AppContext, jwt_extractor: &Arc<JwtContextExtractor>) -> Router {
    let ctx_list = ctx.clone();
    let ctx_set = ctx.clone();
    let ctx_delete = ctx.clone();
    let jwt_list = Arc::clone(jwt_extractor);
    let jwt_set = Arc::clone(jwt_extractor);
    let jwt_delete = Arc::clone(jwt_extractor);
    Router::new()
        .route(
            "/me/provider-credentials",
            get(move |headers| {
                let extractor = Arc::clone(&jwt_list);
                let context = ctx_list.clone();
                async move { list(extractor, context, headers).await }
            }),
        )
        .route(
            "/me/provider-credentials/{credential}",
            put(move |headers, path, body| {
                let extractor = Arc::clone(&jwt_set);
                let context = ctx_set.clone();
                async move { set(extractor, context, headers, path, body).await }
            })
            .delete(move |headers, path| {
                let extractor = Arc::clone(&jwt_delete);
                let context = ctx_delete.clone();
                async move { delete(extractor, context, headers, path).await }
            }),
        )
}
