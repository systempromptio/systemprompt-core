//! Organizational campaign REST surface. Authentication is supplied by the
//! core admin middleware; actor identity is never accepted from JSON.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};
use systemprompt_evaluation::campaigns::CampaignPolicy;
use systemprompt_evaluation::campaigns::repository::{CampaignAction, CampaignRecord};
use systemprompt_identifiers::{EvalCampaignId, EvalExperimentId};
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;

use super::optimization_error::OptimizationHttpError;

pub fn router() -> Router<AppContext> {
    Router::new()
        .merge(super::optimization_resources::router())
        .merge(super::inventory::router())
        .merge(super::consumer::admin_router())
        .route("/campaigns", get(list).post(create))
        .route("/campaigns/{id}", get(show))
        .route("/campaigns/{id}/transitions", post(transition))
        .route("/campaigns/{id}/experiments", get(experiments).post(attach))
        .route(
            "/campaigns/{id}/experiments/{experiment}/report",
            get(report),
        )
        .route("/source-changes", post(accept_source))
        .route("/campaign-runs", post(launch))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    idempotency_key: String,
    policy: CampaignPolicy,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    after: Option<EvalCampaignId>,
}

#[derive(Debug, Serialize)]
struct Page {
    items: Vec<CampaignRecord>,
    next_cursor: Option<EvalCampaignId>,
}

async fn create(
    State(ctx): State<AppContext>,
    Extension(actor): Extension<RequestContext>,
    Json(input): Json<Create>,
) -> Result<impl axum::response::IntoResponse, OptimizationHttpError> {
    let owner = ctx.system_admin().id();
    let resource = ctx
        .managed_repository()
        .revision_resource(owner, &input.policy.baseline_revision_id)
        .await
        .map_err(OptimizationHttpError::Managed)?;
    if resource != input.policy.resource_id {
        return Err(systemprompt_evaluation::EvaluationError::InvalidSpec(
            "Baseline must belong to the campaign resource".to_owned(),
        )
        .into());
    }
    let id = ctx
        .evaluation_repositories()
        .campaigns
        .create(
            owner,
            actor.user_id(),
            &input.idempotency_key,
            &input.policy,
        )
        .await?;
    Ok((
        StatusCode::CREATED,
        [("location", format!("/api/v1/campaigns/{id}"))],
        Json(id),
    ))
}

async fn list(
    State(ctx): State<AppContext>,
    Query(query): Query<Cursor>,
) -> Result<Json<Page>, OptimizationHttpError> {
    let mut items = ctx
        .evaluation_repositories()
        .campaigns
        .list(ctx.system_admin().id(), query.after.as_ref())
        .await?;
    let next_cursor = if items.len() > 50 {
        items.truncate(50);
        items.last().map(|item| item.id.clone())
    } else {
        None
    };
    Ok(Json(Page { items, next_cursor }))
}

async fn show(
    State(ctx): State<AppContext>,
    Path(id): Path<EvalCampaignId>,
) -> Result<Json<CampaignRecord>, OptimizationHttpError> {
    Ok(Json(
        ctx.evaluation_repositories()
            .campaigns
            .get(ctx.system_admin().id(), &id)
            .await?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Transition {
    expected_generation: i64,
    action: CampaignAction,
}

async fn transition(
    State(ctx): State<AppContext>,
    Extension(actor): Extension<RequestContext>,
    Path(id): Path<EvalCampaignId>,
    Json(input): Json<Transition>,
) -> Result<StatusCode, OptimizationHttpError> {
    ctx.evaluation_repositories()
        .campaigns
        .transition(
            ctx.system_admin().id(),
            actor.user_id(),
            &id,
            (input.expected_generation, input.action),
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Attach {
    experiment_id: EvalExperimentId,
}

async fn attach(
    State(ctx): State<AppContext>,
    Extension(actor): Extension<RequestContext>,
    Path(id): Path<EvalCampaignId>,
    Json(input): Json<Attach>,
) -> Result<StatusCode, OptimizationHttpError> {
    orchestrator(&ctx)
        .attach(
            ctx.system_admin().id(),
            actor.user_id(),
            &id,
            &input.experiment_id,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn experiments(
    State(ctx): State<AppContext>,
    Path(id): Path<EvalCampaignId>,
) -> Result<Json<Vec<EvalExperimentId>>, OptimizationHttpError> {
    Ok(Json(
        ctx.evaluation_repositories()
            .campaigns
            .list_experiments(ctx.system_admin().id(), &id)
            .await?,
    ))
}

pub(super) fn orchestrator(
    ctx: &AppContext,
) -> systemprompt_runtime::optimization::SkillOptimizationOrchestrator {
    systemprompt_runtime::optimization::SkillOptimizationOrchestrator::new(
        ctx.managed_repository().as_ref().clone(),
        ctx.evaluation_repositories().as_ref().clone(),
        ctx.evaluation_repositories().revisions.clone(),
    )
}

async fn report(
    State(ctx): State<AppContext>,
    Path((id, experiment)): Path<(EvalCampaignId, EvalExperimentId)>,
) -> Result<Json<systemprompt_evaluation::campaigns::report::CampaignReport>, OptimizationHttpError>
{
    Ok(Json(
        orchestrator(&ctx)
            .report(ctx.system_admin().id(), &id, &experiment)
            .await?,
    ))
}

async fn accept_source(
    State(ctx): State<AppContext>,
    Extension(actor): Extension<RequestContext>,
    Json(input): Json<systemprompt_runtime::optimization::SourceAcceptance>,
) -> Result<impl axum::response::IntoResponse, OptimizationHttpError> {
    Ok((
        StatusCode::CREATED,
        Json(
            orchestrator(&ctx)
                .accept_source(ctx.system_admin().id(), actor.user_id(), &input)
                .await?,
        ),
    ))
}

async fn launch(
    State(ctx): State<AppContext>,
    Extension(actor): Extension<RequestContext>,
    Json(input): Json<systemprompt_evaluation::repository::experiments::CampaignExperiment>,
) -> Result<impl axum::response::IntoResponse, OptimizationHttpError> {
    let id = orchestrator(&ctx)
        .launch(ctx.system_admin().id(), actor.user_id(), &input)
        .await?;
    Ok((
        StatusCode::ACCEPTED,
        [("location", format!("/api/v1/experiments/{id}"))],
        Json(id),
    ))
}
