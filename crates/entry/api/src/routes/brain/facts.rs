//! Fact reads, search, the subject view and person-entered facts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use chrono::Utc;
use systemprompt_brain::FactFilter;
use systemprompt_brain::service::FactWhy;
use systemprompt_identifiers::{BrainFactId, FactKindId};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::RequestActor;
use systemprompt_models::brain::{
    BrainFact, EnteredFact, FactRevision, FactStatus, RecallCtx, SubjectKey,
};

use super::dto::{EnterAccepted, SearchParams, WhyParams};
use super::{BrainHttpError, BrainRoutes};

const MAX_LIMIT: u32 = 500;

pub(super) fn limit(requested: Option<u32>) -> i64 {
    requested.map_or(FactFilter::DEFAULT_LIMIT, |n| {
        i64::from(n.clamp(1, MAX_LIMIT))
    })
}

fn filter(params: SearchParams) -> Result<FactFilter, BrainHttpError> {
    let mut filter = FactFilter::new().with_limit(limit(params.limit));
    if let Some(kind) = params.kind {
        filter = filter.with_kind(FactKindId::try_new(kind)?);
    }
    if let Some(subject) = params.subject {
        filter = filter.with_subject(subject.parse::<SubjectKey>()?);
    }
    if let Some(status) = params.status {
        let statuses = status
            .split(',')
            .map(|s| s.trim().parse::<FactStatus>())
            .collect::<Result<Vec<_>, _>>()?;
        filter = filter.with_statuses(statuses);
    }
    Ok(filter)
}

pub(super) async fn search(
    State(state): State<BrainRoutes>,
    Extension(req_ctx): Extension<RequestContext>,
    Query(params): Query<SearchParams>,
) -> Result<Json<Vec<BrainFact>>, BrainHttpError> {
    let caller = state.caller(req_ctx.user_id()).await?;
    let filter = filter(params)?;
    Ok(Json(state.service.search(&caller, &filter).await?))
}

pub(super) async fn subject(
    State(state): State<BrainRoutes>,
    Extension(req_ctx): Extension<RequestContext>,
    Path((kind, id)): Path<(String, String)>,
) -> Result<Json<Vec<BrainFact>>, BrainHttpError> {
    let caller = state.caller(req_ctx.user_id()).await?;
    let filter = FactFilter::new().with_subject(SubjectKey::new(kind, id));
    Ok(Json(state.service.search(&caller, &filter).await?))
}

pub(super) async fn enter(
    State(state): State<BrainRoutes>,
    Extension(req_ctx): Extension<RequestContext>,
    Json(body): Json<EnteredFact>,
) -> Result<(StatusCode, Json<EnterAccepted>), BrainHttpError> {
    let caller = state.caller(req_ctx.user_id()).await?;
    let outbox_id = state.service.enter(&caller.user_id, &body).await?;
    Ok((StatusCode::ACCEPTED, Json(EnterAccepted { outbox_id })))
}

pub(super) async fn get(
    State(state): State<BrainRoutes>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainFactId>,
) -> Result<Json<BrainFact>, BrainHttpError> {
    let caller = state.caller(req_ctx.user_id()).await?;
    Ok(Json(state.service.get(&caller, &id).await?))
}

pub(super) async fn timeline(
    State(state): State<BrainRoutes>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainFactId>,
) -> Result<Json<Vec<FactRevision>>, BrainHttpError> {
    let caller = state.caller(req_ctx.user_id()).await?;
    Ok(Json(state.service.timeline(&caller, &id).await?))
}

pub(super) async fn why(
    State(state): State<BrainRoutes>,
    Extension(req_ctx): Extension<RequestContext>,
    Path(id): Path<BrainFactId>,
    Query(params): Query<WhyParams>,
) -> Result<Json<FactWhy>, BrainHttpError> {
    let caller = state.caller(req_ctx.user_id()).await?;
    let mut ctx = RecallCtx::new(caller.user_id.clone(), RequestActor::Human, Utc::now())
        .with_scopes(state.profile.recall.scopes.clone());
    if let Some(client) = params.client {
        ctx = ctx.with_client(client);
    }
    if let Some(project) = params.project {
        ctx = ctx.with_project(project);
    }
    Ok(Json(state.service.why(&caller, &id, &ctx).await?))
}
