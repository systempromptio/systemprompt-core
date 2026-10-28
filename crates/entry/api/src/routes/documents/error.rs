//! The one mapping of [`DocumentsError`] onto the HTTP error envelope.
//!
//! A client error answers the envelope with the refusal code of
//! [`refusal_of`] (shared with the MCP tools) as its `error_key` and the
//! refusal's typed detail (the head and base revisions, the anchor's node,
//! hash and current text, the accepted kinds, ...) under `detail`. The
//! status stays per failure: 409 for conflicts and stale anchors, 403, 404,
//! 413, 422, 423 and 400. A server-side failure is the fixed 5xx body with
//! its cause logged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::multipart::{MultipartError, MultipartRejection};
use axum::extract::rejection::JsonRejection;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use systemprompt_files::documents::DocumentsError;
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_models::api::ApiError;
use systemprompt_provider_contracts::documents::CodecError;
use systemprompt_runtime::documents::refusal::{Refused, refusal_of};
use systemprompt_traits::AuthProviderError;

use crate::error::ApiHttpError;

#[derive(Debug)]
pub(crate) enum DocumentsHttpError {
    Service(Box<DocumentsError>),
    Api(Box<ApiHttpError>),
}

impl From<DocumentsError> for DocumentsHttpError {
    fn from(error: DocumentsError) -> Self {
        Self::Service(Box::new(error))
    }
}

impl From<ApiHttpError> for DocumentsHttpError {
    fn from(error: ApiHttpError) -> Self {
        Self::Api(Box::new(error))
    }
}

impl From<ApiError> for DocumentsHttpError {
    fn from(error: ApiError) -> Self {
        Self::Api(Box::new(ApiHttpError::from(error)))
    }
}

impl From<MultipartError> for DocumentsHttpError {
    fn from(error: MultipartError) -> Self {
        Self::from(ApiError::bad_request("invalid multipart body").with_source(error))
    }
}

impl From<MultipartRejection> for DocumentsHttpError {
    fn from(error: MultipartRejection) -> Self {
        Self::from(ApiError::bad_request("invalid multipart body").with_source(error))
    }
}

impl From<JsonRejection> for DocumentsHttpError {
    fn from(error: JsonRejection) -> Self {
        Self::from(ApiError::bad_request("invalid request body").with_source(error))
    }
}

impl From<AuthProviderError> for DocumentsHttpError {
    fn from(error: AuthProviderError) -> Self {
        Self::from(ApiError::internal("document caller lookup failed", error))
    }
}

impl From<IdValidationError> for DocumentsHttpError {
    fn from(error: IdValidationError) -> Self {
        Self::from(ApiError::from(error))
    }
}

pub(crate) type DocumentsResult<T> = Result<T, DocumentsHttpError>;

#[derive(Serialize)]
struct WithDetail<'a, T> {
    #[serde(flatten)]
    error: &'a ApiError,
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

pub(crate) fn refused_response(status: fn(String) -> ApiError, refused: Refused) -> Response {
    let Refused { refusal, message } = refused;
    let error = status(message).with_error_key(refusal.code.as_str());
    with_detail(&error, refusal)
}

impl IntoResponse for DocumentsHttpError {
    fn into_response(self) -> Response {
        match self {
            Self::Api(error) => (*error).into_response(),
            Self::Service(error) => service_response(*error),
        }
    }
}

fn service_response(error: DocumentsError) -> Response {
    match refusal_of(&error) {
        Some(refused) => refused_response(status_of(&error), refused),
        None => ApiHttpError::from(ApiError::internal("document operation failed", error))
            .into_response(),
    }
}

fn conflict(message: String) -> ApiError {
    ApiError::conflict(message)
}

fn forbidden(message: String) -> ApiError {
    ApiError::forbidden(message)
}

fn not_found(message: String) -> ApiError {
    ApiError::not_found(message)
}

fn bad_request(message: String) -> ApiError {
    ApiError::bad_request(message)
}

fn unprocessable(message: String) -> ApiError {
    ApiError::validation_error(message, Vec::new())
}

fn too_large(message: String) -> ApiError {
    ApiError::payload_too_large(message)
}

fn locked(message: String) -> ApiError {
    ApiError::locked(message)
}

pub(crate) fn status_of(error: &DocumentsError) -> fn(String) -> ApiError {
    match error {
        DocumentsError::Conflict { .. }
        | DocumentsError::StaleAnchor { .. }
        | DocumentsError::AnchorsAffected { .. }
        | DocumentsError::OpIdMismatch { .. }
        | DocumentsError::UnknownNode { .. }
        | DocumentsError::FolderNotEmpty { .. }
        | DocumentsError::FindNone { .. }
        | DocumentsError::ProposalConflict { .. }
        | DocumentsError::ProposalClosed { .. }
        | DocumentsError::AccessRequestOpen { .. }
        | DocumentsError::AccessRequestClosed { .. } => conflict,
        DocumentsError::Forbidden { .. }
        | DocumentsError::ShareBelowExisting { .. }
        | DocumentsError::SelfReview { .. } => forbidden,
        DocumentsError::Trashed { .. }
        | DocumentsError::NotFound { .. }
        | DocumentsError::CommentNotFound { .. } => not_found,
        DocumentsError::TooLarge { .. } => too_large,
        DocumentsError::LockHeld { .. } => locked,
        DocumentsError::IntegrityMismatch { .. }
        | DocumentsError::Template { .. }
        | DocumentsError::ContentRefused { .. } => unprocessable,
        DocumentsError::Codec(codec) => codec_status(codec),
        _ => bad_request,
    }
}

fn codec_status(codec: &CodecError) -> fn(String) -> ApiError {
    match codec {
        CodecError::Parse { .. } | CodecError::Malformed { .. } | CodecError::Refused { .. } => {
            unprocessable
        },
        CodecError::StaleAnchor { .. } | CodecError::UnknownNode { .. } => conflict,
        _ => bad_request,
    }
}
