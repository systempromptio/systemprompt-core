//! The bridge routes `gateway-http` serves: profile, manifest, plugin files,
//! device enrolment, host preferences, usage, heartbeat, the live event
//! stream and the release feed.
//!
//! [`BridgeRouteState`] is built once per mount from the `gateway-http` route
//! state. It holds the request authenticator (every handler verifies the
//! bridge bearer through it), the narrow views the `gateway` member confers on
//! `gateway-http` ([`systemprompt_gateway::surface::views`]), the
//! repositories the member builds in its own provide, the marketplace state
//! the `managed_resources` member provides, and the host handles the API mount
//! passes. Nothing in it is the application context. The routes verify their
//! own callers; the API mount owns the rate limit, the confinement and the
//! access log around them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod admission;
pub mod data;
pub mod device;
pub mod error;
pub mod heartbeat;
pub mod manifest;
pub mod plugin_archive;
pub mod plugin_file;
pub mod profile;
pub mod profile_usage;
pub mod release;
pub mod resolved;
pub mod stream;
pub mod whoami;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use systemprompt_analytics::ProfileUsageService;
use systemprompt_extension::router::{DynPersonalCredentialNames, GatewayPrincipal};
use systemprompt_gateway::GatewayHostHandles;
use systemprompt_gateway::surface::views::{
    BridgeEventStream, BridgeReleaseSource, GatewayManifestSigner, GatewaySettingsView,
};
use systemprompt_governance::authz::SubjectProviderSet;
use systemprompt_identifiers::UserId;
use systemprompt_marketplace::contract::MarketplaceSeams;
use systemprompt_marketplace::managed::ManagedRepository;
use systemprompt_marketplace::surface::{ConsumerCatalog, DynConsumerCatalog};
use systemprompt_marketplace::{MarketplaceCache, MarketplaceCandidate};
use systemprompt_models::ApiPaths;
use systemprompt_models::auth::DynRequestAuthenticator;
use systemprompt_oauth::repository::{BridgeHostPrefsRepository, BridgeSessionRepository};
use systemprompt_security::http::resolve_client_ip;
use systemprompt_traits::BoxedSource;
use systemprompt_users::UserRepository;

use self::plugin_archive::ArchiveMemo;
use self::release::{ReleaseError, ReleaseFeed};

/// The repositories the bridge routes read that `gateway-http` builds in its
/// own provide.
#[derive(Clone)]
pub struct BridgeRepositories {
    pub users: Arc<UserRepository>,
    pub host_prefs: BridgeHostPrefsRepository,
    pub sessions: BridgeSessionRepository,
    pub profile_usage: Arc<ProfileUsageService>,
}

impl std::fmt::Debug for BridgeRepositories {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeRepositories").finish_non_exhaustive()
    }
}

/// The marketplace state the `managed_resources` member provides.
#[derive(Clone)]
pub struct BridgeMarketplace {
    pub managed: Arc<ManagedRepository>,
    pub cache: Arc<MarketplaceCache>,
    pub seams: Arc<MarketplaceSeams>,
}

impl std::fmt::Debug for BridgeMarketplace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeMarketplace").finish_non_exhaustive()
    }
}

/// What every bridge handler reads.
#[derive(Clone)]
pub struct BridgeRouteState {
    pub authenticator: DynRequestAuthenticator,
    pub repos: BridgeRepositories,
    pub marketplace: BridgeMarketplace,
    pub signer: Arc<GatewayManifestSigner>,
    pub releases: Arc<BridgeReleaseSource>,
    pub settings: Arc<GatewaySettingsView>,
    pub events: Arc<BridgeEventStream>,
    pub subjects: Arc<SubjectProviderSet>,
    pub personal_credentials: Option<DynPersonalCredentialNames>,
    pub host: GatewayHostHandles,
}

impl std::fmt::Debug for BridgeRouteState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeRouteState").finish_non_exhaustive()
    }
}

macro_rules! route_with {
    ($state:expr, $method:ident, $handler:path) => {{
        let state = $state.clone();
        $method(move |headers, body| {
            let state = state.clone();
            async move { $handler(state, headers, body).await }
        })
    }};
    ($state:expr, $method:ident, $handler:path, headers) => {{
        let state = $state.clone();
        $method(move |headers| {
            let state = state.clone();
            async move { $handler(state, headers).await }
        })
    }};
}

pub fn client_ip(
    request: &axum::extract::Request,
    settings: &GatewaySettingsView,
) -> Option<std::net::IpAddr> {
    resolve_client_ip(
        request.headers(),
        request
            .extensions()
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>(),
        settings.trusted_proxies(),
    )
}

pub fn routes(state: &BridgeRouteState) -> Result<Router, ReleaseError> {
    let pubkey = Arc::clone(&state.signer);
    let memo = ArchiveMemo::default();
    let file_state = state.clone();
    let feed = Arc::new(ReleaseFeed::new()?);
    let latest_state = (state.clone(), Arc::clone(&feed));
    let download_state = (state.clone(), feed);
    Ok(Router::new()
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_PUBKEY),
            get(move || {
                let signer = Arc::clone(&pubkey);
                async move { profile::pubkey(&signer) }
            }),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_WHOAMI),
            route_with!(state, get, whoami::handle, headers),
        )
        .route(ApiPaths::gateway_relative(ApiPaths::BRIDGE_MANIFEST), {
            let state = state.clone();
            get(move |headers| {
                let state = state.clone();
                Box::pin(manifest::manifest(state, headers))
            })
        })
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_PLUGIN_FILE_ROUTE),
            get(move |headers, path| {
                let state = file_state.clone();
                let memo = memo.clone();
                async move { plugin_file::handle(memo, state, headers, path).await }
            }),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_DEVICE),
            route_with!(state, post, device::enroll_self),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_PROFILE_ENABLED_HOSTS),
            route_with!(state, post, profile::set_enabled_host),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_PROFILE_HOST_MODEL_FILTER),
            route_with!(state, post, profile::set_host_model_filter),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_PROFILE_USAGE),
            route_with!(state, get, profile_usage::handle, headers),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_HEARTBEAT),
            route_with!(state, post, heartbeat::handle),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_STREAM),
            route_with!(state, get, stream::handle, headers),
        )
        .route("/models", {
            let state = state.clone();
            get(move |headers, query| {
                let state = state.clone();
                async move { super::models::list(state, headers, query).await }
            })
        })
        .route("/", get(super::models::root))
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_LATEST),
            get(move |headers, query| {
                let (state, feed) = latest_state.clone();
                async move { release::latest(state, feed, headers, query).await }
            }),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_DOWNLOAD_ROUTE),
            get(move |headers, path| {
                let (state, feed) = download_state.clone();
                async move { release::download(state, feed, headers, path).await }
            }),
        ))
}

/// The bridge profile route: the API mount places it behind the kernel's
/// gateway verifier, which attaches the caller's `GatewayPrincipal`.
pub fn principal_bound(state: &BridgeRouteState) -> Router {
    let state = state.clone();
    Router::new().route(
        ApiPaths::gateway_relative(ApiPaths::BRIDGE_PROFILE),
        get(
            move |axum::Extension(caller): axum::Extension<GatewayPrincipal>, headers| {
                let state = state.clone();
                async move { profile::profile(state, caller, headers).await }
            },
        ),
    )
}

struct BridgeConsumerCatalog {
    state: BridgeRouteState,
}

#[async_trait::async_trait]
impl ConsumerCatalog for BridgeConsumerCatalog {
    async fn candidate(&self, consumer: &UserId) -> Result<MarketplaceCandidate, BoxedSource> {
        let services = self.state.host.services.view().config();
        let (candidate, _) = manifest::assemble_candidate(
            &self.state,
            consumer,
            (*services).clone(),
            resolved::Freshness::Memo,
        )
        .await
        .map_err(|error| -> BoxedSource { Box::new(error) })?;
        Ok(candidate)
    }
}

/// The marketplace candidate a consumer's bridge manifest resolves to, for the
/// `marketplace-http` consumer routes.
#[must_use]
pub fn consumer_catalog(state: &BridgeRouteState) -> DynConsumerCatalog {
    Arc::new(BridgeConsumerCatalog {
        state: state.clone(),
    })
}
