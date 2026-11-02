//! The admin recall simulator: what the brain would inject for a chosen
//! principal and prompt, without recording a receipt.
//!
//! The principal is built the way the gateway builds it: with no `project`
//! in the body, the user's default `project` value is resolved through the
//! subject attribute providers, so simulate and the gateway scope the same
//! recall. `reuse: true` with a `conversation_id` reads that conversation's
//! stored block, as a same-turn gateway request does; without one it
//! answers `not_new_turn`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use axum::Json;
use axum::extract::State;
use systemprompt_extension::brain::{
    RecallConversation, RecallPrincipal, RecallRequest, ReuseHint,
};
use systemprompt_models::api::ApiError;
use systemprompt_models::auth::RequestActor;

use super::dto::{SimulateBody, SimulateResponse};
use super::{BrainHttpError, BrainRoutes};

const PROJECT_DIMENSION: &str = "project";

pub(super) async fn simulate(
    State(state): State<BrainRoutes>,
    Json(body): Json<SimulateBody>,
) -> Result<Json<SimulateResponse>, BrainHttpError> {
    let project = match body.project {
        Some(project) => Some(project),
        None => state
            .subject_providers
            .default_value(&body.user_id, PROJECT_DIMENSION)
            .await
            .map_err(|error| ApiError::internal("subject attribute lookup failed", error))?,
    };
    let principal = RecallPrincipal {
        user_id: body.user_id,
        client: body.client,
        actor: RequestActor::Human,
        project,
        client_id: None,
    };
    let reuse = if body.reuse {
        ReuseHint::SameTurn
    } else {
        ReuseHint::NewTurn
    };
    let conversation = RecallConversation {
        gateway_conversation_id: body.conversation_id,
        ..RecallConversation::default()
    };
    let recall = &state.profile.recall;
    let request = RecallRequest::new(principal, conversation)
        .with_subject_hints(&body.prompt)
        .with_budget_tokens(recall.budget_tokens)
        .with_deadline(Duration::from_millis(recall.deadline_ms))
        .with_reuse(reuse);
    let outcome = state.recall.recall(&request).await?;
    Ok(Json(SimulateResponse::from(outcome)))
}
