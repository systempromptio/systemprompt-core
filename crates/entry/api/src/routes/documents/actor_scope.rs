//! The middleware that runs every document and folder route as the actor
//! the request's credential proves (see
//! [`systemprompt_runtime::documents::actor`]): a first-party browser
//! session is the person, any bearer token an agent. A request that reaches
//! the routes without a context runs unscoped, which the document service
//! treats as an unnamed, unattended agent.
//!
//! Two refusals sit in front of that. A bearer-class token sent as the
//! session cookie (one the web login did not mint) is refused `401`, so the
//! cookie cannot carry anything but a browser session. And a browser-session
//! request the browser marks cross-site (`Sec-Fetch-Site`), or whose
//! `Origin` names another host, is refused `403`: the person's cookie acts
//! only from the person's own pages.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Request;
use axum::http::{HeaderMap, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use systemprompt_files::documents::agents::act_as;
use systemprompt_models::RequestContext;
use systemprompt_runtime::documents::actor::request_actor;

use crate::error::ApiHttpError;

/// Mount with `axum::middleware::from_fn(scope)` as a `route_layer` on any
/// router whose handlers call the document service, inside the auth layer
/// that inserts the [`RequestContext`].
pub async fn scope(request: Request, next: Next) -> Response {
    let Some(ctx) = request.extensions().get::<RequestContext>() else {
        return next.run(request).await;
    };
    if ctx.is_cookie_transport() && !ctx.is_browser_session() {
        return ApiHttpError::unauthorized(
            "the session cookie carries a token the web login did not issue",
        )
        .into_response();
    }
    if ctx.is_browser_session() && !same_origin(request.headers()) {
        return ApiHttpError::forbidden("a browser session acts only from this site's pages")
            .into_response();
    }
    let actor = request_actor(ctx);
    act_as(actor, next.run(request)).await
}

/// Whether the request came from this site's own pages: the browser's
/// `Sec-Fetch-Site` is `same-origin` or `none` when sent, and an `Origin`,
/// when sent, names the request's own host.
#[must_use]
pub fn same_origin(headers: &HeaderMap) -> bool {
    let fetch_site = headers
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok());
    if fetch_site.is_some_and(|site| site != "same-origin" && site != "none") {
        return false;
    }
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        return true;
    };
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let origin_host = origin
        .split_once("://")
        .map_or(origin, |(_, rest)| rest)
        .trim_end_matches('/');
    !host.is_empty() && origin_host.eq_ignore_ascii_case(host)
}
