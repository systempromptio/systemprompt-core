//! Stream route mounting: the server-sent event streams under the stream
//! base path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::{AuthzPolicy, ContextKind, RateBudget, RateLimit, RouteGroup};
use systemprompt_models::modules::ApiPaths;

use super::{MountCtx, declare};
use crate::services::middleware::MountError;

pub(in crate::services::server::routes) fn mount_stream(
    router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, MountError> {
    mount.mount(
        router,
        declare(
            crate::routes::stream::stream_router(mount.ctx),
            ApiPaths::STREAM_BASE,
            AuthzPolicy::user(),
            ContextKind::User,
            RouteGroup::McpAndStream,
        )
        .with_rate_limit(RateLimit::new("stream", RateBudget::Stream)),
    )
}
