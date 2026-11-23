//! [`AgentHttpError`]: an agent route's failure, answered through the API
//! error model.
//!
//! Agent and repository failures keep their status: a missing entity is 404,
//! a repository failure 500 with the fixed body and its cause logged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_agent::AgentError;
use systemprompt_extension::capability_error_response;
use systemprompt_extension::router::{ApiHttpError, IntoApiError};
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_models::api::ApiError;
use systemprompt_traits::RepositoryError;

/// An agent handler's failure.
#[derive(Debug, thiserror::Error)]
pub enum AgentHttpError {
    #[error(transparent)]
    Agent(#[from] AgentError),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("request refused by the API error model")]
    Http(ApiHttpError),
}

impl From<ApiHttpError> for AgentHttpError {
    fn from(error: ApiHttpError) -> Self {
        Self::Http(error)
    }
}

impl From<ApiError> for AgentHttpError {
    fn from(error: ApiError) -> Self {
        Self::Http(ApiHttpError::from(error))
    }
}

impl From<IdValidationError> for AgentHttpError {
    fn from(error: IdValidationError) -> Self {
        Self::Http(ApiHttpError::from(ApiError::from(error)))
    }
}

impl IntoApiError for AgentHttpError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Agent(AgentError::NotFound(message)) => ApiError::not_found(message),
            Self::Agent(AgentError::Repository(inner)) | Self::Repository(inner) => {
                ApiError::from(inner)
            },
            Self::Agent(other) => ApiError::internal("Agent operation failed", other),
            Self::Http(error) => error.into_api_error(),
        }
    }
}

capability_error_response!(AgentHttpError);
