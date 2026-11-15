//! [`NotificationsHttpError`], the error every notification handler returns,
//! and the one mapping of [`NotificationsError`] onto the HTTP error envelope:
//! an unknown notification, subscription or stream is 404, a refused
//! publish or a subscription outside the stream audience 403, an escalation
//! of an item already at or above `human`, a write to a configuration-declared
//! row or a storage conflict 409, a resource subscription to a kind no
//! visibility provider handles 422, and an invalid declaration, kind or
//! argument 400, each with its `error_key`. Every other failure is the fixed
//! 5xx body with its cause logged. An invalid feed cursor is 400
//! `invalid_cursor`, and an invalid identifier answers as the API error
//! model maps it. The response is built by the shared
//! `capability_error_response!`, so status, key, body and the 5xx policy are
//! the API's own.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::capability_error_response;
use systemprompt_extension::router::IntoApiError;
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_models::api::ApiError;
use systemprompt_models::notifications::KeysetCursorError;
use systemprompt_notifications::NotificationsError;
use systemprompt_traits::RepositoryError;

/// A notification handler's failure, answered through the API error model.
#[derive(Debug, thiserror::Error)]
pub enum NotificationsHttpError {
    #[error(transparent)]
    Service(#[from] NotificationsError),
    #[error(transparent)]
    Cursor(#[from] KeysetCursorError),
    #[error(transparent)]
    InvalidId(#[from] IdValidationError),
}

impl IntoApiError for NotificationsHttpError {
    fn into_api_error(self) -> ApiError {
        match self {
            Self::Service(error) => service(error),
            Self::Cursor(error) => ApiError::bad_request("invalid since cursor")
                .with_error_key("invalid_cursor")
                .with_source(error),
            Self::InvalidId(error) => ApiError::from(error),
        }
    }
}

capability_error_response!(NotificationsHttpError);

fn service(error: NotificationsError) -> ApiError {
    match error {
        NotificationsError::NotFound { .. } => {
            ApiError::not_found(error.to_string()).with_error_key("notification_not_found")
        },
        NotificationsError::SubscriptionNotFound { .. } => {
            ApiError::not_found(error.to_string()).with_error_key("subscription_not_found")
        },
        NotificationsError::StreamNotFound { .. } => {
            ApiError::not_found(error.to_string()).with_error_key("stream_not_found")
        },
        NotificationsError::NotInAudience { .. } => {
            ApiError::forbidden(error.to_string()).with_error_key("not_in_audience")
        },
        NotificationsError::DeclaredByConfig { .. } => {
            ApiError::conflict(error.to_string()).with_error_key("declared_by_config")
        },
        NotificationsError::UnknownResourceKind { .. } => {
            ApiError::validation_error(error.to_string(), Vec::new())
                .with_error_key("unknown_resource_kind")
        },
        NotificationsError::PublishRefused { .. } => {
            ApiError::forbidden(error.to_string()).with_error_key("publish_refused")
        },
        NotificationsError::NotEscalatable { .. } => {
            ApiError::conflict(error.to_string()).with_error_key("not_escalatable")
        },
        NotificationsError::InvalidDeclaration(_) => {
            ApiError::bad_request(error.to_string()).with_error_key("invalid_declaration")
        },
        NotificationsError::Kind(_) => {
            ApiError::bad_request(error.to_string()).with_error_key("invalid_kind")
        },
        NotificationsError::Repository(repo) => repository(repo),
        other => ApiError::internal("notification operation failed", other),
    }
}

fn repository(error: RepositoryError) -> ApiError {
    match error {
        RepositoryError::NotFound { .. } => {
            ApiError::not_found(error.to_string()).with_error_key("notification_not_found")
        },
        RepositoryError::Conflict { .. } => {
            ApiError::conflict(error.to_string()).with_error_key("conflict")
        },
        RepositoryError::InvalidArgument { .. } | RepositoryError::InvalidData { .. } => {
            ApiError::bad_request(error.to_string()).with_error_key("invalid_params")
        },
        other => ApiError::internal("notification storage failed", other),
    }
}
