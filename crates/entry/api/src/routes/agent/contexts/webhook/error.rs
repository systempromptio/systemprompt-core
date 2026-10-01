//! Webhook event-load error to HTTP mapping.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_agent::AgentError;
use systemprompt_models::api::ApiError;
use systemprompt_traits::RepositoryError;

use super::validation::PayloadValidationError;
use crate::error::ApiHttpError;

#[derive(Debug, thiserror::Error)]
pub enum LoadEventError {
    #[error("Unknown event type: {0}")]
    UnknownEventType(String),
    #[error(transparent)]
    Agent(#[from] AgentError),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("{entity} not found: {id}")]
    NotFound { entity: &'static str, id: String },
    #[error("Invalid {field}: {source}")]
    Deserialize {
        field: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("{0} is required for this event type")]
    MissingField(&'static str),
    #[error("Invalid payload: {0}")]
    InvalidPayload(String),
    #[error(transparent)]
    Payload(#[from] PayloadValidationError),
}

impl From<LoadEventError> for ApiHttpError {
    fn from(err: LoadEventError) -> Self {
        match err {
            LoadEventError::Agent(e) => Self::from(e),
            LoadEventError::Repository(e) => Self::from(e),
            e @ LoadEventError::NotFound { .. } => Self::not_found(e.to_string()),
            e @ (LoadEventError::UnknownEventType(_)
            | LoadEventError::Deserialize { .. }
            | LoadEventError::MissingField(_)
            | LoadEventError::InvalidPayload(_)
            | LoadEventError::Payload(PayloadValidationError::TooLarge { .. })) => {
                Self::bad_request(e.to_string())
            },
            LoadEventError::Payload(e) => {
                ApiError::internal("Webhook payload is invalid", e).into()
            },
        }
    }
}
