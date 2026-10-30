//! Governing one fact: confirm, dispute, retract, erase, pin, unpin and
//! field edits, and resolving a dispute.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use systemprompt_brain::service::DisputeResolution;
use systemprompt_identifiers::{BrainDisputeId, BrainFactId};
use systemprompt_models::RequestContext;
use systemprompt_models::brain::FactRevision;

use super::dto::{DisputeBody, DisputeCreated, FieldsBody, ResolveBody, RetractBody};
use super::{BrainHttpError, BrainRoutes};

type Revision = Result<Json<FactRevision>, BrainHttpError>;

pub(super) async fn confirm(
    State(state): State<BrainRoutes>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainFactId>,
) -> Revision {
    let caller = state.caller(req_ctx.user_id()).await?;
    Ok(Json(state.service.confirm(&caller, &id).await?))
}

pub(super) async fn pin(
    State(state): State<BrainRoutes>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainFactId>,
) -> Revision {
    let caller = state.caller(req_ctx.user_id()).await?;
    Ok(Json(state.service.pin(&caller, &id).await?))
}

pub(super) async fn unpin(
    State(state): State<BrainRoutes>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainFactId>,
) -> Revision {
    let caller = state.caller(req_ctx.user_id()).await?;
    Ok(Json(state.service.unpin(&caller, &id).await?))
}

pub(super) async fn retract(
    State(state): State<BrainRoutes>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainFactId>,
    Json(body): Json<RetractBody>,
) -> Revision {
    let caller = state.caller(req_ctx.user_id()).await?;
    Ok(Json(
        state.service.retract(&caller, &id, &body.reason).await?,
    ))
}

pub(super) async fn edit_fields(
    State(state): State<BrainRoutes>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainFactId>,
    Json(body): Json<FieldsBody>,
) -> Revision {
    let caller = state.caller(req_ctx.user_id()).await?;
    Ok(Json(
        state.service.edit_fields(&caller, &id, body.fields).await?,
    ))
}

pub(super) async fn erase(
    State(state): State<BrainRoutes>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainFactId>,
) -> Result<StatusCode, BrainHttpError> {
    let caller = state.caller(req_ctx.user_id()).await?;
    state.service.erase(&caller, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn dispute(
    State(state): State<BrainRoutes>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainFactId>,
    Json(body): Json<DisputeBody>,
) -> Result<(StatusCode, Json<DisputeCreated>), BrainHttpError> {
    let caller = state.caller(req_ctx.user_id()).await?;
    let dispute_id = state.service.dispute(&caller, &id, &body.reason).await?;
    Ok((StatusCode::CREATED, Json(DisputeCreated { dispute_id })))
}

pub(super) async fn resolve(
    State(state): State<BrainRoutes>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainDisputeId>,
    Json(body): Json<ResolveBody>,
) -> Result<StatusCode, BrainHttpError> {
    let caller = state.caller(req_ctx.user_id()).await?;
    let resolution = DisputeResolution {
        fact_id: body.fact_id,
        dispute_id: id,
        outcome: body.outcome.into(),
        note: body.note,
    };
    state.service.resolve_dispute(&caller, &resolution).await?;
    Ok(StatusCode::NO_CONTENT)
}
