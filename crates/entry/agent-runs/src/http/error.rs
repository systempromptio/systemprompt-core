//! [`AgentRunsHttpError`], the error every agent-run handler returns, and its
//! one mapping onto the HTTP error envelope: a storage or output failure is
//! the fixed 5xx body with its cause logged; a failure the API already
//! models (an invalid run id, an unknown run, a refused request) passes
//! through as its `ApiHttpError`. The response is built by the API kernel's
//! `capability_error_response!`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_agent_runs::AgentRunsError;
use systemprompt_api::kernel::{ApiHttpError, IntoApiError, capability_error_response};
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_models::api::ApiError;

#[derive(Debug, thiserror::Error)]
pub(crate) enum AgentRunsHttpError {
    #[error(transparent)]
    Storage(#[from] AgentRunsError),
    #[error("request refused by the API error model")]
    Http(ApiHttpError),
}

impl From<ApiHttpError> for AgentRunsHttpError {
    fn from(error: ApiHttpError) -> Self {
        Self::Http(error)
    }
}

impl From<IdValidationError> for AgentRunsHttpError {
    fn from(error: IdValidationError) -> Self {
        Self::Http(ApiHttpError::from(error))
    }
}

impl IntoApiError for AgentRunsHttpError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Storage(error) => ApiError::internal("Agent run storage failed", error),
            Self::Http(error) => error.into_api_error(),
        }
    }
}

capability_error_response!(AgentRunsHttpError);
