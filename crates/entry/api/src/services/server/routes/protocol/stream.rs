use axum::Router;
use systemprompt_extension::{AuthzPolicy, ContextKind, RateBudget, RateLimit, RouteGroup};
use systemprompt_models::modules::ApiPaths;

use super::{MountCtx, declare};
use crate::services::middleware::MountError;

pub(in crate::services::server::routes) fn mount_stream(
    router: Router,
    mount: &MountCtx<'_>,
    stream: Option<Router>,
) -> Result<Router, MountError> {
    let Some(stream) = stream else {
        return Ok(router);
    };
    mount.mount(
        router,
        declare(
            stream,
            ApiPaths::STREAM_BASE,
            AuthzPolicy::user(),
            ContextKind::User,
            RouteGroup::McpAndStream,
        )
        .with_rate_limit(RateLimit::new("stream", RateBudget::Stream)),
    )
}
