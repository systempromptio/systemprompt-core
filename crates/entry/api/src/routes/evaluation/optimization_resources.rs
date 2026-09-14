//! Core REST inputs and retained experiment results, independent of any UI.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::optimization_error::OptimizationHttpError;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use systemprompt_evaluation::experiments::records::{
    BudgetRecord, ExperimentDetail, ExperimentRecord,
};
use systemprompt_evaluation::experiments::resources::ResourceContent;
use systemprompt_identifiers::{
    EvalBudgetId, EvalExperimentId, EvalRevisionId, ManagedSourceId, ResourceRevisionId,
};
use systemprompt_marketplace::managed::{GitContentVerification, RevisionBundle, SourceSpec};
use systemprompt_runtime::AppContext;

pub(super) fn router() -> Router<AppContext> {
    Router::new()
        .route("/experiments", get(experiments))
        .route("/experiments/{id}", get(experiment))
        .route("/experiments/{id}/cancellation", post(cancel))
        .route("/budgets", post(create_budget))
        .route("/budgets/{id}", get(budget))
        .route("/evaluation-revisions", post(create_evaluation_revision))
        .route("/evaluation-revisions/{id}", get(evaluation_revision))
        .route("/sources", post(create_source))
        .route("/sources/{id}", get(source))
        .route("/sources/{id}/captures", post(capture_source))
        .route("/revisions/{id}/bundle", get(bundle))
        .route("/revisions/{id}/workspace", post(workspace))
        .route("/source-verifications", post(verify_source))
}

async fn experiments(
    State(ctx): State<AppContext>,
) -> Result<Json<Vec<ExperimentRecord>>, OptimizationHttpError> {
    Ok(Json(
        ctx.evaluation_repositories()
            .experiments
            .list(ctx.system_admin().id())
            .await?,
    ))
}

async fn experiment(
    State(ctx): State<AppContext>,
    Path(id): Path<EvalExperimentId>,
) -> Result<Json<ExperimentDetail>, OptimizationHttpError> {
    Ok(Json(
        ctx.evaluation_repositories()
            .experiments
            .get(ctx.system_admin().id(), &id)
            .await?,
    ))
}

async fn cancel(
    State(ctx): State<AppContext>,
    Path(id): Path<EvalExperimentId>,
) -> Result<StatusCode, OptimizationHttpError> {
    ctx.evaluation_repositories()
        .experiments
        .cancel(ctx.system_admin().id(), &id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateBudget {
    idempotency_key: String,
    cap_microdollars: i64,
}

async fn create_budget(
    State(ctx): State<AppContext>,
    Json(input): Json<CreateBudget>,
) -> Result<impl axum::response::IntoResponse, OptimizationHttpError> {
    let id = ctx
        .evaluation_repositories()
        .budgets
        .create_shared(
            ctx.system_admin().id(),
            &input.idempotency_key,
            input.cap_microdollars,
        )
        .await?;
    Ok((
        StatusCode::CREATED,
        [("location", format!("/api/v1/budgets/{id}"))],
        Json(id),
    ))
}

async fn budget(
    State(ctx): State<AppContext>,
    Path(id): Path<EvalBudgetId>,
) -> Result<Json<BudgetRecord>, OptimizationHttpError> {
    Ok(Json(
        ctx.evaluation_repositories()
            .budgets
            .get(ctx.system_admin().id(), &id)
            .await?,
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateEvaluationRevision {
    key: String,
    content: ResourceContent,
}

async fn create_evaluation_revision(
    State(ctx): State<AppContext>,
    Json(input): Json<CreateEvaluationRevision>,
) -> Result<impl axum::response::IntoResponse, OptimizationHttpError> {
    let id = ctx
        .evaluation_repositories()
        .revisions
        .create(ctx.system_admin().id(), &input.key, &input.content)
        .await?;
    Ok((
        StatusCode::CREATED,
        [("location", format!("/api/v1/evaluation-revisions/{id}"))],
        Json(id),
    ))
}

async fn evaluation_revision(
    State(ctx): State<AppContext>,
    Path(id): Path<EvalRevisionId>,
) -> Result<Json<ResourceContent>, OptimizationHttpError> {
    Ok(Json(
        ctx.evaluation_repositories()
            .revisions
            .get(ctx.system_admin().id(), &id)
            .await?,
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateSource {
    name: String,
    specification: SourceSpec,
}

async fn create_source(
    State(ctx): State<AppContext>,
    Json(input): Json<CreateSource>,
) -> Result<impl axum::response::IntoResponse, OptimizationHttpError> {
    let id = ctx
        .managed_repository()
        .register_source(ctx.system_admin().id(), &input.name, &input.specification)
        .await?;
    Ok((
        StatusCode::CREATED,
        [("location", format!("/api/v1/sources/{id}"))],
        Json(id),
    ))
}

async fn source(
    State(ctx): State<AppContext>,
    Path(id): Path<ManagedSourceId>,
) -> Result<Json<SourceSpec>, OptimizationHttpError> {
    Ok(Json(
        ctx.managed_repository()
            .get_source(ctx.system_admin().id(), &id)
            .await?,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CaptureSource {
    skill_ids: Vec<String>,
}

async fn capture_source(
    State(ctx): State<AppContext>,
    Path(id): Path<ManagedSourceId>,
    Json(input): Json<CaptureSource>,
) -> Result<Json<systemprompt_marketplace::managed::ImportedSkills>, OptimizationHttpError> {
    Ok(Json(
        super::campaigns::orchestrator(&ctx)
            .capture_authoring_skills(
                ctx.system_admin().id(),
                &id,
                ctx.app_paths().system().services(),
                input.skill_ids,
            )
            .await?,
    ))
}

async fn bundle(
    State(ctx): State<AppContext>,
    Path(id): Path<ResourceRevisionId>,
) -> Result<Json<RevisionBundle>, OptimizationHttpError> {
    Ok(Json(
        ctx.managed_repository()
            .get_revision_bundle(ctx.system_admin().id(), &id)
            .await?,
    ))
}

async fn workspace(
    State(ctx): State<AppContext>,
    Path(id): Path<ResourceRevisionId>,
) -> Result<Json<String>, OptimizationHttpError> {
    Ok(Json(
        super::campaigns::orchestrator(&ctx)
            .register_workspace(ctx.system_admin().id(), &id)
            .await?,
    ))
}

async fn verify_source(
    State(ctx): State<AppContext>,
    Json(input): Json<GitContentVerification>,
) -> Result<Json<systemprompt_marketplace::managed::AssetDigest>, OptimizationHttpError> {
    Ok(Json(
        ctx.managed_repository()
            .verify_git_content(ctx.system_admin().id(), &input)
            .await?,
    ))
}
