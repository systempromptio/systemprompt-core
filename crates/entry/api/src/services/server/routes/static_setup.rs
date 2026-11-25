//! Static-content route setup for the web bundle.
//!
//! The static router owns the tree's fallback, so it is layered whole: its
//! protection covers `/` and every page and file the fallback serves. When the
//! composition admits a site fallback, that fallback is the tree's instead
//! (public policy only, reserved paths refused), and the static router keeps
//! its routes alone under its own protection.
//!
//! The static router is a public declaration; when an extension declares a
//! `SiteAuthConfig`, it also carries the `SiteAuth` protection layer, which
//! redirects unauthenticated page requests to the extension's login path.
//! With no admitted declaration the static content is served without site
//! auth, and boot says so: a structured `warn!` and a startup warning, since
//! a disabled member's declaration never reaches the kernel.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use axum::routing::get;
use std::sync::Arc;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, ProtectionLayer, RouteGroup, RouteProtection,
};
use systemprompt_runtime::AppContext;
use systemprompt_traits::{StartupEvent, StartupEventSender};

use crate::services::middleware::{
    MountError, MountLayers, RootFallback, protect, protect_root_fallback, protect_site_fallback,
};
use crate::services::static_content::{
    StaticContentMatcher, StaticContentState, serve_homepage, smart_fallback_handler,
};

pub(super) fn build_static_router(
    ctx: &AppContext,
    layers: &MountLayers,
    events: Option<&StartupEventSender>,
    site_fallback: Option<Router>,
) -> Result<RootFallback, MountError> {
    let state = ctx.kernel().state();
    let path = state
        .get::<Arc<systemprompt_config::paths::AppPaths>>()?
        .system()
        .content_config()
        .to_path_buf();
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
        route_classifier: Arc::clone(state.get::<Arc<systemprompt_models::RouteClassifier>>()?),
    };

    let static_router = Router::new().route("/", get(serve_homepage));
    let static_router = if site_fallback.is_some() {
        static_router
    } else {
        static_router.fallback(smart_fallback_handler)
    }
    .with_state(static_state);

    let mut protection = RouteProtection::new(AuthzPolicy::public(), ContextKind::Public);
    if layers.guards.site_auth.is_some() {
        protection = protection.with_layer(ProtectionLayer::SiteAuth);
    } else {
        tracing::warn!(
            mount = systemprompt_extension::reserved::WEB_ROOT_BASE_PATH,
            site_auth = false,
            "static content is served without site auth: no admitted member declares it"
        );
        if let Some(tx) = events
            && tx
                .unbounded_send(StartupEvent::Warning {
                    message: "Static content is served without site auth".to_owned(),
                    context: Some("No admitted member declares site auth".to_owned()),
                })
                .is_err()
        {
            tracing::debug!("Startup event receiver dropped");
        }
    }
    owned_fallback(static_router, &protection, layers, site_fallback)
}

fn owned_fallback(
    static_router: Router,
    protection: &RouteProtection,
    layers: &MountLayers,
    site_fallback: Option<Router>,
) -> Result<RootFallback, MountError> {
    match site_fallback {
        Some(fallback) => Ok(protect_site_fallback(fallback, layers)?.beside(protect(
            static_router,
            protection,
            layers,
            systemprompt_extension::reserved::WEB_ROOT_BASE_PATH,
        )?)),
        None => protect_root_fallback(static_router, protection, layers, RouteGroup::Static),
    }
}
