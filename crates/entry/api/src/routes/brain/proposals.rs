//! The open proposal queue and its decisions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use systemprompt_identifiers::BrainProposalId;
use systemprompt_models::RequestContext;
use systemprompt_models::brain::BrainProposal;

use super::dto::LimitParams;
use super::facts::limit;
use super::{BrainHttpError, BrainRoutes};

pub(super) async fn list(
    State(state): State<BrainRoutes>,
    Extension(req_ctx): Extension<RequestContext>,
    Query(params): Query<LimitParams>,
) -> Result<Json<Vec<BrainProposal>>, BrainHttpError> {
    let caller = state.caller(req_ctx.user_id()).await?;
    let limit = limit(params.limit);
    Ok(Json(state.service.proposals(&caller, limit).await?))
}

pub(super) async fn approve(
    State(state): State<BrainRoutes>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainProposalId>,
) -> Result<StatusCode, BrainHttpError> {
    let caller = state.caller(req_ctx.user_id()).await?;
    state.service.approve_proposal(&caller, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn reject(
    State(state): State<BrainRoutes>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainProposalId>,
) -> Result<StatusCode, BrainHttpError> {
    let caller = state.caller(req_ctx.user_id()).await?;
    state.service.reject_proposal(&caller, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
