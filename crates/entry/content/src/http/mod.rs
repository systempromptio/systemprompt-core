//! The content HTTP surface: three route sets
//! [`crate::extension::ContentHttpExtension`] contributes.
//!
//! - [`public_routes`] — published content by source and slug (JSON, or
//!   Markdown through content negotiation), search, and link analytics, under
//!   `/api/v1/content` with no authentication.
//! - [`authenticated_routes`] — trackable-link generation under
//!   `/api/v1/content` for a signed-in user.
//! - [`redirect_routes`] — the short-link redirect `/r/{short_code}` at the web
//!   root, which records a click for every session that is not a bot session.
//!
//! All three are in the content-and-misc role group under the `content`
//! rate-limit scope and budget with the default body limit. Every handler
//! reads [`ContentHttpState`]: the content repositories the `content` member
//! provides, and the [`ContentHttpSettings`] the `content-http` member's own
//! provide fn builds from the application configuration — the API's external
//! URL and the content-negotiation settings, plain values with no capability.
//! The route state reads no host seed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod blog;
mod error;
pub mod links;
pub mod query;

pub use error::ContentHttpError;

use std::sync::Arc;

use axum::http::Method;
use systemprompt_content::repository::ContentRepositories;
use systemprompt_extension::router::{
    ApiBase, BasePath, BodyLimit, RouteLimits, RouteSet, RouteSpec, WebBase,
};
use systemprompt_extension::{
    AuthzPolicy, ContextKind, FromState, MissingDependency, RateBudget, RateLimit, Resolved,
    RouteGroup,
};
use systemprompt_manifest::Config;
use systemprompt_manifest::profile::ContentNegotiationConfig;

pub use blog::{get_content_handler, get_content_markdown_handler, list_content_by_source_handler};
pub use links::{
    generate_link_handler, get_campaign_performance_handler, get_content_journey_handler,
    get_link_clicks_handler, get_link_performance_handler, list_links_handler, redirect_handler,
};
pub use query::query_handler;

const LIMITS: RouteLimits = RouteLimits {
    rate: RateLimit::new("content", RateBudget::Content),
    body: BodyLimit::Default,
};
const PUBLIC: RouteSpec = RouteSpec::new(AuthzPolicy::public(), ContextKind::Public, LIMITS);
const USER: RouteSpec = RouteSpec::new(AuthzPolicy::user(), ContextKind::User, LIMITS);

/// The application settings the content routes read, copied out of the
/// configuration by the member's provide fn.
#[derive(Debug, Clone)]
pub struct ContentHttpSettings {
    api_external_url: Arc<str>,
    content_negotiation: Arc<ContentNegotiationConfig>,
}

impl ContentHttpSettings {
    #[must_use]
    pub fn of(config: &Config) -> Self {
        Self {
            api_external_url: Arc::from(config.api_external_url.as_str()),
            content_negotiation: Arc::new(config.content_negotiation.clone()),
        }
    }
}

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
    pub fn new(repositories: Arc<ContentRepositories>, settings: ContentHttpSettings) -> Self {
        Self {
            repositories,
            api_external_url: settings.api_external_url,
            content_negotiation: settings.content_negotiation,
        }
    }

    fn markdown_suffix(&self) -> &str {
        self.content_negotiation
            .markdown_suffix
            .trim_start_matches('.')
    }
}

impl FromState for ContentHttpState {
    fn from_state(r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        Ok(Self::new(r.get()?, r.get()?))
    }
}

pub fn public_routes() -> RouteSet {
    RouteSet::at::<ContentHttpState>(BasePath::Api(ApiBase::Content), RouteGroup::ContentAndMisc)
        .route(Method::POST, "/query", query_handler, PUBLIC)
        .route(
            Method::GET,
            "/{source_id}",
            list_content_by_source_handler,
            PUBLIC,
        )
        .route(
            Method::GET,
            "/{source_id}/{slug}",
            get_content_handler,
            PUBLIC,
        )
        .route(
            Method::GET,
            "/{source_id}/{slug}/{suffix}",
            get_content_markdown_handler,
            PUBLIC,
        )
        .route(Method::GET, "/links", list_links_handler, PUBLIC)
        .route(
            Method::GET,
            "/links/{link_id}/performance",
            get_link_performance_handler,
            PUBLIC,
        )
        .route(
            Method::GET,
            "/links/{link_id}/clicks",
            get_link_clicks_handler,
            PUBLIC,
        )
        .route(
            Method::GET,
            "/links/campaigns/{campaign_id}/performance",
            get_campaign_performance_handler,
            PUBLIC,
        )
        .route(
            Method::GET,
            "/links/journey",
            get_content_journey_handler,
            PUBLIC,
        )
        .build()
}

pub fn authenticated_routes() -> RouteSet {
    RouteSet::at::<ContentHttpState>(BasePath::Api(ApiBase::Content), RouteGroup::ContentAndMisc)
        .route(Method::POST, "/links/generate", generate_link_handler, USER)
        .build()
}

pub fn redirect_routes() -> RouteSet {
    RouteSet::at::<ContentHttpState>(BasePath::Web(WebBase::Root), RouteGroup::ContentAndMisc)
        .route(Method::GET, "/r/{short_code}", redirect_handler, PUBLIC)
        .build()
}
