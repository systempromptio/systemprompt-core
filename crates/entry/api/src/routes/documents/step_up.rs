//! RFC 9470 step-up for the high-impact document and folder routes: sharing
//! (grant create and revoke, revoke-all, granting an access request),
//! transfer, delete, and setting an agent cap.
//!
//! A person's UI session whose sign-in is older than
//! [`UI_SESSION_STEP_UP_MAX_AGE_SECS`] is answered `401` with
//! `WWW-Authenticate: Bearer error="insufficient_user_authentication"` and that
//! `max_age`, so its client sends the person through the sign-in again. A
//! browser session and an agent pass through: the browser session is bound to
//! the site's own pages and an agent is governed by its document policy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Request;
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::ui_session::{UI_SESSION_STEP_UP_MAX_AGE_SECS, step_up_required};

use crate::error::ApiHttpError;

pub(super) async fn require_recent_sign_in(request: Request, next: Next) -> Response {
    let Some(ctx) = request.extensions().get::<RequestContext>() else {
        return ApiHttpError::unauthorized("the request carries no authenticated context")
            .into_response();
    };
    if let Some(auth_time) = ctx.ui_session_auth_time()
        && step_up_required(auth_time, chrono::Utc::now().timestamp())
    {
        return insufficient_user_authentication();
    }
    next.run(request).await
}

fn insufficient_user_authentication() -> Response {
    let mut response =
        ApiHttpError::unauthorized("this action needs a sign-in no older than the step-up window")
            .into_response();
    let challenge = format!(
        "Bearer error=\"insufficient_user_authentication\", error_description=\"A more recent \
         authentication is required\", max_age={UI_SESSION_STEP_UP_MAX_AGE_SECS}"
    );
    if let Ok(value) = HeaderValue::from_str(&challenge) {
        response
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, value);
    }
    response
}
