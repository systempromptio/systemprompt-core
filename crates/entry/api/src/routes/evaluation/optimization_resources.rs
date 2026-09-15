//! Core REST inputs and retained experiment results, independent of any UI.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::optimization_error::OptimizationHttpError;
use super::optimization_state::OptimizationState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use systemprompt_evaluation::capabilities::{EvaluatorCapability, evaluator_capabilities};
use systemprompt_evaluation::experiments::records::{BudgetRecord, ExperimentRecord};
use systemprompt_evaluation::experiments::resources::ResourceContent;
use systemprompt_identifiers::{
    EvalBudgetId, EvalExperimentId, EvalRevisionId, ManagedSourceId, ResourceRevisionId,
};
use systemprompt_marketplace::managed::{GitSourceBinding, RevisionBundle, SourceSpec};
use systemprompt_models::feedback::verification::DependencyVerificationManifest;
use systemprompt_runtime::AppContext;

pub(super) fn router() -> Router<OptimizationState> {
    Router::new()
        .route("/experiments", get(experiments))
        .route("/experiments/{id}", get(super::execution_pages::detail))
        .route(
            "/experiments/{id}/executions",
            get(super::execution_pages::list),
        )
        .route("/experiments/{id}/cancellation", post(cancel))
        .route("/budgets", post(create_budget))
        .route("/budgets/{id}", get(budget))
        .route("/evaluation-revisions", post(create_evaluation_revision))
        .route("/evaluation-revisions/{id}", get(evaluation_revision))
        .route("/sources", post(create_source))
        .route("/sources/{id}", get(source))
        .route(
            "/sources/{id}/verification-bindings",
            post(bind_verification_source),
        )
        .route(
            "/sources/{id}/captures",
            post(super::operation_handlers::capture),
        )
        .route("/revisions/{id}/bundle", get(bundle))
        .route("/revisions/{id}/workspace", post(workspace))
        .route(
            "/source-verifications",
            post(super::operation_handlers::verify),
        )
        .route("/source-verifications/{id}", get(source_verification))
        .route(
            "/evaluator-capabilities",
            get(evaluator_capability_registry),
        )
}

async fn evaluator_capability_registry(
    State(ctx): State<AppContext>,
    axum::extract::Query(query): axum::extract::Query<super::collections::Cursor>,
) -> Result<Json<super::collections::Page<EvaluatorCapability>>, OptimizationHttpError> {
    let limit = query.limit()?;
    let mut items = evaluator_capabilities();
    items.sort_by_key(|item| systemprompt_marketplace::managed::consumer::host_key(item.client));
    items.retain(|item| {
        query.after.as_ref().is_none_or(|after| {
            systemprompt_marketplace::managed::consumer::host_key(item.client) > after.as_str()
        })
    });
    items.truncate(limit as usize);
    for item in &mut items {
        for observation in &mut item.observed_readiness {
            *observation = ctx
                .evaluation_repositories()
                .events
                .native_readiness(ctx.system_admin().id(), &observation.target)
                .await?;
        }
    }
    let next_cursor = if items.len() == limit as usize {
        items.last().map(|item| {
            systemprompt_marketplace::managed::consumer::host_key(item.client).to_owned()
        })
    } else {
        None
    };
    Ok(Json(super::collections::Page { items, next_cursor }))
}

async fn experiments(
    State(ctx): State<AppContext>,
    axum::extract::Query(query): axum::extract::Query<super::collections::Cursor>,
) -> Result<Json<super::collections::Page<ExperimentRecord>>, OptimizationHttpError> {
    let limit = query.limit()?;
    let items = ctx
        .evaluation_repositories()
        .experiments
        .list_page(ctx.system_admin().id(), query.after.as_deref(), limit)
        .await?;
    let next_cursor = if items.len() == limit as usize {
        items.last().map(|item| item.id.to_string())
    } else {
        None
    };
    Ok(Json(super::collections::Page { items, next_cursor }))
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

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateBudget {
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

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateEvaluationRevision {
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

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateSource {
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
    State(state): State<OptimizationState>,
    Path(id): Path<ResourceRevisionId>,
) -> Result<Json<String>, OptimizationHttpError> {
    Ok(Json(
        state
            .orchestrator()
            .register_workspace(state.ctx().system_admin().id(), &id)
            .await?,
    ))
}

async fn source_verification(
    State(ctx): State<AppContext>,
    Path(id): Path<systemprompt_identifiers::DependencyVerificationId>,
) -> Result<Json<DependencyVerificationManifest>, OptimizationHttpError> {
    Ok(Json(
        ctx.managed_repository()
            .git_verification(ctx.system_admin().id(), &id)
            .await?,
    ))
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct VerificationSourceBinding {
    resource_id: systemprompt_identifiers::ManagedResourceId,
    relative_root: String,
}

async fn bind_verification_source(
    State(ctx): State<AppContext>,
    axum::Extension(actor): axum::Extension<systemprompt_models::RequestContext>,
    Path(id): Path<ManagedSourceId>,
    Json(input): Json<VerificationSourceBinding>,
) -> Result<StatusCode, OptimizationHttpError> {
    ctx.managed_repository()
        .bind_git_verification_source(
            ctx.system_admin().id(),
            actor.user_id(),
            &GitSourceBinding {
                resource: &input.resource_id,
                source: &id,
                relative_root: &input.relative_root,
            },
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
