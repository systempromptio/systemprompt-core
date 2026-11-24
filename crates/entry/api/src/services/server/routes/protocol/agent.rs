use axum::Router;
use systemprompt_extension::{AuthzPolicy, ContextKind, RateBudget, RateLimit, RouteGroup};
use systemprompt_models::modules::ApiPaths;

use super::{MountCtx, declare};
use crate::services::middleware::MountError;

pub(in crate::services::server::routes) fn mount_agent(
    mut router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, MountError> {
    let ctx = mount.ctx;
    let group = RouteGroup::Agent;

    router = mount.mount(
        router,
        declare(
            crate::routes::webhook::router(crate::routes::webhook::WebhookState::from_context(
                ctx,
            )?),
            ApiPaths::WEBHOOK,
            AuthzPolicy::authenticated(),
            ContextKind::User,
            group,
        ),
    )?;

    mount.mount(
        router,
        declare(
            crate::routes::proxy::agents::router(ctx),
            ApiPaths::AGENTS_BASE,
            AuthzPolicy::authenticated(),
            ContextKind::A2a,
            group,
        )
        .with_body_limit(systemprompt_models::net::AGENT_REQUEST_BODY_LIMIT_BYTES)
        .with_rate_limit(RateLimit::new("agents", RateBudget::Agents)),
    )
}
