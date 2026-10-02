//! `From` impls mapping domain and repository errors onto [`ApiHttpError`],
//! keeping the variant-to-HTTP-status mapping in one place so non-OAuth
//! handlers use `?`. `RepositoryError` already classifies into [`ApiError`] in
//! `systemprompt-models`; that impl is reused here. The umbrella domain errors
//! are classified by variant so that, e.g., a repository failure surfaces as
//! 500 while a missing entity surfaces as 404.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_agent::AgentError;
use systemprompt_config::{ProfileBootstrapError, SecretsBootstrapError};
use systemprompt_content::ContentError;
use systemprompt_loader::{BundleError, ConfigLoadError};
use systemprompt_marketplace::MarketplaceError;
use systemprompt_marketplace::managed::ManagedError;
use systemprompt_models::api::ApiError;
use systemprompt_models::errors::GlobalConfigError;
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_oauth::services::SessionCreationError;
use systemprompt_oauth::{OauthError, OauthErrorKind};
use systemprompt_traits::RepositoryError;
use systemprompt_users::UserError;

use super::ApiHttpError;

impl From<RepositoryError> for ApiHttpError {
    fn from(err: RepositoryError) -> Self {
        Self(ApiError::from(err))
    }
}

impl From<ConfigLoadError> for ApiHttpError {
    fn from(err: ConfigLoadError) -> Self {
        Self(ApiError::internal(
            "Services configuration unavailable",
            err,
        ))
    }
}

impl From<ProfileBootstrapError> for ApiHttpError {
    fn from(err: ProfileBootstrapError) -> Self {
        Self(ApiError::internal("Profile not ready", err))
    }
}

impl From<SecretsBootstrapError> for ApiHttpError {
    fn from(err: SecretsBootstrapError) -> Self {
        Self(ApiError::internal("Secrets not ready", err))
    }
}

impl From<GlobalConfigError> for ApiHttpError {
    fn from(err: GlobalConfigError) -> Self {
        Self(ApiError::internal("Configuration not ready", err))
    }
}

impl From<AgentError> for ApiHttpError {
    fn from(err: AgentError) -> Self {
        let api = match err {
            AgentError::NotFound(msg) => ApiError::not_found(msg),
            AgentError::Validation(msg) => ApiError::bad_request(msg),
            AgentError::Repository(inner) => ApiError::from(inner),
            other => ApiError::internal("Agent operation failed", other),
        };
        Self(api)
    }
}

impl From<ContentError> for ApiHttpError {
    fn from(err: ContentError) -> Self {
        let api = match err {
            ContentError::Repository(inner) => ApiError::from(inner),
            e @ (ContentError::ContentNotFound(_) | ContentError::LinkNotFound(_)) => {
                ApiError::not_found(e.to_string())
            },
            e @ (ContentError::InvalidRequest(_) | ContentError::Validation(_)) => {
                ApiError::bad_request(e.to_string())
            },
            other => ApiError::internal("Content operation failed", other),
        };
        Self(api)
    }
}

impl From<MarketplaceError> for ApiHttpError {
    fn from(err: MarketplaceError) -> Self {
        let api = match err {
            MarketplaceError::Managed(ManagedError::Repository(inner)) => ApiError::from(inner),
            e @ (MarketplaceError::NotFound(_)
            | MarketplaceError::NoDefault
            | MarketplaceError::Managed(ManagedError::Unavailable)) => {
                ApiError::not_found(e.to_string())
            },
            e @ (MarketplaceError::Validation(_)
            | MarketplaceError::Managed(
                ManagedError::Invalid(_) | ManagedError::InvalidInput { .. },
            )) => ApiError::bad_request(e.to_string()),
            e @ MarketplaceError::Managed(ManagedError::Conflict(_)) => {
                ApiError::conflict(e.to_string())
            },
            e @ (MarketplaceError::Catalog(_)
            | MarketplaceError::CatalogSource { .. }
            | MarketplaceError::Managed(_)
            | MarketplaceError::Import { .. }
            | MarketplaceError::ImportSource { .. }
            | MarketplaceError::Signing(_)
            | MarketplaceError::Filter(_)) => ApiError::internal("Marketplace operation failed", e),
        };
        Self(api)
    }
}

impl From<UserError> for ApiHttpError {
    fn from(err: UserError) -> Self {
        let api = match err {
            UserError::Repository(inner) => ApiError::from(inner),
            e @ UserError::NotFound(_) => ApiError::not_found(e.to_string()),
            e @ UserError::EmailAlreadyExists(_) => ApiError::conflict(e.to_string()),
            e @ (UserError::Validation(_)
            | UserError::InvalidStatus(_)
            | UserError::InvalidRole(_)
            | UserError::InvalidRoles(_)) => ApiError::bad_request(e.to_string()),
            e @ (UserError::Pool(_)
            | UserError::MergeUnavailable
            | UserError::PurgeIdentifier { .. }
            | UserError::OwnerReassignment { .. }) => {
                ApiError::internal("User operation failed", e)
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
        Self(match err {
            e @ SessionCreationError::UserNotFound { .. } => ApiError::not_found(e.to_string()),
            other => ApiError::internal("Session creation failed", other),
        })
    }
}

impl From<ContextExtractionError> for ApiHttpError {
    fn from(err: ContextExtractionError) -> Self {
        let api = match err {
            ContextExtractionError::InvalidToken(source) => {
                ApiError::unauthorized("Invalid or expired token").with_source(source)
            },
            e @ (ContextExtractionError::MissingHeader(_)
            | ContextExtractionError::MissingAuthHeader
            | ContextExtractionError::Revoked
            | ContextExtractionError::MissingSessionId
            | ContextExtractionError::MissingUserId) => ApiError::unauthorized(e.to_string()),
            e @ (ContextExtractionError::MissingContextId
            | ContextExtractionError::InvalidHeaderValue { .. }
            | ContextExtractionError::InvalidUserId(_)) => ApiError::bad_request(e.to_string()),
            e @ ContextExtractionError::ForbiddenHeader { .. } => {
                ApiError::forbidden(e.to_string())
            },
            e @ ContextExtractionError::UserNotFound(_) => ApiError::not_found(e.to_string()),
            e @ ContextExtractionError::DatabaseError { .. } => {
                ApiError::internal("Request context lookup failed", e)
            },
        };
        Self(api)
    }
}

impl From<BundleError> for ApiHttpError {
    fn from(err: BundleError) -> Self {
        let api = match err {
            e @ BundleError::Auth { .. } => ApiError::forbidden(e.to_string()),
            e @ (BundleError::Verify(_)
            | BundleError::Ownership { .. }
            | BundleError::Policy { .. }
            | BundleError::TooLarge { .. }) => ApiError::bad_request(e.to_string()),
            e @ BundleError::SourceMissing { .. } => ApiError::not_found(e.to_string()),
            e @ (BundleError::Fetch { .. } | BundleError::Extract { .. } | BundleError::Io(_)) => {
                ApiError::internal("Services bundle operation failed", e)
            },
        };
        Self(api)
    }
}
