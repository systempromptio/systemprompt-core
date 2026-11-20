//! The marketplace HTTP surface.
//!
//! One route set [`crate::extension::MarketplaceHttpExtension`] contributes
//! under `/api/v1/marketplace`, behind the public gate with a public request
//! context, in the content-and-misc role group under the `marketplace`
//! rate-limit scope with the content budget and the default body limit.
//!
//! - `GET /marketplace.json` — the default marketplace;
//! - `GET /marketplaces` and `GET /marketplaces/{id}` — the enabled
//!   marketplaces as JSON;
//! - `GET /marketplaces/{id}/manifest.yaml` — a marketplace's raw
//!   `config.yaml`, confined to the marketplaces directory of the services
//!   root;
//! - `GET /plugins/{plugin_id}/{*path}` — a plugin file from the scoped,
//!   content-gated plugin bundles, without the organization's managed
//!   resources.
//!
//! Every handler reads [`MarketplaceHttpState`]: the services view and the
//! services root, the API's external URL, the catalogue cache and the managed
//! repository the `managed_resources` member provides, and the system admin
//! that owns the managed inventory.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod error;
pub mod handlers;

pub use error::{MarketplaceHttpError, MarketplaceRouteError};
pub use handlers::{
    get_marketplace, get_marketplace_yaml, list_marketplaces, serve_default_marketplace_json,
    serve_plugin_file,
};

use std::path::PathBuf;
use std::sync::Arc;

use axum::http::Method;
use systemprompt_config::paths::AppPaths;
use systemprompt_extension::router::{
    ApiBase, BasePath, BodyLimit, RouteLimits, RouteSet, RouteSpec,
};
use systemprompt_extension::{
    AuthzPolicy, ContextKind, FromState, MissingDependency, RateBudget, RateLimit, Resolved,
    RouteGroup,
};
use systemprompt_identifiers::UserId;
use systemprompt_manifest::Config;
use systemprompt_manifest::services::{ServicesView, SystemAdmin};
use systemprompt_marketplace::MarketplaceCache;
use systemprompt_marketplace::managed::ManagedRepository;

const LIMITS: RouteLimits = RouteLimits {
    rate: RateLimit::new("marketplace", RateBudget::Content),
    body: BodyLimit::Default,
};
const PUBLIC: RouteSpec = RouteSpec::new(AuthzPolicy::public(), ContextKind::Public, LIMITS);

/// Router-scoped state of the marketplace routes.
#[derive(Clone)]
pub struct MarketplaceHttpState {
    pub(crate) services: ServicesView,
    pub(crate) services_root: PathBuf,
    pub(crate) api_external_url: Arc<str>,
    pub(crate) cache: Arc<MarketplaceCache>,
    pub(crate) repository: Arc<ManagedRepository>,
    pub(crate) owner: UserId,
}

impl std::fmt::Debug for MarketplaceHttpState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MarketplaceHttpState")
            .field("services_root", &self.services_root)
            .field("api_external_url", &self.api_external_url)
            .field("owner", &self.owner)
            .finish_non_exhaustive()
    }
}

impl FromState for MarketplaceHttpState {
    fn from_state(r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        let paths = r.get::<Arc<AppPaths>>()?;
        let config = r.get::<Arc<Config>>()?;
        Ok(Self {
            services: r.get()?,
            services_root: paths.system().services().to_path_buf(),
            api_external_url: Arc::from(config.api_external_url.as_str()),
            cache: r.get()?,
            repository: r.get()?,
            owner: r.get::<SystemAdmin>()?.id().clone(),
        })
    }
}

pub fn public_routes() -> RouteSet {
    RouteSet::at::<MarketplaceHttpState>(
        BasePath::Api(ApiBase::Marketplace),
        RouteGroup::ContentAndMisc,
    )
    .route(
        Method::GET,
        "/marketplace.json",
        serve_default_marketplace_json,
        PUBLIC,
    )
    .route(Method::GET, "/marketplaces", list_marketplaces, PUBLIC)
    .route(Method::GET, "/marketplaces/{id}", get_marketplace, PUBLIC)
    .route(
        Method::GET,
        "/marketplaces/{id}/manifest.yaml",
        get_marketplace_yaml,
        PUBLIC,
    )
    .route(
        Method::GET,
        "/plugins/{plugin_id}/{*path}",
        serve_plugin_file,
        PUBLIC,
    )
    .build()
}
