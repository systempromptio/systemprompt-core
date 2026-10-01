//! `From` impls mapping domain and repository errors onto [`ApiHttpError`],
//! keeping the variant-to-HTTP-status mapping in one place so non-OAuth
//! handlers use `?`. `RepositoryError` already classifies into [`ApiError`] in
//! `systemprompt-models`; that impl is reused here. The umbrella domain errors
//! are classified by variant so that, e.g., a repository failure surfaces as
//! 500 while a missing entity surfaces as 404.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_agent::{AgentError, ProtocolError};
use systemprompt_loader::BundleError;
use systemprompt_marketplace::MarketplaceError;
use systemprompt_marketplace::managed::ManagedError;
use systemprompt_models::api::ApiError;
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_oauth::services::SessionCreationError;
use systemprompt_oauth::{OauthError, OauthErrorKind};
use systemprompt_traits::RepositoryError;
use systemprompt_users::UserError;

use super::{ApiHttpError, internal_api_error};

impl From<RepositoryError> for ApiHttpError {
    fn from(err: RepositoryError) -> Self {
        Self(ApiError::from(err))
    }
}

impl From<AgentError> for ApiHttpError {
    fn from(err: AgentError) -> Self {
        let api = match err {
            AgentError::NotFound(msg) => ApiError::not_found(msg),
            AgentError::Validation(msg)
            | AgentError::Protocol(ProtocolError::ValidationFailed(msg)) => {
                ApiError::bad_request(msg)
            },
            other => internal_api_error("Agent operation failed", &other),
        };
        Self(api)
    }
}

impl From<MarketplaceError> for ApiHttpError {
    fn from(err: MarketplaceError) -> Self {
        let err = match err {
            MarketplaceError::Managed(ManagedError::Repository(inner)) => {
                return Self(ApiError::from(inner));
            },
            other => other,
        };
        let api = match &err {
            MarketplaceError::NotFound(_)
            | MarketplaceError::NoDefault
            | MarketplaceError::Managed(ManagedError::Unavailable) => {
                ApiError::not_found(err.to_string())
            },
            MarketplaceError::Validation(_)
            | MarketplaceError::Managed(
                ManagedError::Invalid(_) | ManagedError::InvalidInput { .. },
            ) => ApiError::bad_request(err.to_string()),
            MarketplaceError::Managed(ManagedError::Conflict(_)) => {
                ApiError::conflict(err.to_string())
            },
            MarketplaceError::Catalog(_)
            | MarketplaceError::CatalogSource { .. }
            | MarketplaceError::Managed(_)
            | MarketplaceError::Import { .. }
            | MarketplaceError::ImportSource { .. }
            | MarketplaceError::Signing(_)
            | MarketplaceError::Filter(_) => {
                internal_api_error("Marketplace operation failed", &err)
            },
        };
        Self(api)
    }
}

impl From<UserError> for ApiHttpError {
    fn from(err: UserError) -> Self {
        let message = err.to_string();
        let api = match err {
            UserError::Repository(inner) => ApiError::from(inner),
            UserError::NotFound(_) => ApiError::not_found(message),
            UserError::EmailAlreadyExists(_) => ApiError::conflict(message),
            UserError::Validation(_)
            | UserError::InvalidStatus(_)
            | UserError::InvalidRole(_)
            | UserError::InvalidRoles(_) => ApiError::bad_request(message),
            UserError::Pool(_)
            | UserError::MergeUnavailable
            | UserError::PurgeIdentifier { .. }
            | UserError::OwnerReassignment { .. } => {
                internal_api_error("User operation failed", &message)
            },
        };
        Self(api)
    }
}

impl From<OauthError> for ApiHttpError {
    fn from(err: OauthError) -> Self {
        if matches!(
            err,
            OauthError::CodeNotFound(_)
                | OauthError::TokenNotFound(_)
                | OauthError::ClientNotFound(_)
                | OauthError::UserNotFound(_)
        ) {
            return Self(ApiError::not_found(err.to_string()));
        }
        let api = match err.kind() {
            OauthErrorKind::InvalidRequest
            | OauthErrorKind::InvalidClientMetadata
            | OauthErrorKind::ExpiredChallenge
            | OauthErrorKind::InvalidCredential => ApiError::bad_request(err.to_string()),
            OauthErrorKind::UsernameUnavailable | OauthErrorKind::EmailExists => {
                ApiError::conflict(err.to_string())
            },
            OauthErrorKind::InvalidClient => ApiError::unauthorized("Client authentication failed"),
            OauthErrorKind::AuthenticationFailed => ApiError::unauthorized("Authentication failed"),
            OauthErrorKind::InvalidGrant
            | OauthErrorKind::InvalidToken
            | OauthErrorKind::AccessDenied => ApiError::unauthorized(err.to_string()),
            OauthErrorKind::NotFound => ApiError::not_found(err.to_string()),
            OauthErrorKind::ServerError => {
                ApiError::internal("Authorization operation failed", err)
            },
        };
        Self(api)
    }
}

impl From<SessionCreationError> for ApiHttpError {
    fn from(err: SessionCreationError) -> Self {
        let message = err.to_string();
        Self(match err {
            SessionCreationError::UserNotFound { .. } => ApiError::not_found(message),
            SessionCreationError::Internal(_) => {
                internal_api_error("Session creation failed", &message)
            },
        })
    }
}

impl From<ContextExtractionError> for ApiHttpError {
    fn from(err: ContextExtractionError) -> Self {
        let message = err.to_string();
        let api = match err {
            ContextExtractionError::MissingHeader(_)
            | ContextExtractionError::MissingAuthHeader
            | ContextExtractionError::InvalidToken(_)
            | ContextExtractionError::Revoked
            | ContextExtractionError::MissingSessionId
            | ContextExtractionError::MissingUserId => ApiError::unauthorized(message),
            ContextExtractionError::MissingContextId
            | ContextExtractionError::InvalidHeaderValue { .. }
            | ContextExtractionError::InvalidUserId(_) => ApiError::bad_request(message),
            ContextExtractionError::ForbiddenHeader { .. } => ApiError::forbidden(message),
            ContextExtractionError::UserNotFound(_) => ApiError::not_found(message),
            ContextExtractionError::DatabaseError { .. } => {
                internal_api_error("Request context lookup failed", &message)
            },
        };
        Self(api)
    }
}

impl From<BundleError> for ApiHttpError {
    fn from(err: BundleError) -> Self {
        let message = err.to_string();
        let api = match err {
            BundleError::Auth { .. } => ApiError::forbidden(message),
            BundleError::Verify(_)
            | BundleError::Ownership { .. }
            | BundleError::Policy { .. }
            | BundleError::TooLarge { .. } => ApiError::bad_request(message),
            BundleError::SourceMissing { .. } => ApiError::not_found(message),
            BundleError::Fetch { .. } | BundleError::Extract { .. } | BundleError::Io(_) => {
                internal_api_error("Services bundle operation failed", &message)
            },
        };
        Self(api)
    }
}
