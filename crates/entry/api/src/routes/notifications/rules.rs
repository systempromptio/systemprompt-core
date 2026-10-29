//! The caller's personal routing rules, read and replaced as a whole.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::State;
use axum::http::StatusCode;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use systemprompt_models::RequestContext;
use systemprompt_models::notifications::RuleDecl;

use super::NotificationsState;
use crate::error::ApiHttpError;

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct RuleSet {
    rules: Vec<RuleDecl>,
}

pub(super) async fn list(
    State(state): State<NotificationsState>,
    Extension(req_ctx): Extension<RequestContext>,
) -> Result<Json<RuleSet>, ApiHttpError> {
    let rules = state.service.own_rules(req_ctx.user_id()).await?;
    Ok(Json(RuleSet { rules }))
}

pub(super) async fn replace(
    State(state): State<NotificationsState>,
    Extension(req_ctx): Extension<RequestContext>,
    Json(body): Json<RuleSet>,
) -> Result<StatusCode, ApiHttpError> {
    state
        .service
        .set_own_rules(req_ctx.user_id(), body.rules)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
