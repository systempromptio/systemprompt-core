//! [`MarketplaceHttpError`], the error every marketplace handler returns. A
//! marketplace failure maps through the one classification of
//! `MarketplaceError` onto the API error model (`ApiError::from`): a
//! repository failure by `RepositoryError`, an unknown marketplace or an
//! unavailable resource 404, invalid input 400, a conflict 409, every other
//! failure the fixed 5xx body. A rendering or file-read failure is the fixed
//! 5xx body with its cause logged; a refusal a handler builds passes through
//! as its `ApiHttpError`. The response is built by the shared
//! `capability_error_response!`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::capability_error_response;
use systemprompt_extension::router::{ApiHttpError, IntoApiError};
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_marketplace::MarketplaceError;
use systemprompt_models::api::ApiError;

/// A marketplace handler's failure, answered through the API error model.
#[derive(Debug, thiserror::Error)]
pub enum MarketplaceHttpError {
    #[error(transparent)]
    Marketplace(#[from] MarketplaceError),
    #[error(transparent)]
    Route(#[from] MarketplaceRouteError),
    #[error("request refused by the API error model")]
    Http(ApiHttpError),
}

/// A rendering or file-read failure of a marketplace handler.
#[derive(Debug, thiserror::Error)]
pub enum MarketplaceRouteError {
    #[error("failed to render marketplace JSON")]
    Render(#[source] serde_json::Error),
    #[error("failed to read marketplace config")]
    ReadConfig(#[source] std::io::Error),
}

impl From<ApiHttpError> for MarketplaceHttpError {
    fn from(error: ApiHttpError) -> Self {
        Self::Http(error)
    }
}

impl From<ApiError> for MarketplaceHttpError {
    fn from(error: ApiError) -> Self {
        Self::Http(ApiHttpError::from(error))
    }
}

impl From<IdValidationError> for MarketplaceHttpError {
    fn from(error: IdValidationError) -> Self {
        Self::Http(ApiHttpError::from(ApiError::from(error)))
    }
}

impl IntoApiError for MarketplaceHttpError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Marketplace(error) => ApiError::from(error),
            Self::Route(error) => {
                let context = match &error {
                    MarketplaceRouteError::Render(_) => "Failed to render marketplace",
                    MarketplaceRouteError::ReadConfig(_) => "Failed to read marketplace config",
                };
                ApiError::internal(context, error)
            },
            Self::Http(error) => error.into_api_error(),
        }
    }
}

capability_error_response!(MarketplaceHttpError);
