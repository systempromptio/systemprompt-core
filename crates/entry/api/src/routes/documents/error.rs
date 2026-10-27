//! The one mapping of [`DocumentsError`] onto the HTTP error envelope.
//!
//! Conflicts and stale anchors answer 409 with the envelope plus the fields a
//! client needs to retry: the head and base revisions, or the anchor's node,
//! hashes and current text. Every other client error is the plain envelope;
//! a server-side failure is the fixed 5xx body with its cause logged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use systemprompt_files::documents::DocumentsError;
use systemprompt_identifiers::{ContentHash, DocumentNodeId, DocumentRevisionId};
use systemprompt_models::api::ApiError;
use systemprompt_provider_contracts::documents::CodecError;

use crate::error::ApiHttpError;

#[derive(Debug)]
pub(crate) enum DocumentsHttpError {
    Service(DocumentsError),
    Api(ApiHttpError),
}

impl From<DocumentsError> for DocumentsHttpError {
    fn from(error: DocumentsError) -> Self {
        Self::Service(error)
    }
}

impl From<ApiHttpError> for DocumentsHttpError {
    fn from(error: ApiHttpError) -> Self {
        Self::Api(error)
    }
}

impl From<ApiError> for DocumentsHttpError {
    fn from(error: ApiError) -> Self {
        Self::Api(ApiHttpError::from(error))
    }
}

pub(crate) type DocumentsResult<T> = Result<T, DocumentsHttpError>;

#[derive(Serialize)]
struct ConflictDetail<'a> {
    head: &'a DocumentRevisionId,
    #[serde(skip_serializing_if = "Option::is_none")]
    head_content_sha256: Option<&'a ContentHash>,
    base: Option<&'a DocumentRevisionId>,
}

#[derive(Serialize)]
struct StaleAnchorDetail<'a> {
    node: &'a DocumentNodeId,
    expected: &'a str,
    actual: &'a str,
    current_text: &'a str,
}

#[derive(Serialize)]
struct WithDetail<'a, T: Serialize> {
    #[serde(flatten)]
    error: &'a ApiError,
    #[serde(flatten)]
    detail: T,
}

pub(crate) fn with_detail(error: &ApiError, detail: impl Serialize) -> Response {
    let status = error.code.status_code();
    tracing::warn!(
        error_code = ?error.code,
        message = %error.message,
        error_key = ?error.error_key,
        "API client error response"
    );
    (status, Json(WithDetail { error, detail })).into_response()
}

pub(crate) fn conflict_response(
    message: String,
    head: &DocumentRevisionId,
    head_content_sha256: Option<&ContentHash>,
    base: Option<&DocumentRevisionId>,
) -> Response {
    let error = ApiError::conflict(message).with_error_key("revision_conflict");
    with_detail(
        &error,
        ConflictDetail {
            head,
            head_content_sha256,
            base,
        },
    )
}

impl IntoResponse for DocumentsHttpError {
    fn into_response(self) -> Response {
        match self {
            Self::Api(error) => error.into_response(),
            Self::Service(error) => service_response(error),
        }
    }
}

fn service_response(error: DocumentsError) -> Response {
    let message = error.to_string();
    match &error {
        DocumentsError::Conflict { head, base, .. } => {
            return conflict_response(message, head, None, base.as_ref());
        },
        DocumentsError::StaleAnchor {
            node,
            expected,
            actual,
            current_text,
        } => {
            let api = ApiError::conflict(message.clone()).with_error_key("stale_anchor");
            return with_detail(
                &api,
                StaleAnchorDetail {
                    node,
                    expected,
                    actual,
                    current_text,
                },
            );
        },
        _ => {},
    }
    ApiHttpError::from(api_error(error, message)).into_response()
}

fn api_error(error: DocumentsError, message: String) -> ApiError {
    match error {
        DocumentsError::Forbidden { .. } => ApiError::forbidden(message),
        DocumentsError::NotFound { .. } => ApiError::not_found(message),
        DocumentsError::TooLarge { .. } => ApiError::payload_too_large(message),
        DocumentsError::LockHeld { .. } => ApiError::locked(message).with_error_key("lock_held"),
        DocumentsError::IntegrityMismatch { .. } => {
            ApiError::validation_error(message, Vec::new()).with_error_key("integrity_mismatch")
        },
        DocumentsError::UnknownNode { .. } => {
            ApiError::conflict(message).with_error_key("unknown_node")
        },
        DocumentsError::FolderNotEmpty { .. } => {
            ApiError::conflict(message).with_error_key("folder_not_empty")
        },
        DocumentsError::InvalidName { .. }
        | DocumentsError::InvalidCursor { .. }
        | DocumentsError::UnknownFormat { .. }
        | DocumentsError::UnsupportedFormat { .. }
        | DocumentsError::UnknownSubjectKind { .. }
        | DocumentsError::InvalidMove { .. } => ApiError::bad_request(message),
        DocumentsError::Codec(source) => codec_error(source),
        other => ApiError::internal("document operation failed", other),
    }
}

fn codec_error(source: CodecError) -> ApiError {
    let message = source.to_string();
    match source {
        CodecError::UnsupportedOp { .. } | CodecError::AmbiguousMatch { .. } | CodecError::NoMatch => {
            ApiError::bad_request(message).with_error_key("edit_rejected")
        },
        CodecError::Parse { .. } | CodecError::Malformed { .. } => {
            ApiError::validation_error(message, Vec::new()).with_error_key("malformed_document")
        },
        other => ApiError::internal("document codec failed", DocumentsError::Codec(other)),
    }
}
