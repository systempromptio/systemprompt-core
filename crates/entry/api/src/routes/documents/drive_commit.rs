//! The Drive commit route: one multipart `meta` and `blob`, answered with the
//! status of how the commit landed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::{Multipart, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use systemprompt_files::documents::agents::act_as;
use systemprompt_identifiers::TraceId;
use systemprompt_models::api::ApiError;
use systemprompt_models::documents::{DriveCommitMeta, DriveCommitResponse};

use super::drive::DriveState;
use super::drive_token::device_user;
use super::error::DocumentsResult;
use super::headers::base_revision;

pub(super) async fn commit(
    State(state): State<DriveState>,
    headers: HeaderMap,
    form: Multipart,
) -> DocumentsResult<Response> {
    let (who, actor) = device_user(&state, &headers).await?;
    let trace = TraceId::generate();
    let (mut meta, bytes) = read_commit(form).await?;
    if let Some(base) = base_revision(&headers)? {
        if meta
            .base_revision
            .as_ref()
            .is_some_and(|declared| declared != &base)
        {
            return Err(ApiError::bad_request(
                "If-Match and meta.base_revision name different revisions",
            )
            .into());
        }
        meta.base_revision = Some(base);
    }
    let outcome = Box::pin(act_as(
        actor,
        state.service.drive_commit(&who, meta, bytes, &trace),
    ))
    .await?;
    let status = match outcome {
        DriveCommitResponse::Committed { .. } | DriveCommitResponse::Merged { .. } => {
            StatusCode::OK
        },
        DriveCommitResponse::Conflict { .. } => StatusCode::CONFLICT,
        DriveCommitResponse::Proposed { .. } => StatusCode::CREATED,
    };
    Ok((status, Json(outcome)).into_response())
}

async fn read_commit(mut form: Multipart) -> DocumentsResult<(DriveCommitMeta, Vec<u8>)> {
    let mut meta = None;
    let mut blob = None;
    while let Some(field) = form.next_field().await? {
        let part = field.name().unwrap_or_default().to_owned();
        let bytes = field.bytes().await?;
        match part.as_str() {
            "meta" => {
                meta = Some(serde_json::from_slice::<DriveCommitMeta>(&bytes).map_err(
                    |source| {
                        ApiError::bad_request("meta is not a drive commit").with_source(source)
                    },
                )?);
            },
            "blob" => blob = Some(bytes.to_vec()),
            _ => {},
        }
    }
    let meta = meta.ok_or_else(|| ApiError::bad_request("commit needs a `meta` part"))?;
    let blob = blob.ok_or_else(|| ApiError::bad_request("commit needs a `blob` part"))?;
    Ok((meta, blob))
}
