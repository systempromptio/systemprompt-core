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
use systemprompt_ai::repository::UserProviderCredentialMeta;
use systemprompt_ai::{
    PersonalKeyEntry, PersonalKeyError, PersonalProviderCredentials, personal_credential,
    personal_credentials,
};
use systemprompt_extension::router::GatewayPrincipal;
use systemprompt_manifest::services::catalog::{ResolvedCatalog, ResolvedCredential};
use systemprompt_models::bridge::profile::{
    ProviderCredentialStatus, SetProviderCredentialRequest,
};
use systemprompt_runtime::AppContext;

use super::bridge_error::{BridgeError, authenticate_bridge};
use crate::kernel::{ApiHttpError, JwtContextExtractor};

pub async fn list(
    jwt_extractor: Arc<JwtContextExtractor>,
    ctx: AppContext,
    headers: HeaderMap,
) -> Result<Json<Vec<ProviderCredentialStatus>>, ApiHttpError> {
    let caller = caller(&jwt_extractor, &headers).await?;
    let stored = credentials(&ctx)?
        .list(&caller)
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
    let caller = caller(&jwt_extractor, &headers).await?;
    let Json(body) = body.map_err(BridgeError::InvalidBody)?;
    let catalog = ctx.services_state().catalog();
    let resolved = personal_credential(catalog, &credential).map_err(key_error)?;
    let meta = credentials(&ctx)?
        .store(
            &caller,
            catalog,
            PersonalKeyEntry {
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
    let caller = caller(&jwt_extractor, &headers).await?;
    let resolved =
        personal_credential(ctx.services_state().catalog(), &credential).map_err(key_error)?;
    let removed = credentials(&ctx)?
        .remove(&caller, &resolved.id)
        .await
        .map_err(|e| BridgeError::internal("personal key could not be removed", e))?;
    if removed {
        tracing::info!(
            user_id = %caller.user_id(),
            credential = %resolved.id.as_str(),
            "Personal credential key removed"
        );
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn caller(
    jwt_extractor: &JwtContextExtractor,
    headers: &HeaderMap,
) -> Result<GatewayPrincipal, BridgeError> {
    let (_claims, user) = authenticate_bridge(jwt_extractor, headers).await?;
    Ok(GatewayPrincipal::__from_kernel(user.id))
}

fn credentials(ctx: &AppContext) -> Result<Arc<PersonalProviderCredentials>, BridgeError> {
    ctx.kernel()
        .state()
        .get::<Arc<PersonalProviderCredentials>>()
        .cloned()
        .map_err(|e| BridgeError::internal("the personal credential view is unavailable", e))
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
