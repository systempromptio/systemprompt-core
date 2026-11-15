//! [`AnalyticsHttpExtension`]: declares the analytics and engagement route
//! groups.
//!
//! `/api/v1/analytics` (event ingestion and the live stream) sits behind the
//! admin gate with a user context; `/track/engagement` is public. Both are in
//! the content-and-misc role group under the `content` rate-limit scope and
//! budget, with the default body limit and no extra layers. The engagement
//! group is declared and nested at `/track/engagement`, a base path outside
//! `/api/` that only its core declaration row's `WebPath` allowance admits.
//! Each declaration equals its row in the API's core declaration table
//! (`Analytics`, `Engagement`), keyed on this extension's id, `analytics-http`,
//! and the table requires exactly these two declarations from it.
//!
//! The routes read the analytics repositories the API kernel seeds, the
//! loaded content configuration when there is one, and the content catalog
//! the `content` extension provides, which this extension `requires`. The
//! live stream's analytics broadcaster is not in the extension state: the API
//! layers it onto the analytics router alone because the `Analytics` row is
//! handed it, and the stream answers 503 without it:
//!
//! | profile disables | analytics | engagement |
//! |---|---|---|
//! | `analytics-http` | 404 | 404 |
//! | `content` | boot refused | boot refused |
//! | `analytics` | boot refused (required extension) | |
//!
//! Disabling `analytics-http` leaves request recording and the analytics jobs
//! untouched: they belong to the `analytics` extension, which owns the
//! protective jobs and so cannot be disabled.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::RouteGroup;
use systemprompt_extension::prelude::*;
use systemprompt_models::modules::ApiPaths;
use systemprompt_traits::DynContentCatalogStats;

use crate::http::{AnalyticsHttpState, analytics, engagement};

pub const ANALYTICS_HTTP_EXTENSION: &str = "analytics-http";

const RATE_LIMIT: RateLimit = RateLimit::new("content", RateBudget::Content);

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

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["analytics", "content"]
    }

    fn requires(&self) -> Vec<StateRequirement> {
        vec![StateRequirement::of::<DynContentCatalogStats>()]
    }

    fn routers(
        &self,
        ctx: &dyn ExtensionContext,
    ) -> Result<Vec<ExtensionRouter>, ExtensionStateError> {
        let state = AnalyticsHttpState::from_extension_state(&ctx.extension_state())?;
        Ok(vec![
            ExtensionRouter::new(
                analytics::router(state.clone()),
                ApiPaths::ANALYTICS_BASE,
                AuthzPolicy::admin(),
                ContextKind::User,
            )
            .in_group(RouteGroup::ContentAndMisc)
            .with_rate_limit(RATE_LIMIT),
            ExtensionRouter::new(
                engagement::router(state),
                ApiPaths::TRACK_ENGAGEMENT,
                AuthzPolicy::public(),
                ContextKind::Public,
            )
            .in_group(RouteGroup::ContentAndMisc)
            .with_rate_limit(RATE_LIMIT),
        ])
    }
}

register_extension!(AnalyticsHttpExtension);
