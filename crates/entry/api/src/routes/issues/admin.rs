//! Triage: every report, newest seen first, and the status transitions.
//! Open to the configured triage roles and to admins.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, Query, State};
use axum::{Extension, Json};
use systemprompt_files::issues::IssueListQuery;
use systemprompt_identifiers::IssueReportId;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::AuthenticatedUser;

use super::views::{IssuePage, ListParams, StatusBody, StatusJson};
use super::{IssuesState, caller};
use crate::error::ApiHttpError;

#[expect(
    clippy::result_large_err,
    reason = "ApiHttpError is the handler error type; boxing it here would propagate to every \
              caller for negligible gain"
)]
fn triager<'a>(
    state: &IssuesState,
    req_ctx: &'a RequestContext,
) -> Result<&'a AuthenticatedUser, ApiHttpError> {
    let user = caller(req_ctx)?;
    if user.is_admin() || state.issues.is_triager(&user.roles) {
        Ok(user)
    } else {
        Err(ApiHttpError::forbidden(
            "issue triage needs a triage role or admin",
        ))
    }
}

pub(super) async fn list(
    State(state): State<IssuesState>,
    Extension(req_ctx): Extension<RequestContext>,
    Query(params): Query<ListParams>,
) -> Result<Json<IssuePage>, ApiHttpError> {
    triager(&state, &req_ctx)?;
    let limit = params.limit();
    let query = IssueListQuery {
        reporter: params.reporter()?,
        status: params.status,
        before: params.before()?,
        limit,
    };
    let issues = state.issues.list(&query).await?;
    Ok(Json(IssuePage::new(issues, limit)))
}

pub(super) async fn set_status(
    State(state): State<IssuesState>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<String>,
    Json(body): Json<StatusBody>,
) -> Result<Json<StatusJson>, ApiHttpError> {
    let user = triager(&state, &req_ctx)?;
    let id = IssueReportId::try_new(&id)?;
    let from = state
        .issues
        .set_status(&id, body.status, body.note, &user.id)
        .await?;
    Ok(Json(StatusJson {
        from,
        to: body.status,
    }))
}
