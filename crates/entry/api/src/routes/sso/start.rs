//! `GET /{id}/start?redirect=&prompt=&max_age=` — begin a sign-in with
//! provider `id`.
//!
//! `prompt` containing `login`, or `max_age=0`, asks the identity provider to
//! make the person present a credential again; a positive `max_age` bounds
//! how old the authentication it reports may be. A deployment login page
//! forwards the two values it received from `/authorize`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use systemprompt_traits::sso::SsoReauth;

use super::SsoRouteState;
use super::respond::{refused_redirect, start_response};

#[derive(Debug, Deserialize)]
pub(super) struct StartQuery {
    pub redirect: Option<String>,
    pub prompt: Option<String>,
    pub max_age: Option<i64>,
}

impl StartQuery {
    fn reauth(&self) -> SsoReauth {
        let prompts_login = self
            .prompt
            .as_deref()
            .is_some_and(|prompt| prompt.split_whitespace().any(|value| value == "login"));
        let max_age = self.max_age.filter(|max_age| *max_age >= 0);
        SsoReauth {
            force: prompts_login || max_age == Some(0),
            max_age,
        }
    }
}

pub(super) async fn handle_start(
    Path(id): Path<String>,
    Query(query): Query<StartQuery>,
    State(state): State<SsoRouteState>,
) -> Response {
    let Some(provider) = state.driver.provider(&id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match state
        .driver
        .start(&id, query.redirect.as_deref(), query.reauth())
        .await
    {
        Ok(outcome) => start_response(&outcome),
        Err(err) => {
            tracing::warn!(provider_id = %id, error = %err, "SSO start failed");
            refused_redirect(provider.login_path(), err.reason_code())
        },
    }
}
