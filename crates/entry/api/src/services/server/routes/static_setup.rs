//! Static-content route setup for the web bundle.
//!
//! The static router is a public declaration; when an extension declares a
//! `SiteAuthConfig`, it also carries the `SiteAuth` protection layer, which
//! redirects unauthenticated page requests to the extension's login path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use axum::routing::get;
use std::sync::Arc;
use systemprompt_extension::{AuthzPolicy, ContextKind, ProtectionLayer, RouteProtection};
use systemprompt_runtime::AppContext;
use systemprompt_traits::{StartupEvent, StartupEventSender};

use crate::services::middleware::{MountError, MountLayers, protect};
use crate::services::static_content::{
    StaticContentMatcher, StaticContentState, serve_homepage, smart_fallback_handler,
};

pub(super) fn build_static_router(
    ctx: &AppContext,
    layers: &MountLayers,
    events: Option<&StartupEventSender>,
) -> Result<Router, MountError> {
    let path = ctx.app_paths().system().content_config().to_path_buf();
    let content_matcher = path.to_str().map_or_else(
        || {
            if let Some(tx) = events
                && tx
                    .unbounded_send(StartupEvent::Warning {
                        message: "CONTENT_CONFIG_PATH contains invalid UTF-8".to_owned(),
                        context: None,
                    })
                    .is_err()
            {
                tracing::debug!("Startup event receiver dropped");
            }
            Arc::new(StaticContentMatcher::empty())
        },
        |path_str| match StaticContentMatcher::from_config(path_str) {
            Ok(matcher) => Arc::new(matcher),
            Err(e) => {
                if let Some(tx) = events
                    && tx
                        .unbounded_send(StartupEvent::Warning {
                            message: format!("Failed to load content config: {e}"),
                            context: Some("Static content matching will be disabled".to_owned()),
                        })
                        .is_err()
                {
                    tracing::debug!("Startup event receiver dropped");
                }
                Arc::new(StaticContentMatcher::empty())
            },
        },
    );

    let static_state = StaticContentState {
        ctx: Arc::new(ctx.clone()),
        matcher: content_matcher,
        route_classifier: Arc::clone(ctx.route_classifier()),
    };

    let static_router = Router::new()
        .route("/", get(serve_homepage))
        .fallback(smart_fallback_handler)
        .with_state(static_state);

    let mut protection = RouteProtection::new(AuthzPolicy::public(), ContextKind::Public);
    if layers.guards.site_auth.is_some() {
        protection = protection.with_layer(ProtectionLayer::SiteAuth);
    }
    protect(
        static_router,
        &protection,
        layers,
        systemprompt_extension::registry::WEB_ROOT_BASE_PATH,
    )
}
