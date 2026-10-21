//! Typed failures of the bridge routes, their single mapping onto the unified
//! `ApiError` envelope, and the bridge credential check the routes share.
//!
//! A rejected credential answers 401 with a fixed message; the decoder's cause
//! travels as the error source, so it reaches the log and never the client.
//! Server-side failures carry a static context and their cause, and leave
//! through `ApiError`'s fixed 5xx text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::rejection::JsonRejection;
use axum::http::HeaderMap;
use systemprompt_identifiers::{JwtToken, ManagedResourceId, UserId};
use systemprompt_marketplace::managed::ManagedError;
use systemprompt_models::api::ApiError;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_oauth::OauthError;
use systemprompt_traits::{AuthUser, BoxedSource};
use systemprompt_users::UserError;

use super::messages::extract_credential;
use crate::error::ApiHttpError;
use crate::services::middleware::JwtContextExtractor;
use crate::services::middleware::jwt::JwtUserContext;

#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("Missing Authorization or x-api-key credential")]
    MissingCredential,
    #[error("Bridge credential rejected")]
    CredentialRejected(#[source] ContextExtractionError),
    #[error("heartbeat session_id must match the authenticated session")]
    SessionMismatch,
    #[error("invalid request body: {0}")]
    InvalidBody(#[source] JsonRejection),
    #[error("unknown API surface: {0}")]
    UnknownSurface(String),
    #[error("host '{0}' is disabled on this installation")]
    HostDisabled(HostKind),
    #[error("host '{0}' is the last enabled host; at least one must stay enabled")]
    LastHostDisabled(HostKind),
    #[error("Gateway not enabled")]
    GatewayDisabled,
    #[error("provider '{0}' does not take a personal key")]
    NotPersonalProvider(String),
    #[error("{0} rejected the key; check it was copied whole and the subscription is active")]
    PersonalKeyRejected(String),
    #[error("{provider} could not verify the key (upstream answered {status}); try again shortly")]
    PersonalKeyUnverified { provider: String, status: String },
    #[error("User not found: {0}")]
    UserNotFound(UserId),
    #[error("Invalid path")]
    InvalidPath,
    #[error("Plugin not found")]
    PluginNotFound,
    #[error("File not found")]
    FileNotFound,
    #[error("Plugin archive is not the current revision")]
    StaleArchive,
    #[error("{0}")]
    DeviceForeignUser(String),
    #[error("{0}")]
    DeviceRejected(String),
    #[error("manifest: catalogue grant not recorded for {resource}")]
    CatalogGrant {
        resource: ManagedResourceId,
        #[source]
        source: ManagedError,
    },
    #[error("manifest: {count} catalogue grants not recorded")]
    CatalogGrants {
        count: usize,
        #[source]
        source: ManagedError,
    },
    #[error("{context}")]
    Unavailable {
        context: &'static str,
        #[source]
        source: BoxedSource,
    },
    #[error("{context}")]
    Internal {
        context: &'static str,
        #[source]
        source: BoxedSource,
    },
    #[error(transparent)]
    Users(#[from] UserError),
    #[error(transparent)]
    Oauth(#[from] OauthError),
}

impl BridgeError {
    pub fn internal(context: &'static str, source: impl Into<BoxedSource>) -> Self {
        Self::Internal {
            context,
            source: source.into(),
        }
    }

    pub fn unavailable(context: &'static str, source: impl Into<BoxedSource>) -> Self {
        Self::Unavailable {
            context,
            source: source.into(),
        }
    }
}

impl From<BridgeError> for ApiHttpError {
    fn from(err: BridgeError) -> Self {
        let message = err.to_string();
        let api = match err {
            BridgeError::MissingCredential => {
                ApiError::unauthorized(message).with_error_key("missing_credential")
            },
            BridgeError::CredentialRejected(source) => ApiError::unauthorized(message)
                .with_error_key("invalid_credential")
                .with_source(source),
            BridgeError::SessionMismatch => {
                ApiError::unauthorized(message).with_error_key("session_mismatch")
            },
            BridgeError::InvalidBody(_) => {
                ApiError::bad_request(message).with_error_key("invalid_body")
            },
            BridgeError::UnknownSurface(_) => {
                ApiError::bad_request(message).with_error_key("unknown_api_surface")
            },
            BridgeError::HostDisabled(_) => {
                ApiError::validation_error(message, Vec::new()).with_error_key("host_disabled")
            },
            BridgeError::LastHostDisabled(_) => {
                ApiError::conflict(message).with_error_key("last_host_disabled")
            },
            BridgeError::GatewayDisabled => {
                ApiError::not_found(message).with_error_key("gateway_disabled")
            },
            BridgeError::NotPersonalProvider(_) => {
                ApiError::not_found(message).with_error_key("not_personal_provider")
            },
            BridgeError::PersonalKeyRejected(_) => ApiError::validation_error(message, Vec::new())
                .with_error_key("personal_key_rejected"),
            BridgeError::PersonalKeyUnverified { .. } => {
                ApiError::bad_request(message).with_error_key("personal_key_unverified")
            },
            BridgeError::UserNotFound(_)
            | BridgeError::PluginNotFound
            | BridgeError::FileNotFound => ApiError::not_found(message),
            BridgeError::InvalidPath | BridgeError::DeviceRejected(_) => {
                ApiError::bad_request(message)
            },
            BridgeError::StaleArchive => {
                ApiError::conflict(message).with_error_key("stale_plugin_archive")
            },
            BridgeError::DeviceForeignUser(_) => {
                ApiError::conflict(message).with_error_key("device_fingerprint_foreign_user")
            },
            err @ (BridgeError::CatalogGrant { .. } | BridgeError::CatalogGrants { .. }) => {
                ApiError::internal("manifest: catalogue grant not recorded", err)
            },
            BridgeError::Unavailable { context, source } => {
                ApiError::service_unavailable(context).with_source(source)
            },
            BridgeError::Internal { context, source } => ApiError::internal(context, source),
            BridgeError::Users(inner) => return Self::from(inner),
            BridgeError::Oauth(inner) => return Self::from(inner),
        };
        Self::from(api)
    }
}

pub async fn authenticate_bridge(
    jwt_extractor: &JwtContextExtractor,
    headers: &HeaderMap,
) -> Result<(JwtUserContext, AuthUser), BridgeError> {
    let credential = extract_credential(headers).ok_or(BridgeError::MissingCredential)?;
    jwt_extractor
        .decode_for_gateway(&JwtToken::new(credential))
        .await
        .map_err(BridgeError::CredentialRejected)
}
