//! Governing one fact: confirm, dispute, retract, erase, pin, unpin and
//! field edits, and resolving a dispute.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Path;
use axum::http::StatusCode;
use axum::{Extension, Json};
use systemprompt_brain::service::DisputeResolution;
use systemprompt_identifiers::{BrainDisputeId, BrainFactId};
use systemprompt_models::RequestContext;
use systemprompt_models::brain::FactRevision;

use super::dto::{DisputeBody, DisputeCreated, FieldsBody, ResolveBody, RetractBody};
use super::{BrainHttpError, OpenBrain};

type Revision = Result<Json<FactRevision>, BrainHttpError>;

pub(super) async fn confirm(
    state: OpenBrain,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainFactId>,
) -> Revision {
    let (service, caller) = state.open(&req_ctx).await?;
    Ok(Json(service.confirm(&caller, &id).await?))
}

pub(super) async fn pin(
    state: OpenBrain,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainFactId>,
) -> Revision {
    let (service, caller) = state.open(&req_ctx).await?;
    Ok(Json(service.pin(&caller, &id).await?))
}

pub(super) async fn unpin(
    state: OpenBrain,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainFactId>,
) -> Revision {
    let (service, caller) = state.open(&req_ctx).await?;
    Ok(Json(service.unpin(&caller, &id).await?))
}

pub(super) async fn retract(
    state: OpenBrain,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainFactId>,
    Json(body): Json<RetractBody>,
) -> Revision {
    let (service, caller) = state.open(&req_ctx).await?;
    Ok(Json(
        service.retract(&caller, &id, &body.reason).await?,
    ))
}

pub(super) async fn edit_fields(
    state: OpenBrain,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainFactId>,
    Json(body): Json<FieldsBody>,
) -> Revision {
    let (service, caller) = state.open(&req_ctx).await?;
    Ok(Json(
        service.edit_fields(&caller, &id, body.fields).await?,
    ))
}

pub(super) async fn erase(
    state: OpenBrain,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainFactId>,
) -> Result<StatusCode, BrainHttpError> {
    let (service, caller) = state.open(&req_ctx).await?;
    service.erase(&caller, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn dispute(
    state: OpenBrain,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainFactId>,
    Json(body): Json<DisputeBody>,
) -> Result<(StatusCode, Json<DisputeCreated>), BrainHttpError> {
    let (service, caller) = state.open(&req_ctx).await?;
    let dispute_id = service.dispute(&caller, &id, &body.reason).await?;
    Ok((StatusCode::CREATED, Json(DisputeCreated { dispute_id })))
}

pub(super) async fn resolve(
    state: OpenBrain,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainDisputeId>,
    Json(body): Json<ResolveBody>,
) -> Result<StatusCode, BrainHttpError> {
    let (service, caller) = state.open(&req_ctx).await?;
    let resolution = DisputeResolution {
        fact_id: body.fact_id,
        dispute_id: id,
        outcome: body.outcome.into(),
        note: body.note,
    };
    service.resolve_dispute(&caller, &resolution).await?;
    Ok(StatusCode::NO_CONTENT)
}
