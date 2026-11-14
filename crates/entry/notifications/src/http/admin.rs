//! The admin routing simulator: how an event would route, recipient by
//! recipient, without delivering anything.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::State;
use systemprompt_models::notifications::{SimulateRequest, SimulateResponse};

use super::{NotificationsHttpError, NotificationsState};

pub(super) async fn simulate(
    State(state): State<NotificationsState>,
    Json(body): Json<SimulateRequest>,
) -> Result<Json<SimulateResponse>, NotificationsHttpError> {
    Ok(Json(state.service.simulate(body).await?))
}
