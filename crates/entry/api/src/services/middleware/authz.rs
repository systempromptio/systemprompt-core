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
//! Two route groups authenticate by bespoke means and deliberately do not use
//! `with_auth`: the AI gateway (`/v1/messages`, its own credential extraction
//! accepting `x-api-key`) and `/sync` (a machine `SYNC_TOKEN` shared secret).

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use systemprompt_models::RequestContext;
use systemprompt_models::api::ApiError;
use systemprompt_models::auth::UserType;

/// The minimum caller identity a route group accepts. No database lookup —
/// evaluated purely against the authenticated [`UserType`].
#[derive(Clone, Copy, Debug)]
pub struct AuthzPolicy {
    allowed: &'static [UserType],
}

impl AuthzPolicy {
    /// Anonymous and every authenticated caller. A deliberate declaration that
    /// a route group is open — never the result of forgetting to set a policy.
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

    /// Any authenticated caller — anything except anonymous.
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

    /// Interactive users and admins.
    #[must_use]
    pub const fn user() -> Self {
        Self {
            allowed: &[UserType::User, UserType::Admin],
        }
    }

    /// Admins only.
    #[must_use]
    pub const fn admin() -> Self {
        Self {
            allowed: &[UserType::Admin],
        }
    }

    /// An explicit allow-list of caller types.
    #[must_use]
    pub const fn restricted_to(allowed: &'static [UserType]) -> Self {
        Self { allowed }
    }

    fn permits(self, user_type: UserType) -> bool {
        self.allowed.contains(&user_type)
    }
}

/// Authorization gate.
///
/// Runs after the auth middleware, so a [`RequestContext`] is present for
/// every authenticated caller. An absent context means the caller is
/// unauthenticated and is treated as [`UserType::Anon`] — which only
/// [`AuthzPolicy::public`] admits, so the gate fails closed.
pub async fn authz_gate(policy: AuthzPolicy, request: Request, next: Next) -> Response {
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
