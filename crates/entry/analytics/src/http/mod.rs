//! The analytics HTTP surface: event ingestion and the live stream under
//! `/api/v1/analytics` ([`analytics`]) and engagement ingestion under
//! `/track/engagement` ([`engagement`]).
//!
//! Both resolve their handles through a hand-written [`FromState`]: the
//! analytics repositories (session store included) the kernel seeds, the
//! loaded content configuration when one is configured, and the content
//! catalog the `content` extension provides. The engagement group's
//! [`EngagementState`] holds only those. [`AnalyticsHttpState`] adds the
//! analytics broadcaster, which the kernel hands only to a route set at the
//! `/api/v1/analytics` base its conferral names; it is never in the shared
//! state. An event that names no content id gets one
//! from [`ContentResolver`]: the slug it carries, or the slug the loaded
//! content configuration resolves its page URL to, looked up in that catalog. A
//! failed lookup is logged and leaves the event without a content id; it never
//! fails the request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod analytics;
pub mod engagement;

use std::sync::Arc;

use systemprompt_analytics::repository::AnalyticsRepositories;
use systemprompt_extension::contribute::handles::AnalyticsBroadcaster;
use systemprompt_extension::{FromState, Handed, MissingDependency, Resolved};
use systemprompt_identifiers::{ContentId, LocaleCode};
use systemprompt_models::{ContentConfigRaw, ContentRouting};
use systemprompt_traits::DynContentCatalogStats;

/// Resolves the content id an ingested event belongs to, from its slug or
/// its page URL.
#[derive(Clone)]
pub struct ContentResolver {
    catalog: DynContentCatalogStats,
    routing: Option<Arc<dyn ContentRouting>>,
}

impl std::fmt::Debug for ContentResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentResolver")
            .field("routing", &self.routing.is_some())
            .finish_non_exhaustive()
    }
}

impl ContentResolver {
    pub fn new(catalog: DynContentCatalogStats, routing: Option<Arc<dyn ContentRouting>>) -> Self {
        Self { catalog, routing }
    }

    pub async fn resolve(&self, page_url: &str, slug: Option<&str>) -> Option<ContentId> {
        let resolved_slug;
        let slug = if let Some(slug) = slug {
            slug
        } else {
            resolved_slug = self.routing.as_deref()?.resolve_slug(page_url)?;
            &resolved_slug
        };
        self.catalog
            .content_id_by_slug(slug, &LocaleCode::english())
            .await
            .inspect_err(|e| {
                tracing::warn!(error = %e, slug = %slug, "Failed to lookup content by slug");
            })
            .ok()
            .flatten()
    }
}

/// The handles the engagement routes are built from: the repositories and
/// the content resolver, without the analytics broadcaster.
#[derive(Clone)]
pub struct EngagementState {
    pub(crate) repositories: Arc<AnalyticsRepositories>,
    pub(crate) content: ContentResolver,
}

impl std::fmt::Debug for EngagementState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngagementState")
            .field("content", &self.content)
            .finish_non_exhaustive()
    }
}

impl FromState for EngagementState {
    fn from_state(r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        let routing = r
            .optional::<Arc<ContentConfigRaw>>()
            .map(|config| -> Arc<dyn ContentRouting> { config });
        Ok(Self {
            repositories: r.get()?,
            content: ContentResolver::new(r.get()?, routing),
        })
    }
}

/// The handles the `/api/v1/analytics` routes are built from: the
/// engagement handles plus the analytics broadcaster the kernel hands this
/// route set.
#[derive(Clone)]
pub struct AnalyticsHttpState {
    pub(crate) ingest: EngagementState,
    pub(crate) broadcaster: Handed<AnalyticsBroadcaster>,
}

impl std::fmt::Debug for AnalyticsHttpState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalyticsHttpState")
            .field("ingest", &self.ingest)
            .field("broadcaster", &self.broadcaster)
            .finish()
    }
}

impl FromState for AnalyticsHttpState {
    fn from_state(r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        Ok(Self {
            ingest: EngagementState::from_state(r)?,
            broadcaster: Handed::from_state(r)?,
        })
    }
}
