//! Attaching files to a report: declaring the upload answers signed upload
//! links, and attaching a completed upload binds it to the report.
//!
//! The declare route refuses early when the report is not the caller's
//! active report or its attachments are full; the cap is enforced again,
//! under lock, when the upload is attached.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use systemprompt_files::uploads::NewUpload;
use systemprompt_identifiers::{DocumentUploadId, IssueReportId};
use systemprompt_models::RequestContext;
use systemprompt_runtime::documents::upload_links::{
    UPLOAD_TOKEN_HEADER, UploadGrant, UploadLinkTarget, signed_upload_links,
};

use super::views::{AttachmentBody, AttachmentUploadJson};
use super::{IssuesState, caller};
use crate::error::ApiHttpError;

pub(super) async fn declare(
    State(state): State<IssuesState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<AttachmentBody>,
) -> Result<(StatusCode, Json<AttachmentUploadJson>), ApiHttpError> {
    let id = IssueReportId::try_new(&id)?;
    let user = caller(&req_ctx)?;
    state.issues.ensure_attachable(&id, &user.id).await?;
    let request = NewUpload {
        name: body.file_name,
        mime: body.mime,
        size: body.size,
        sha256: body.sha256,
    };
    let session = state
        .uploads
        .create(&user.id, request, req_ctx.trace_id())
        .await?;
    let links = signed_upload_links(
        state.tokens.as_ref(),
        UploadLinkTarget {
            grant: UploadGrant::Write,
            base_url: &state.base_url,
            user,
            session_id: req_ctx.session_id(),
            upload: &session.id,
        },
    )?;
    Ok((
        StatusCode::CREATED,
        Json(AttachmentUploadJson {
            part_size: session.part_size,
            part_count: session.part_count(),
            upload_id: session.id,
            parts_url: links.parts_url,
            status_url: links.status_url,
            complete_url: links.complete_url,
            upload_token: links.token,
            token_header: UPLOAD_TOKEN_HEADER,
            expires_at: links.expires_at.min(session.expires_at),
        }),
    ))
}

pub(super) async fn attach(
    State(state): State<IssuesState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path((id, upload_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiHttpError> {
    let user = caller(&req_ctx)?;
    let id = IssueReportId::try_new(&id)?;
    let upload = DocumentUploadId::try_new(&upload_id)?;
    state.issues.attach(&id, &upload, &user.id).await?;
    Ok(StatusCode::NO_CONTENT)
}
