//! [`AnalyticsHttpExtension`] (`analytics-http`): the analytics and engagement
//! route sets and the analytics CLI.
//!
//! `/api/v1/analytics` (event ingestion and the live stream) sits behind the
//! admin gate with a user context; `/track/engagement` is public. Both are in
//! the content-and-misc role group under the `content` rate-limit scope and
//! budget, with the default body limit. Each set's shape equals the
//! reference composition's conferral for `analytics-http` at its base, and
//! only the `/api/v1/analytics` set is handed the analytics broadcaster. The
//! `analytics` command group requests the local-export and skip-validation
//! grants its reserved CLI path confers; with `content-commands`, the
//! `popular` leaf attaches under `core content`.
//!
//! The routes read the analytics repositories the `analytics` member
//! provides, the loaded content configuration when there is one, and the
//! content catalog the `content` extension provides, which this extension
//! `requires`:
//!
//! | profile disables | analytics | engagement | CLI |
//! |---|---|---|---|
//! | `analytics-http` | 404 | 404 | refused at dispatch |
//! | `content` | boot refused | boot refused | |
//! | `analytics` | boot refused (required member) | | |
//!
//! Disabling `analytics-http` leaves request recording and the analytics jobs
//! untouched: they belong to the `analytics` member, which owns the
//! protective jobs and so cannot be disabled.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

pub const ANALYTICS_HTTP_EXTENSION: &str = "analytics-http";

#[derive(Debug, Clone, Copy, Default)]
pub struct AnalyticsHttpExtension;

impl Extension for AnalyticsHttpExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: ANALYTICS_HTTP_EXTENSION,
            name: "Analytics HTTP",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        #[cfg(feature = "http")]
        routes::contribute(c);
        #[cfg(feature = "cli")]
        commands::contribute(c);
        Ok(())
    }
}

#[cfg(feature = "http")]
mod routes {
    use axum::http::Method;
    use systemprompt_extension::RouteGroup;
    use systemprompt_extension::prelude::*;
    use systemprompt_extension::router::{
        ApiBase, BasePath, BodyLimit, RouteLimits, RouteSet, RouteSpec, WebBase,
    };
    use systemprompt_traits::DynContentCatalogStats;

    use crate::http::{AnalyticsHttpState, EngagementState, analytics, engagement};

    const LIMITS: RouteLimits = RouteLimits {
        rate: RateLimit::new("content", RateBudget::Content),
        body: BodyLimit::Default,
    };
    const ADMIN: RouteSpec = RouteSpec::new(AuthzPolicy::admin(), ContextKind::User, LIMITS);
    const PUBLIC: RouteSpec = RouteSpec::new(AuthzPolicy::public(), ContextKind::Public, LIMITS);

    pub(super) fn contribute(c: &mut Contributions<'_>) {
        c.requires::<DynContentCatalogStats>()
            .routes(
                RouteSet::at::<AnalyticsHttpState>(
                    BasePath::Api(ApiBase::Analytics),
                    RouteGroup::ContentAndMisc,
                )
                .route(Method::POST, "/events", analytics::record_event, ADMIN)
                .route(
                    Method::POST,
                    "/events/batch",
                    analytics::record_events_batch,
                    ADMIN,
                )
                .route(Method::GET, "/stream", analytics::analytics_stream, ADMIN)
                .build(),
            )
            .routes(
                RouteSet::at::<EngagementState>(
                    BasePath::Web(WebBase::TrackEngagement),
                    RouteGroup::ContentAndMisc,
                )
                .route(Method::POST, "/", engagement::record_engagement, PUBLIC)
                .route(
                    Method::POST,
                    "/batch",
                    engagement::record_engagement_batch,
                    PUBLIC,
                )
                .build(),
            );
    }
}

#[cfg(feature = "cli")]
mod commands {
    use systemprompt_extension::cli::{CommandDecl, KernelGrant, parents};
    use systemprompt_extension::prelude::*;

    use crate::cli::AnalyticsCommands;

    pub(super) fn contribute(c: &mut Contributions<'_>) {
        c.command(
            CommandDecl::group::<AnalyticsCommands>()
                .requesting(&[KernelGrant::LocalExport, KernelGrant::SkipValidation]),
        );
        #[cfg(feature = "content-commands")]
        c.command(CommandDecl::leaf_under::<
            parents::ContentGroup,
            crate::cli::content_popular::PopularArgs,
        >());
    }
}
