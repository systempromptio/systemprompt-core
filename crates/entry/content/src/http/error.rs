//! [`ContentHttpError`], the error every content handler returns, and the one
//! mapping of [`ContentError`] onto the HTTP error envelope: a repository
//! failure is classified by `RepositoryError`, unknown content or an unknown
//! link is 404, an invalid request or a failed validation 400, and every other
//! failure is the fixed 5xx body with its cause logged. A failure the API
//! already models (an invalid identifier, a repository error, a refusal built
//! by a handler) passes through as its `ApiHttpError`. The response is built
//! by the shared `capability_error_response!`, so status, key, body and
//! the 5xx policy are the API's own.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::capability_error_response;
use systemprompt_extension::router::{ApiHttpError, IntoApiError};
use systemprompt_content::ContentError;
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_models::api::ApiError;
use systemprompt_traits::RepositoryError;

/// A content handler's failure, answered through the API error model.
#[derive(Debug, thiserror::Error)]
pub enum ContentHttpError {
    #[error(transparent)]
    Content(#[from] ContentError),
    #[error("request refused by the API error model")]
    Http(ApiHttpError),
}

impl From<ApiHttpError> for ContentHttpError {
    fn from(error: ApiHttpError) -> Self {
        Self::Http(error)
    }
}

impl From<ApiError> for ContentHttpError {
    fn from(error: ApiError) -> Self {
        Self::Http(ApiHttpError::from(error))
    }
}

impl From<IdValidationError> for ContentHttpError {
    fn from(error: IdValidationError) -> Self {
        Self::Http(ApiHttpError::from(ApiError::from(error)))
    }
}

impl From<RepositoryError> for ContentHttpError {
    fn from(error: RepositoryError) -> Self {
        Self::Http(ApiHttpError::from(ApiError::from(error)))
    }
}

impl IntoApiError for ContentHttpError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Content(error) => content(error),
            Self::Http(error) => error.into_api_error(),
        }
    }
}

capability_error_response!(ContentHttpError);

fn content(error: ContentError) -> ApiError {
    match error {
        ContentError::Repository(inner) => ApiError::from(inner),
        e @ (ContentError::ContentNotFound(_) | ContentError::LinkNotFound(_)) => {
            ApiError::not_found(e.to_string())
        },
        e @ (ContentError::InvalidRequest(_) | ContentError::Validation(_)) => {
            ApiError::bad_request(e.to_string())
        },
        other => ApiError::internal("Content operation failed", other),
    }
}
