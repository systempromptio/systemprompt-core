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
//! The caller is verified through the request authenticator; the credential
//! view is the one conferred on `gateway-http`.
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
use systemprompt_extension::router::{ApiHttpError, unreadable_body};
use systemprompt_identifiers::JwtToken;
use systemprompt_manifest::services::catalog::{ResolvedCatalog, ResolvedCredential};
use systemprompt_models::api::ApiError;
use systemprompt_models::bridge::profile::{
    ProviderCredentialStatus, SetProviderCredentialRequest,
};
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_traits::BoxedSource;

use super::state::GatewayHttpState;
use systemprompt_gateway::surface::credential::extract_credential;

/// The personal-credential routes' state: the gateway state and the
/// caller-bound credential view conferred on `gateway-http`.
#[derive(Clone)]
pub struct ProviderCredentialsState {
    pub gateway: GatewayHttpState,
    pub credentials: Arc<PersonalProviderCredentials>,
}

impl std::fmt::Debug for ProviderCredentialsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderCredentialsState")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderCredentialsError {
    #[error("Missing Authorization or x-api-key credential")]
    MissingCredential,
    #[error("Bridge credential rejected")]
    CredentialRejected(#[source] ContextExtractionError),
    #[error("invalid request body: {0}")]
    InvalidBody(#[source] JsonRejection),
    #[error("credential '{0}' does not take a personal key")]
    NotPersonalCredential(String),
    #[error("{0} rejected the key; check it was copied whole and the subscription is active")]
    PersonalKeyRejected(String),
    #[error(
        "{credential} could not verify the key (upstream answered {status}); try again shortly"
    )]
    PersonalKeyUnverified { credential: String, status: String },
    #[error("{context}")]
    Internal {
        context: &'static str,
        #[source]
        source: BoxedSource,
    },
}

impl ProviderCredentialsError {
    fn internal(context: &'static str, source: impl Into<BoxedSource>) -> Self {
        Self::Internal {
            context,
            source: source.into(),
        }
    }
}

impl From<ProviderCredentialsError> for ApiHttpError {
    fn from(err: ProviderCredentialsError) -> Self {
        let message = err.to_string();
        let api = match err {
            ProviderCredentialsError::MissingCredential => {
                ApiError::unauthorized(message).with_error_key("missing_credential")
            },
            ProviderCredentialsError::CredentialRejected(source) => ApiError::unauthorized(message)
                .with_error_key("invalid_credential")
                .with_source(source),
            ProviderCredentialsError::InvalidBody(source) => {
                unreadable_body(source.status(), &message, source).with_error_key("invalid_body")
            },
            ProviderCredentialsError::NotPersonalCredential(_) => {
                ApiError::not_found(message).with_error_key("not_personal_credential")
            },
            ProviderCredentialsError::PersonalKeyRejected(_) => {
                ApiError::validation_error(message, Vec::new())
                    .with_error_key("personal_key_rejected")
            },
            ProviderCredentialsError::PersonalKeyUnverified { .. } => {
                ApiError::bad_request(message).with_error_key("personal_key_unverified")
            },
            ProviderCredentialsError::Internal { context, source } => {
                ApiError::internal(context, source)
            },
        };
        Self::from(api)
    }
}

pub async fn list(
    state: ProviderCredentialsState,
    headers: HeaderMap,
) -> Result<Json<Vec<ProviderCredentialStatus>>, ApiHttpError> {
    let caller = caller(&state.gateway, &headers).await?;
    let stored = state
        .credentials
        .list(&caller)
        .await
        .map_err(|e| ProviderCredentialsError::internal("personal key listing failed", e))?;
    let catalog = state.gateway.host.services.catalog();
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
    state: ProviderCredentialsState,
    headers: HeaderMap,
    Path(credential): Path<String>,
    body: Result<Json<SetProviderCredentialRequest>, JsonRejection>,
) -> Result<Json<ProviderCredentialStatus>, ApiHttpError> {
    let caller = caller(&state.gateway, &headers).await?;
    let Json(body) = body.map_err(ProviderCredentialsError::InvalidBody)?;
    let catalog = state.gateway.host.services.catalog();
    let resolved = personal_credential(catalog, &credential).map_err(key_error)?;
    let meta = state
        .credentials
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
    state: ProviderCredentialsState,
    headers: HeaderMap,
    Path(credential): Path<String>,
) -> Result<StatusCode, ApiHttpError> {
    let caller = caller(&state.gateway, &headers).await?;
    let resolved = personal_credential(state.gateway.host.services.catalog(), &credential)
        .map_err(key_error)?;
    let removed = state
        .credentials
        .remove(&caller, &resolved.id)
        .await
        .map_err(|e| ProviderCredentialsError::internal("personal key could not be removed", e))?;
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
    state: &GatewayHttpState,
    headers: &HeaderMap,
) -> Result<GatewayPrincipal, ProviderCredentialsError> {
    let credential =
        extract_credential(headers).ok_or(ProviderCredentialsError::MissingCredential)?;
    let (_claims, user) = state
        .authenticator
        .decode_for_gateway(&JwtToken::new(credential))
        .await
        .map_err(ProviderCredentialsError::CredentialRejected)?;
    Ok(GatewayPrincipal::__from_kernel(user.id))
}

fn key_error(error: PersonalKeyError) -> ProviderCredentialsError {
    match error {
        PersonalKeyError::NotPersonalCredential(id) => {
            ProviderCredentialsError::NotPersonalCredential(id)
        },
        PersonalKeyError::Rejected(credential) => {
            ProviderCredentialsError::PersonalKeyRejected(credential)
        },
        PersonalKeyError::Unverified { credential, status } => {
            ProviderCredentialsError::PersonalKeyUnverified { credential, status }
        },
        PersonalKeyError::Internal(source) => {
            ProviderCredentialsError::internal("personal key could not be stored", source)
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

pub(super) fn routes(state: &ProviderCredentialsState) -> Router {
    let (list_state, set_state, delete_state) = (state.clone(), state.clone(), state.clone());
    Router::new()
        .route(
            "/me/provider-credentials",
            get(move |headers| {
                let state = list_state.clone();
                async move { list(state, headers).await }
            }),
        )
        .route(
            "/me/provider-credentials/{credential}",
            put(move |headers, path, body| {
                let state = set_state.clone();
                async move { set(state, headers, path, body).await }
            })
            .delete(move |headers, path| {
                let state = delete_state.clone();
                async move { delete(state, headers, path).await }
            }),
        )
}
