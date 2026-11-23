//! The mapping of a live edit session's [`CollabError`] onto the HTTP error
//! envelope: conflicts and lost leases answer 409 with their `error_key`, a
//! malformed update 400, a session the caller does not edit 403, and a
//! deployment with no live engine 503 `collab_engine_unavailable`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_files::documents::collab::CollabError;
use systemprompt_models::api::ApiError;

use super::error::DocumentsHttpError;

impl From<CollabError> for DocumentsHttpError {
    fn from(error: CollabError) -> Self {
        collab_error(error)
    }
}

fn collab_error(error: CollabError) -> DocumentsHttpError {
    type Http = DocumentsHttpError;
    let message = error.to_string();
    match error {
        CollabError::Documents(error) => Http::from(error),
        CollabError::SessionClosed { .. } => {
            Http::from(ApiError::conflict(message).with_error_key("session_closed"))
        },
        CollabError::CheckpointBusy { .. } => {
            Http::from(ApiError::conflict(message).with_error_key("checkpoint_busy"))
        },
        CollabError::CheckpointLeaseLost { .. } => {
            Http::from(ApiError::conflict(message).with_error_key("checkpoint_lease_lost"))
        },
        CollabError::NotJoined { .. } => {
            Http::from(ApiError::conflict(message).with_error_key("not_joined"))
        },
        CollabError::NotEditing { .. } => {
            Http::from(ApiError::forbidden(message).with_error_key("not_editing"))
        },
        CollabError::InvalidUpdate(source) | CollabError::InvalidVersion(source) => {
            Http::from(ApiError::bad_request(message).with_source(source))
        },
        CollabError::UpdateRejected(rejection) => Http::from(
            ApiError::bad_request(message)
                .with_error_key("update_rejected")
                .with_details(rejection.reason()),
        ),
        CollabError::PeerNotAssigned { .. } => {
            Http::from(ApiError::bad_request(message).with_error_key("peer_not_assigned"))
        },
        CollabError::MissingDependencies => {
            Http::from(ApiError::conflict(message).with_error_key("update_dependencies_missing"))
        },
        CollabError::UpdateContended { .. } => {
            Http::from(ApiError::conflict(message).with_error_key("update_contended"))
        },
        CollabError::NotText { .. } => Http::from(ApiError::bad_request(message)),
        CollabError::EngineUnavailable => Http::from(
            ApiError::service_unavailable("no live session engine is enabled")
                .with_error_key("collab_engine_unavailable"),
        ),
        other => Http::from(ApiError::internal("live edit session failed", other)),
    }
}
