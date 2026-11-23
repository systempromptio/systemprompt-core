//! The two brain route sets: `/api/v1/brain` behind the user gate and
//! `/api/v1/admin/brain` behind the admin gate, both with a user request
//! context in the content-and-misc role group under the `brain` rate-limit
//! scope and budget with the default body limit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::Method;
use systemprompt_extension::router::{
    ApiBase, BasePath, BodyLimit, RouteLimits, RouteSet, RouteSpec,
};
use systemprompt_extension::{AuthzPolicy, ContextKind, RateBudget, RateLimit, RouteGroup};

use super::{BrainHttpState, admin, facts, lifecycle, proposals};

const LIMITS: RouteLimits = RouteLimits {
    rate: RateLimit::new("brain", RateBudget::Brain),
    body: BodyLimit::Default,
};
const USER: RouteSpec = RouteSpec::new(AuthzPolicy::user(), ContextKind::User, LIMITS);
const ADMIN: RouteSpec = RouteSpec::new(AuthzPolicy::admin(), ContextKind::User, LIMITS);

#[must_use]
pub fn user_routes() -> RouteSet {
    RouteSet::at::<BrainHttpState>(BasePath::Api(ApiBase::Brain), RouteGroup::ContentAndMisc)
        .route(Method::GET, "/facts", facts::search, USER)
        .route(Method::POST, "/facts", facts::enter, USER)
        .route(Method::GET, "/facts/{id}", facts::get, USER)
        .route(Method::GET, "/facts/{id}/timeline", facts::timeline, USER)
        .route(Method::GET, "/facts/{id}/why", facts::why, USER)
        .route(Method::GET, "/subjects/{kind}/{id}", facts::subject, USER)
        .route(
            Method::POST,
            "/facts/{id}/confirm",
            lifecycle::confirm,
            USER,
        )
        .route(
            Method::POST,
            "/facts/{id}/dispute",
            lifecycle::dispute,
            USER,
        )
        .route(
            Method::POST,
            "/facts/{id}/retract",
            lifecycle::retract,
            USER,
        )
        .route(Method::POST, "/facts/{id}/erase", lifecycle::erase, USER)
        .route(Method::POST, "/facts/{id}/pin", lifecycle::pin, USER)
        .route(Method::POST, "/facts/{id}/unpin", lifecycle::unpin, USER)
        .route(
            Method::PATCH,
            "/facts/{id}/fields",
            lifecycle::edit_fields,
            USER,
        )
        .route(
            Method::POST,
            "/disputes/{id}/resolve",
            lifecycle::resolve,
            USER,
        )
        .route(Method::GET, "/proposals", proposals::list, USER)
        .route(
            Method::POST,
            "/proposals/{id}/approve",
            proposals::approve,
            USER,
        )
        .route(
            Method::POST,
            "/proposals/{id}/reject",
            proposals::reject,
            USER,
        )
        .route(
            Method::POST,
            "/proposals/{id}/replay",
            proposals::replay,
            USER,
        )
        .build()
}

#[must_use]
pub fn admin_routes() -> RouteSet {
    RouteSet::at::<BrainHttpState>(
        BasePath::Api(ApiBase::AdminBrain),
        RouteGroup::ContentAndMisc,
    )
    .route(Method::POST, "/simulate", admin::simulate, ADMIN)
    .build()
}
