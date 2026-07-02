//! `From` impls mapping domain, repository, and service errors onto
//! [`ApiHttpError`], keeping the variant-to-HTTP-status mapping in one place so
//! non-OAuth handlers use `?`. `RepositoryError` and `ServiceError` already
//! classify into [`ApiError`] in `systemprompt-models`; those impls are reused
//! here. The umbrella domain errors are classified by variant so that, e.g., a
//! repository failure surfaces as 500 while a missing entity surfaces as 404.

use systemprompt_agent::{AgentError, ProtocolError};
use systemprompt_marketplace::MarketplaceError;
use systemprompt_models::api::ApiError;
use systemprompt_models::errors::ServiceError;
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_oauth::OauthError;
use systemprompt_traits::RepositoryError;
use systemprompt_users::UserError;

use super::ApiHttpError;

impl From<RepositoryError> for ApiHttpError {
    fn from(err: RepositoryError) -> Self {
        Self(ApiError::from(err))
    }
}

impl From<ServiceError> for ApiHttpError {
    fn from(err: ServiceError) -> Self {
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
            other => ApiError::internal_error(other.to_string()),
        };
        Self(api)
    }
}

impl From<MarketplaceError> for ApiHttpError {
    fn from(err: MarketplaceError) -> Self {
        let api = match &err {
            MarketplaceError::NotFound(_) | MarketplaceError::NoDefault => {
                ApiError::not_found(err.to_string())
            },
            MarketplaceError::Validation(_) => ApiError::bad_request(err.to_string()),
            MarketplaceError::Catalog(_)
            | MarketplaceError::Signing(_)
            | MarketplaceError::Filter(_) => ApiError::internal_error(err.to_string()),
        };
        Self(api)
    }
}

impl From<UserError> for ApiHttpError {
    fn from(err: UserError) -> Self {
        let message = err.to_string();
        let api = match err {
            UserError::Repository(inner) => ApiError::from(RepositoryError::from(inner)),
            UserError::NotFound(_) => ApiError::not_found(message),
            UserError::EmailAlreadyExists(_) => ApiError::conflict(message),
            UserError::Validation(_)
            | UserError::InvalidStatus(_)
            | UserError::InvalidRole(_)
            | UserError::InvalidRoles(_) => ApiError::bad_request(message),
            UserError::Pool(_) => ApiError::internal_error(message),
        };
        Self(api)
    }
}

impl From<OauthError> for ApiHttpError {
    fn from(err: OauthError) -> Self {
        let message = err.to_string();
        let api = match err {
            OauthError::CodeNotFound(_)
            | OauthError::TokenNotFound(_)
            | OauthError::ClientNotFound(_)
            | OauthError::UserNotFound(_) => ApiError::not_found(message),
            OauthError::Validation(_) | OauthError::InvalidClientMetadata(_) => {
                ApiError::bad_request(message)
            },
            OauthError::UsernameTaken(_) | OauthError::EmailRegistered(_) => {
                ApiError::conflict(message)
            },
            OauthError::Unauthorized(_)
            | OauthError::InvalidGrant(_)
            | OauthError::InvalidClient(_)
            | OauthError::TokenInvalid(_)
            | OauthError::TokenAlgMismatch { .. }
            | OauthError::TokenMissingKid
            | OauthError::TokenUnknownKid { .. }
            | OauthError::PkceMismatch(_)
            | OauthError::Expired(_) => ApiError::unauthorized(message),
            OauthError::Provider(_)
            | OauthError::Session(_)
            | OauthError::WebAuthn(_)
            | OauthError::RegistrationStateExpired
            | OauthError::WebAuthnVerificationFailed(_)
            | OauthError::User(_)
            | OauthError::Repository(_)
            | OauthError::DatabaseRepository(_)
            | OauthError::Config(_)
            | OauthError::Crypto(_)
            | OauthError::CimdFetch(_)
            | OauthError::WebAuthnConfig(_)
            | OauthError::Internal(_) => ApiError::internal_error(message),
        };
        Self(api)
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
            ContextExtractionError::DatabaseError { .. } => ApiError::internal_error(message),
        };
        Self(api)
    }
}
