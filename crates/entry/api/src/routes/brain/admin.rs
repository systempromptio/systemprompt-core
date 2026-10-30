//! The admin recall simulator: what the brain would inject for a chosen
//! principal and prompt, without recording a receipt.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use axum::Json;
use axum::extract::State;
use systemprompt_extension::brain::{
    RecallConversation, RecallPrincipal, RecallRequest, ReuseHint,
};
use systemprompt_models::auth::RequestActor;

use super::dto::{SimulateBody, SimulateResponse};
use super::{BrainHttpError, BrainRoutes};

pub(super) async fn simulate(
    State(state): State<BrainRoutes>,
    Json(body): Json<SimulateBody>,
) -> Result<Json<SimulateResponse>, BrainHttpError> {
    let principal = RecallPrincipal {
        user_id: body.user_id,
        client: body.client,
        actor: RequestActor::Human,
        project: body.project,
        client_id: None,
    };
    let reuse = if body.reuse {
        ReuseHint::SameTurn
    } else {
        ReuseHint::NewTurn
    };
    let recall = &state.profile.recall;
    let request = RecallRequest::new(principal, RecallConversation::default())
        .with_subject_hints(&body.prompt)
        .with_budget_tokens(recall.budget_tokens)
        .with_deadline(Duration::from_millis(recall.deadline_ms))
        .with_reuse(reuse);
    let outcome = state.recall.recall(&request).await?;
    Ok(Json(SimulateResponse::from(outcome)))
}
