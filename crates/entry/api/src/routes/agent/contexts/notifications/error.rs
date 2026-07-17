//! Notification error to HTTP mapping.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_agent::AgentError;
use systemprompt_traits::RepositoryError;

use crate::error::ApiHttpError;

#[derive(Debug, thiserror::Error)]
pub(super) enum NotificationError {
    #[error(transparent)]
    Agent(#[from] AgentError),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error("Missing {0} in notification")]
    MissingField(&'static str),
}

impl From<NotificationError> for ApiHttpError {
    fn from(err: NotificationError) -> Self {
        match err {
            NotificationError::Agent(e) => Self::from(e),
            NotificationError::Repository(e) => Self::from(e),
            NotificationError::Serde(e) => Self::internal_error(e.to_string()),
            e @ NotificationError::MissingField(_) => Self::bad_request(e.to_string()),
        }
    }
}
