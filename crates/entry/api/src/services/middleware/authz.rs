//! Static, compile-time-enforced per-route authorization.
//!
//! Authentication (the `RouterExt::with_auth` extension) builds a
//! [`RequestContext`]; this layer then decides whether that caller may reach
//! the route group at all.
//!
//! The guarantee: attaching the auth middleware to a route group is only
//! possible via `with_auth`, which *requires* an [`AuthzPolicy`]. There is no
//! way to authenticate a route group without also declaring its authorization
//! tier — omitting the policy is a compile error.
//!
//! This is a COARSE gate — "may this kind of caller reach this route group".
//! It does NOT replace per-resource ownership checks (e.g. "does this user own
//! task X"); those remain the handler/repository layer's responsibility.
//!
//! One route group authenticates by bespoke means and deliberately does not
//! use `with_auth`: the AI gateway (`/v1/messages`, its own credential
//! extraction accepting `x-api-key`).

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_models::auth::UserType;

#[derive(Clone, Copy, Debug)]
pub struct AuthzPolicy {
    allowed: &'static [UserType],
}

impl AuthzPolicy {
    #[must_use]
    pub const fn public() -> Self {
        Self {
            allowed: &[
                UserType::Anon,
                UserType::User,
                UserType::Admin,
                UserType::A2a,
                UserType::Mcp,
                UserType::Service,
            ],
        }
    }

    #[must_use]
    pub const fn authenticated() -> Self {
        Self {
            allowed: &[
                UserType::User,
                UserType::Admin,
                UserType::A2a,
                UserType::Mcp,
                UserType::Service,
            ],
        }
    }

    #[must_use]
    pub const fn user() -> Self {
        Self {
            allowed: &[UserType::User, UserType::Admin],
        }
    }

    #[must_use]
    pub const fn admin() -> Self {
        Self {
            allowed: &[UserType::Admin],
        }
    }

    #[must_use]
    pub const fn restricted_to(allowed: &'static [UserType]) -> Self {
        Self { allowed }
    }

    fn permits(self, user_type: UserType) -> bool {
        self.allowed.contains(&user_type)
    }
}

pub async fn authz_gate(policy: AuthzPolicy, request: Request, next: Next) -> Response {
    // Why: an absent RequestContext means the caller never authenticated;
    // treating that as Anon means only AuthzPolicy::public admits it, so the
    // gate fails closed rather than open.
    let user_type = request
        .extensions()
        .get::<RequestContext>()
        .map_or(UserType::Anon, RequestContext::user_type);

    if policy.permits(user_type) {
        next.run(request).await
    } else {
        ApiError::forbidden(format!(
            "caller type '{}' is not authorized for this route",
            user_type.as_str()
        ))
        .into_response()
    }
}
