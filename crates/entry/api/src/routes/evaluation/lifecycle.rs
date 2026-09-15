//! Owned suggestion and human approval resources expose stable mutation status.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::campaigns::OptimizationState;
use super::optimization_error::OptimizationHttpError;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};
use systemprompt_evaluation::campaigns::suggestions::RetainedSuggestion;
use systemprompt_evaluation::repository::experiments::{
    ApprovalDecision, ApprovalVerdict, ExecutionApproval, SuggestionRequest,
};
use systemprompt_identifiers::{EvalApprovalId, EvalSuggestionId};
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;

/// Exact human decision and observed state; actor comes from authentication.
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecideApproval {
    pub approve: bool,
    pub observed_precondition_digest: String,
}

pub(super) fn router() -> Router<OptimizationState> {
    Router::new()
        .route("/evaluation-suggestions", post(create))
        .route("/evaluation-suggestions/{id}", get(suggestion))
        .route("/evaluation-approvals/{id}", get(approval))
        .route("/evaluation-approvals/{id}/decisions", post(decide))
}
async fn create(
    State(ctx): State<AppContext>,
    Json(input): Json<SuggestionRequest>,
) -> Result<
    (
        StatusCode,
        [(&'static str, String); 1],
        Json<RetainedSuggestion>,
    ),
    OptimizationHttpError,
> {
    let owner = ctx.system_admin().id();
    let id = ctx
        .evaluation_repositories()
        .lifecycle
        .create_suggestion(owner, &input)
        .await?;
    let value = ctx
        .evaluation_repositories()
        .lifecycle
        .suggestion(owner, &id)
        .await?;
    Ok((
        StatusCode::CREATED,
        [("location", format!("/api/v1/evaluation-suggestions/{id}"))],
        Json(value),
    ))
}
async fn suggestion(
    State(ctx): State<AppContext>,
    Path(id): Path<EvalSuggestionId>,
) -> Result<Json<RetainedSuggestion>, OptimizationHttpError> {
    Ok(Json(
        ctx.evaluation_repositories()
            .lifecycle
            .suggestion(ctx.system_admin().id(), &id)
            .await?,
    ))
}
async fn approval(
    State(ctx): State<AppContext>,
    Path(id): Path<EvalApprovalId>,
) -> Result<Json<ExecutionApproval>, OptimizationHttpError> {
    Ok(Json(
        ctx.evaluation_repositories()
            .lifecycle
            .approval(ctx.system_admin().id(), &id)
            .await?,
    ))
}
async fn decide(
    State(ctx): State<AppContext>,
    Extension(actor): Extension<RequestContext>,
    Path(id): Path<EvalApprovalId>,
    headers: HeaderMap,
    Json(input): Json<DecideApproval>,
) -> Result<
    (
        StatusCode,
        [(&'static str, String); 1],
        Json<super::operations::OperationResponse<ExecutionApproval>>,
    ),
    OptimizationHttpError,
> {
    use systemprompt_marketplace::managed::operations::ApiOperationClaim;
    let claim = super::operations::begin(
        &ctx,
        &headers,
        "approval_decision",
        &(&id, actor.user_id(), &input),
    )
    .await?;
    let response = match claim {
        ApiOperationClaim::Retained(operation) => super::operations::response(&operation)?,
        ApiOperationClaim::Acquired(operation) => {
            let result = async {
                ctx.evaluation_repositories()
                    .lifecycle
                    .decide_approval(
                        ctx.system_admin().id(),
                        &ApprovalVerdict {
                            actor: actor.user_id(),
                            approval: &id,
                            decision: if input.approve {
                                ApprovalDecision::Approve
                            } else {
                                ApprovalDecision::Deny
                            },
                            observed_precondition: &input.observed_precondition_digest,
                        },
                    )
                    .await?;
                Ok(ctx
                    .evaluation_repositories()
                    .lifecycle
                    .approval(ctx.system_admin().id(), &id)
                    .await?)
            }
            .await;
            super::operations::finish(&ctx, &operation, result).await?
        },
    };
    Ok((
        super::operations::status_code(&response),
        [(
            "location",
            format!("/api/v1/operations/{}", response.operation.id),
        )],
        Json(response),
    ))
}
