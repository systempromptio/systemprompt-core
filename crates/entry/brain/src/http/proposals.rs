//! The open proposal queue and its decisions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::{Extension, Json};
use systemprompt_identifiers::BrainProposalId;
use systemprompt_models::RequestContext;
use systemprompt_models::brain::BrainProposal;

use systemprompt_brain::replay::ReplayReport;
use systemprompt_extension::brain::AnalysisWindow;

use super::dto::{LimitParams, ReplayBody};
use super::facts::limit;
use super::{BrainHttpError, OpenBrain};

pub(super) async fn list(
    state: OpenBrain,
    Extension(req_ctx): Extension<RequestContext>,
    Query(params): Query<LimitParams>,
) -> Result<Json<Vec<BrainProposal>>, BrainHttpError> {
    let (service, caller) = state.open(&req_ctx).await?;
    let limit = limit(params.limit);
    Ok(Json(service.proposals(&caller, limit).await?))
}

pub(super) async fn approve(
    state: OpenBrain,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainProposalId>,
) -> Result<StatusCode, BrainHttpError> {
    let (service, caller) = state.open(&req_ctx).await?;
    service.approve_proposal(&caller, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn reject(
    state: OpenBrain,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainProposalId>,
) -> Result<StatusCode, BrainHttpError> {
    let (service, caller) = state.open(&req_ctx).await?;
    service.reject_proposal(&caller, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn replay(
    state: OpenBrain,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainProposalId>,
    Json(body): Json<ReplayBody>,
) -> Result<Json<ReplayReport>, BrainHttpError> {
    let (service, caller) = state.open(&req_ctx).await?;
    let window = AnalysisWindow {
        since: body.since,
        until: body.until,
    };
    Ok(Json(
        service.replay_proposal(&caller, &id, window).await?,
    ))
}
