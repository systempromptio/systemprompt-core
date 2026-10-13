//! `GET /{id}/start?redirect=` — begin a sign-in with provider `id`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use super::SsoRouteState;
use super::respond::{refused_redirect, start_response};

#[derive(Debug, Deserialize)]
pub(super) struct StartQuery {
    pub redirect: Option<String>,
}

pub(super) async fn handle_start(
    Path(id): Path<String>,
    Query(query): Query<StartQuery>,
    State(state): State<SsoRouteState>,
) -> Response {
    let Some(provider) = state.driver.provider(&id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match state.driver.start(&id, query.redirect.as_deref()).await {
        Ok(outcome) => start_response(&outcome),
        Err(err) => {
            tracing::warn!(provider_id = %id, error = %err, "SSO start failed");
            refused_redirect(provider.login_path(), err.reason_code())
        },
    }
}
