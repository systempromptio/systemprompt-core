//! The one mapping of [`NotificationsError`] onto the HTTP error envelope:
//! an unknown notification, subscription or stream is 404, a refused
//! publish or a subscription outside the stream audience 403, an escalation
//! of an item already at or above `human`, a write to a configuration-declared
//! row or a storage conflict 409, a resource subscription to a kind no
//! visibility provider handles 422, and an invalid declaration, kind or
//! argument 400, each with its `error_key`. Every other failure is the fixed
//! 5xx body with its cause logged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::api::ApiError;
use systemprompt_models::notifications::KeysetCursorError;
use systemprompt_notifications::NotificationsError;
use systemprompt_traits::RepositoryError;

use crate::error::ApiHttpError;

impl From<NotificationsError> for ApiHttpError {
    fn from(error: NotificationsError) -> Self {
        let api = match error {
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
        };
        Self::from(api)
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

impl From<KeysetCursorError> for ApiHttpError {
    fn from(error: KeysetCursorError) -> Self {
        ApiError::bad_request("invalid since cursor")
            .with_error_key("invalid_cursor")
            .with_source(error)
            .into()
    }
}
