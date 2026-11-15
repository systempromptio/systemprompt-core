//! The analytics HTTP surface: event ingestion and the live stream under
//! `/api/v1/analytics` ([`analytics`]) and engagement ingestion under
//! `/track/engagement` ([`engagement`]).
//!
//! Both read their handles from the extension state through
//! [`AnalyticsHttpState`]: the analytics repositories (session store
//! included) and the analytics broadcaster the API kernel seeds, the loaded
//! content configuration when one is configured, and the content catalog the
//! `content` extension provides. An event that names no content id gets one from
//! [`ContentResolver`]: the slug it carries, or the slug the loaded content
//! configuration resolves its page URL to, looked up in that catalog. A failed
//! lookup is logged and leaves the event without a content id; it never fails
//! the request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod analytics;
pub mod engagement;

use std::sync::Arc;

use systemprompt_analytics::repository::AnalyticsRepositories;
use systemprompt_events::AnalyticsBroadcaster;
use systemprompt_identifiers::{ContentId, LocaleCode};
use systemprompt_models::{ContentConfigRaw, ContentRouting};
use systemprompt_provider_contracts::{Dependencies, MissingDependency};
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

/// The handles both analytics route groups are built from, read once from
/// the extension state.
#[derive(Clone)]
pub struct AnalyticsHttpState {
    pub(crate) repositories: Arc<AnalyticsRepositories>,
    pub(crate) broadcaster: Arc<AnalyticsBroadcaster>,
    pub(crate) content: ContentResolver,
}

impl std::fmt::Debug for AnalyticsHttpState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalyticsHttpState")
            .field("content", &self.content)
            .finish_non_exhaustive()
    }
}

impl AnalyticsHttpState {
    pub fn from_extension_state(state: &Dependencies) -> Result<Self, MissingDependency> {
        let routing = state
            .find::<Arc<ContentConfigRaw>>()
            .map(|config| Arc::clone(config) as Arc<dyn ContentRouting>);
        Ok(Self {
            repositories: Arc::clone(state.get::<Arc<AnalyticsRepositories>>()?),
            broadcaster: Arc::clone(state.get::<Arc<AnalyticsBroadcaster>>()?),
            content: ContentResolver::new(
                Arc::clone(state.get::<DynContentCatalogStats>()?),
                routing,
            ),
        })
    }
}
