//! The admin recall simulator: what the brain would inject for a chosen
//! principal and prompt, without recording a receipt.
//!
//! The principal is built the way the gateway builds it: with no `project`
//! in the body, the user's default `project` value is resolved through the
//! subject attribute providers, so simulate and the gateway scope the same
//! recall. `reuse: true` with a `conversation_id` reads that conversation's
//! stored block, as a same-turn gateway request does; without one it
//! answers `not_new_turn`. The recall runs under `recall.deadline_ms` as
//! the gateway's does, and one that misses it answers `deadline_exceeded`;
//! the simulator keeps nothing running in the background. `as_of` recalls the
//! store as it stood at that instant.
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
use crate::error::ApiHttpError;

const PROJECT_DIMENSION: &str = "project";
const DEADLINE_EXCEEDED: &str = "deadline_exceeded";

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
            .map_err(ApiHttpError::from)?,
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
        .with_reuse(reuse)
        .with_as_of(body.as_of);
    let Ok(outcome) = tokio::time::timeout(request.deadline, state.recall.recall(&request)).await
    else {
        return Ok(Json(SimulateResponse {
            descriptor: format!("skipped={DEADLINE_EXCEEDED}"),
            skipped: Some(DEADLINE_EXCEEDED),
            block: None,
        }));
    };
    Ok(Json(SimulateResponse::from(outcome?)))
}
