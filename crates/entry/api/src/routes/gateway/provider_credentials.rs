//! `/v1/me/provider-credentials` — the caller's personal provider keys.
//!
//! A provider declared `credential: user` is served only from a key the
//! calling user stored for themselves. These routes list which such providers
//! the caller has a key for, store one (after one cheap upstream call proves it
//! works), and remove it. The key is sealed at rest and never returned, logged
//! or echoed in an error; every store and removal is recorded with the user and
//! provider only.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::routing::{get, put};
use axum::{Json, Router};
use axum::extract::Path;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderMap, StatusCode};
use systemprompt_ai::repository::{UserProviderCredentialMeta, UserProviderCredentialRepository};
use systemprompt_ai::{
    PersonalKeyError, personal_provider, personal_providers, store_personal_key,
};
use systemprompt_loader::ServicesBootstrap;
use systemprompt_manifest::services::ProviderEntry;
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
    let rows = personal_providers_now()?
        .iter()
        .map(|entry| {
            let meta = stored.iter().find(|m| m.provider == entry.name.as_str());
            status_row(entry, meta)
        })
        .collect();
    Ok(Json(rows))
}

pub async fn set(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
    Path(provider): Path<String>,
    body: Result<Json<SetProviderCredentialRequest>, JsonRejection>,
) -> Result<Json<ProviderCredentialStatus>, ApiHttpError> {
    let (claims, _user) = authenticate_bridge(&jwt_extractor, &headers).await?;
    let Json(body) = body.map_err(BridgeError::InvalidBody)?;
    let services = ServicesBootstrap::get()
        .map_err(|e| BridgeError::unavailable("services config not ready", e))?;
    let entry = personal_provider(&services.providers, &provider).map_err(key_error)?;
    let meta = store_personal_key(
        repository(&ctx),
        &services.providers,
        &claims.user_id,
        &provider,
        &body.api_key,
        body.label.as_deref(),
    )
    .await
    .map_err(key_error)?;
    Ok(Json(status_row(&entry, Some(&meta))))
}

pub async fn delete(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
    Path(provider): Path<String>,
) -> Result<StatusCode, ApiHttpError> {
    let (claims, _user) = authenticate_bridge(&jwt_extractor, &headers).await?;
    let services = ServicesBootstrap::get()
        .map_err(|e| BridgeError::unavailable("services config not ready", e))?;
    let entry = personal_provider(&services.providers, &provider).map_err(key_error)?;
    let removed = repository(&ctx)
        .delete_key(&claims.user_id, entry.name.as_str())
        .await
        .map_err(|e| BridgeError::internal("personal key could not be removed", e))?;
    if removed {
        tracing::info!(
            user_id = %claims.user_id,
            provider = %entry.name.as_str(),
            "Personal provider key removed"
        );
    }
    Ok(StatusCode::NO_CONTENT)
}

fn repository(ctx: &AppContext) -> &UserProviderCredentialRepository {
    &ctx.ai_repositories().user_provider_credentials
}

fn personal_providers_now() -> Result<Vec<ProviderEntry>, BridgeError> {
    let services = ServicesBootstrap::get()
        .map_err(|e| BridgeError::unavailable("services config not ready", e))?;
    Ok(personal_providers(&services.providers))
}

fn key_error(error: PersonalKeyError) -> BridgeError {
    match error {
        PersonalKeyError::NotPersonalProvider(name) => BridgeError::NotPersonalProvider(name),
        PersonalKeyError::Rejected(provider) => BridgeError::PersonalKeyRejected(provider),
        PersonalKeyError::Unverified { provider, status } => {
            BridgeError::PersonalKeyUnverified { provider, status }
        },
        PersonalKeyError::Internal(source) => {
            BridgeError::internal("personal key could not be stored", source)
        },
    }
}

fn status_row(
    entry: &ProviderEntry,
    meta: Option<&UserProviderCredentialMeta>,
) -> ProviderCredentialStatus {
    ProviderCredentialStatus {
        provider: entry.name.as_str().to_owned(),
        display_name: entry.display_name.clone(),
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
            "/me/provider-credentials/{provider}",
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
