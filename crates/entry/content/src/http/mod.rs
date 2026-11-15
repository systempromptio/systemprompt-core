//! The content HTTP surface: three route groups the API server mounts from
//! [`crate::extension::ContentHttpExtension`].
//!
//! - [`public_router`] — published content by source and slug (JSON, or
//!   Markdown through content negotiation), search, and link analytics, under
//!   `/api/v1/content` with no authentication.
//! - [`authenticated_router`] — trackable-link generation under
//!   `/api/v1/content` for a signed-in user.
//! - [`redirect_router`] — the short-link redirect `/r/{short_code}` at the web
//!   root, which records a click for every session that is not a bot session.
//!
//! Every handler reads [`ContentHttpState`]: the content repositories the
//! `content` extension provides, and the API's external URL and
//! content-negotiation settings from the application configuration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod blog;
mod error;
pub mod links;
pub mod query;

pub use error::ContentHttpError;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use systemprompt_content::repository::ContentRepositories;
use systemprompt_manifest::Config;
use systemprompt_manifest::profile::ContentNegotiationConfig;
use systemprompt_provider_contracts::{Dependencies, MissingDependency};

pub use blog::{get_content_handler, get_content_markdown_handler, list_content_by_source_handler};
pub use links::{
    generate_link_handler, get_campaign_performance_handler, get_content_journey_handler,
    get_link_clicks_handler, get_link_performance_handler, list_links_handler, redirect_handler,
};
pub use query::query_handler;

/// Router-scoped state of the content routes.
#[derive(Clone)]
pub struct ContentHttpState {
    pub(crate) repositories: Arc<ContentRepositories>,
    pub(crate) api_external_url: Arc<str>,
    pub(crate) content_negotiation: Arc<ContentNegotiationConfig>,
}

impl std::fmt::Debug for ContentHttpState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentHttpState")
            .field("api_external_url", &self.api_external_url)
            .field("content_negotiation", &self.content_negotiation)
            .finish_non_exhaustive()
    }
}

impl ContentHttpState {
    pub fn new(repositories: Arc<ContentRepositories>, config: &Config) -> Self {
        Self {
            repositories,
            api_external_url: Arc::from(config.api_external_url.as_str()),
            content_negotiation: Arc::new(config.content_negotiation.clone()),
        }
    }

    pub fn from_extension_state(
        state: &Dependencies,
        config: &Config,
    ) -> Result<Self, MissingDependency> {
        let repositories = Arc::clone(state.get::<Arc<ContentRepositories>>()?);
        Ok(Self::new(repositories, config))
    }

    fn markdown_suffix(&self) -> &str {
        self.content_negotiation
            .markdown_suffix
            .trim_start_matches('.')
    }
}

pub fn public_router(state: ContentHttpState) -> Router {
    let mut router = Router::new()
        .route("/query", post(query_handler))
        .route("/{source_id}", get(list_content_by_source_handler))
        .route("/{source_id}/{slug}", get(get_content_handler))
        .route("/links", get(list_links_handler))
        .route(
            "/links/{link_id}/performance",
            get(get_link_performance_handler),
        )
        .route("/links/{link_id}/clicks", get(get_link_clicks_handler))
        .route(
            "/links/campaigns/{campaign_id}/performance",
            get(get_campaign_performance_handler),
        )
        .route("/links/journey", get(get_content_journey_handler));

    if state.content_negotiation.enabled {
        let suffix = state.markdown_suffix();
        router = router.route(
            &format!("/{{source_id}}/{{slug}}/{}", suffix),
            get(get_content_markdown_handler),
        );
    }

    router.with_state(state)
}

pub fn authenticated_router(state: ContentHttpState) -> Router {
    Router::new()
        .route("/links/generate", post(generate_link_handler))
        .with_state(state)
}

pub fn redirect_router(state: ContentHttpState) -> Router {
    Router::new()
        .route("/r/{short_code}", get(redirect_handler))
        .with_state(state)
}
