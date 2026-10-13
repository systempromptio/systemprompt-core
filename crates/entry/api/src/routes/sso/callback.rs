//! `GET|POST /{id}/callback` — the identity provider's return, as a query
//! string or an `application/x-www-form-urlencoded` form post.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Form;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use systemprompt_oauth_issuance::sso::SsoCallback;
use systemprompt_traits::ExtractSignals;
use systemprompt_traits::sso::{SsoReturn, SsoReturnBinding};

use super::SsoRouteState;
use super::respond::{binder_from_headers, into_response};
use crate::services::middleware::client_addr::ClientIp;

pub(super) async fn handle_callback_query(
    Path(id): Path<String>,
    State(state): State<SsoRouteState>,
    ClientIp(caller_ip): ClientIp,
    headers: HeaderMap,
    Query(params): Query<Vec<(String, String)>>,
) -> Response {
    finish(
        &state,
        &id,
        &headers,
        caller_ip,
        SsoReturn {
            binding: SsoReturnBinding::Query,
            params,
        },
    )
    .await
}

pub(super) async fn handle_callback_form(
    Path(id): Path<String>,
    State(state): State<SsoRouteState>,
    ClientIp(caller_ip): ClientIp,
    headers: HeaderMap,
    Form(params): Form<Vec<(String, String)>>,
) -> Response {
    finish(
        &state,
        &id,
        &headers,
        caller_ip,
        SsoReturn {
            binding: SsoReturnBinding::Form,
            params,
        },
    )
    .await
}

async fn finish(
    state: &SsoRouteState,
    id: &str,
    headers: &HeaderMap,
    caller_ip: Option<std::net::IpAddr>,
    ret: SsoReturn,
) -> Response {
    let analytics = state.analytics.extract_analytics(
        headers,
        ExtractSignals {
            caller_ip,
            ..Default::default()
        },
    );
    let binder = binder_from_headers(headers);
    let outcome = state
        .driver
        .finish(
            id,
            SsoCallback {
                ret,
                binder_cookie: binder.as_deref(),
                analytics: &analytics,
                ip: caller_ip,
            },
        )
        .await;
    into_response(outcome)
}
