//! Enforcement of the per-route [`AuthzPolicy`].
//!
//! Authentication (the `RouterExt::with_auth` extension) builds a
//! [`RequestContext`]; [`authz_gate`] then decides whether that caller may
//! reach the route group at all. The policy type is declared in
//! `systemprompt_extension::authz` so a route declaration in any crate can
//! name it; this gate is its one enforcement point.
//!
//! Attaching the auth middleware to a route group is only possible via
//! `with_auth`, which *requires* an [`AuthzPolicy`]: omitting the policy is a
//! compile error.
//!
//! One route group authenticates by bespoke means and deliberately does not
//! use `with_auth`: the AI gateway (`/v1/messages`, its own credential
//! extraction accepting `x-api-key`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::ApiHttpError;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use systemprompt_extension::AuthzPolicy;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::UserType;

pub async fn authz_gate(policy: AuthzPolicy, request: Request, next: Next) -> Response {
    let user_type = request
        .extensions()
        .get::<RequestContext>()
        .map_or(UserType::Anon, RequestContext::user_type);

    if policy.permits(user_type) {
        next.run(request).await
    } else {
        ApiHttpError::forbidden(format!(
            "caller type '{}' is not authorized for this route",
            user_type.as_str()
        ))
        .into_response()
    }
}
