//! Filing a report, listing the caller's own and reading one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use systemprompt_files::issues::{IssueFiling, IssueListQuery};
use systemprompt_identifiers::IssueReportId;
use systemprompt_models::RequestContext;
use systemprompt_models::issues::{IssueContext, IssueReport};

use super::views::{FileBody, FiledJson, IssuePage, ListParams};
use super::{IssuesState, caller};
use crate::error::ApiHttpError;

fn request_context(req_ctx: &RequestContext) -> IssueContext {
    IssueContext {
        trace: Some(req_ctx.trace_id().clone()),
        context: Some(req_ctx.context_id().clone()),
        session: Some(req_ctx.session_id().clone()),
        task: req_ctx.task_id().cloned(),
        mcp_execution: None,
    }
}

pub(super) async fn file(
    State(state): State<IssuesState>,
    Extension(req_ctx): Extension<RequestContext>,
    Json(body): Json<FileBody>,
) -> Result<(StatusCode, Json<FiledJson>), ApiHttpError> {
    let user = caller(&req_ctx)?;
    let filing = IssueFiling {
        reporter: user.id.clone(),
        via_agent: None,
        kind: body.kind,
        severity: body.severity,
        title: body.title,
        body: body.body.unwrap_or_default(),
        fingerprint_key: body.fingerprint,
        context: request_context(&req_ctx),
        client: body.client.unwrap_or(serde_json::Value::Null),
    };
    let (id, filed) = state.issues.file(filing).await?;
    Ok((
        StatusCode::CREATED,
        Json(FiledJson {
            id,
            folded: !filed.inserted,
            occurrences: filed.occurrences,
        }),
    ))
}

pub(super) async fn list(
    State(state): State<IssuesState>,
    Extension(req_ctx): Extension<RequestContext>,
    Query(params): Query<ListParams>,
) -> Result<Json<IssuePage>, ApiHttpError> {
    let user = caller(&req_ctx)?;
    let limit = params.limit();
    let query = IssueListQuery {
        reporter: Some(user.id.clone()),
        status: params.status,
        before: params.before()?,
        limit,
    };
    let issues = state.issues.list(&query).await?;
    Ok(Json(IssuePage::new(issues, limit)))
}

pub(super) async fn find(
    State(state): State<IssuesState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
) -> Result<Json<IssueReport>, ApiHttpError> {
    let user = caller(&req_ctx)?;
    let id = IssueReportId::try_new(&id)?;
    let report = state
        .issues
        .find_visible(&id, &user.id, &user.roles)
        .await?;
    Ok(Json(report))
}
