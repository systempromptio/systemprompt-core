//! Typed guided confirmation and durable diagnostic status resources.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::campaigns::OptimizationState;
use super::optimization_error::OptimizationHttpError;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};
use systemprompt_evaluation::campaigns::diagnostics::CampaignDiagnostic;
use systemprompt_evaluation::campaigns::holdout::HoldoutProposal;
use systemprompt_identifiers::{EvalCampaignId, EvalHoldoutProposalId};
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;
use systemprompt_runtime::optimization::holdout::{
    ConfirmHoldout, HoldoutConfirmationTarget, HoldoutReview, PrepareHoldout,
};

pub(super) fn router() -> Router<OptimizationState> {
    Router::new()
        .route("/campaign-diagnostics", get(diagnostics))
        .route("/campaigns/{id}/holdout-proposals", post(prepare))
        .route("/campaigns/{id}/holdout-proposals/{proposal}", get(show))
        .route(
            "/campaigns/{id}/holdout-proposals/{proposal}/confirm",
            post(confirm),
        )
}
/// Bounded cursor collection optionally scoped to one organizational campaign.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticQuery {
    pub campaign_id: Option<EvalCampaignId>,
    pub after: Option<String>,
    pub limit: Option<u32>,
}
/// Durable blocked operations, including setup failures before a campaign
/// exists.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct DiagnosticPage {
    pub items: Vec<CampaignDiagnostic>,
    pub next_cursor: Option<String>,
}
async fn diagnostics(
    State(ctx): State<AppContext>,
    Query(query): Query<DiagnosticQuery>,
) -> Result<Json<DiagnosticPage>, OptimizationHttpError> {
    let limit = query.limit.unwrap_or(50);
    let items = ctx
        .evaluation_repositories()
        .campaigns
        .diagnostics(
            ctx.system_admin().id(),
            query.campaign_id.as_ref(),
            query.after.as_deref(),
            limit,
        )
        .await?;
    let next_cursor = if items.len() == limit as usize {
        items.last().map(|item| item.id.clone())
    } else {
        None
    };
    Ok(Json(DiagnosticPage { items, next_cursor }))
}
async fn prepare(
    State(state): State<OptimizationState>,
    Extension(actor): Extension<RequestContext>,
    Path(id): Path<EvalCampaignId>,
    Json(input): Json<PrepareHoldout>,
) -> Result<Json<HoldoutReview>, OptimizationHttpError> {
    Ok(Json(
        state
            .orchestrator()
            .prepare_holdout(
                state.ctx().system_admin().id(),
                actor.user_id(),
                &id,
                &input,
            )
            .await?,
    ))
}
async fn show(
    State(ctx): State<AppContext>,
    Path((id, proposal)): Path<(EvalCampaignId, EvalHoldoutProposalId)>,
) -> Result<Json<HoldoutProposal>, OptimizationHttpError> {
    Ok(Json(
        ctx.evaluation_repositories()
            .campaigns
            .holdout_proposal(ctx.system_admin().id(), &id, &proposal)
            .await?,
    ))
}
async fn confirm(
    State(state): State<OptimizationState>,
    Extension(actor): Extension<RequestContext>,
    Path((id, proposal)): Path<(EvalCampaignId, EvalHoldoutProposalId)>,
    Json(input): Json<ConfirmHoldout>,
) -> Result<Json<HoldoutProposal>, OptimizationHttpError> {
    Ok(Json(
        state
            .orchestrator()
            .confirm_holdout(
                state.ctx().system_admin().id(),
                &HoldoutConfirmationTarget {
                    actor: actor.user_id(),
                    campaign: &id,
                    id: &proposal,
                },
                &input,
            )
            .await?,
    ))
}
