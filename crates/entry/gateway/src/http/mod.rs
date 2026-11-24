//! The `gateway-http` handlers and the surface that builds them.
//!
//! [`GatewayHttpSurface`] implements
//! [`GatewaySurface`]: from the
//! route-scoped state of `gateway-http` it builds the inference proxy
//! (`/messages`, `/responses`, `/chat/completions`, each bound to an
//! [`InboundAdapter`]), `/messages/count_tokens` and, with the `otel`
//! feature, the credential-gated `/otel` ingest, and — while the `ai` member
//! provides the personal credential view conferred on `gateway-http` — the
//! caller's `/me/provider-credentials` routes (absent, so a confined 404,
//! when it does not), which it returns principal-bound: the API mount places
//! them behind the kernel's gateway verifier and wraps everything in the
//! gateway layering; nothing here verifies a principal, limits, confines or
//! logs access.
//!
//! The bridge routes ([`bridge`]: profile, manifest, plugin files, device
//! enrolment, host preferences, usage, heartbeat, event stream, release feed),
//! `/models` and `/` join the traffic; the bridge profile is principal-bound;
//! the API-key session mint ([`sessions`]) is its own part for the public
//! gateway base.
//!
//! The gateway repositories are built per mount from the route-scoped state
//! ([`gateway_repositories`]), with the at-rest material conferred on
//! `gateway-http`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod bridge;
pub mod count_tokens;
pub mod messages;
pub mod models;
#[cfg(feature = "otel")]
pub mod otel;
pub mod provider_credentials;
pub mod sessions;
pub mod state;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use systemprompt_database::DbPool;
use systemprompt_gateway::protocol::inbound::InboundAdapter;
use systemprompt_gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_gateway::protocol::inbound::openai_chat::OpenAiChatInbound;
use systemprompt_gateway::protocol::inbound::openai_responses::OpenAiResponsesInbound;
use systemprompt_gateway::surface::repositories::gateway_repositories;
use systemprompt_gateway::{
    GatewayEngines, GatewayRouteState, GatewaySurface, GatewaySurfaceError, GatewaySurfaceRoutes,
};
use systemprompt_traits::DynGatewaySessionMint;

use self::bridge::{BridgeMarketplace, BridgeRouteState};
use self::state::{GatewayHttpRepositories, GatewayHttpState};
use systemprompt_gateway::surface::views::{
    BridgeEventStream, BridgeReleaseSource, GatewayManifestSigner, GatewaySettingsView,
};

/// The `gateway-http` member's [`GatewaySurface`].
#[derive(Debug, Clone, Copy, Default)]
pub struct GatewayHttpSurface;

impl GatewaySurface for GatewayHttpSurface {
    fn build(
        &self,
        state: GatewayRouteState<'_>,
    ) -> Result<GatewaySurfaceRoutes, GatewaySurfaceError> {
        let GatewayRouteState { resolved, host } = state;
        let own = resolved.get::<Arc<GatewayHttpRepositories>>()?;
        let db = resolved.get::<DbPool>()?;
        let repos = gateway_repositories(&resolved, &host, Arc::clone(&own.contexts), &db)?;
        let credentials = resolved.optional::<Arc<systemprompt_ai::PersonalProviderCredentials>>();
        let bridge = match (
            resolved.get::<Option<Arc<GatewayManifestSigner>>>()?,
            resolved.get::<Option<Arc<BridgeReleaseSource>>>()?,
            resolved.get::<Option<Arc<GatewaySettingsView>>>()?,
            resolved.get::<Option<Arc<BridgeEventStream>>>()?,
        ) {
            (Some(signer), Some(releases), Some(settings), Some(events)) => {
                Some(BridgeRouteState {
                    authenticator: resolved.get()?,
                    repos: own.bridge.clone(),
                    marketplace: BridgeMarketplace {
                        managed: resolved.get()?,
                        cache: resolved.get()?,
                        seams: resolved.get()?,
                    },
                    signer,
                    releases,
                    settings,
                    events,
                    subjects: resolved.get()?,
                    personal_credentials: credentials.clone().map(
                    |credentials| -> systemprompt_extension::router::DynPersonalCredentialNames {
                        credentials
                    },
                ),
                    host: host.clone(),
                })
            },
            _ => {
                tracing::info!("this host seeds no bridge views; no bridge route is built");
                None
            },
        };
        let session_mint = resolved.get::<DynGatewaySessionMint>()?;
        let state = GatewayHttpState {
            authenticator: resolved.get()?,
            api_keys: own.api_keys.clone(),
            session_mint: Arc::clone(&session_mint),
            repos: Arc::new(repos),
            engines: resolved.get::<Arc<GatewayEngines>>()?,
            host,
            db,
        };
        let bridge_routes = match &bridge {
            Some(bridge) => {
                bridge::routes(bridge).map_err(|e| GatewaySurfaceError::ReleaseFeed(Box::new(e)))?
            },
            None => Router::new(),
        };
        let models_state = models::ModelsState {
            services: state.host.services.clone(),
            settings: bridge.as_ref().map(|bridge| Arc::clone(&bridge.settings)),
        };
        let traffic = routes(&state)
            .merge(bridge_routes)
            .route(
                "/models",
                get(move |headers, query| {
                    let state = models_state.clone();
                    async move { models::list(state, headers, query).await }
                }),
            )
            .route("/", get(models::root));
        let analytics = Arc::clone(&state.host.analytics);
        let credential_routes = credentials.map_or_else(Router::new, |credentials| {
            provider_credentials::routes(&provider_credentials::ProviderCredentialsState {
                gateway: state,
                credentials,
            })
        });
        let credential_routes = match &bridge {
            Some(bridge) => credential_routes.merge(bridge::principal_bound(bridge)),
            None => credential_routes,
        };
        Ok(GatewaySurfaceRoutes {
            traffic,
            principal_bound: credential_routes,
            consumer_catalog: bridge.as_ref().map(bridge::consumer_catalog),
            public_sessions: sessions::public_router(sessions::SessionsState {
                mint: session_mint,
                analytics,
                settings: bridge.map(|bridge| bridge.settings),
            }),
        })
    }
}

fn routes(state: &GatewayHttpState) -> Router {
    let router = Router::new()
        .route(
            "/messages",
            inference(state, Arc::new(AnthropicMessagesInbound)),
        )
        .route(
            "/responses",
            inference(state, Arc::new(OpenAiResponsesInbound)),
        )
        .route(
            "/chat/completions",
            inference(state, Arc::new(OpenAiChatInbound)),
        )
        .route("/messages/count_tokens", token_count(state));
    #[cfg(feature = "otel")]
    let router = otel_routes(router, state);
    router
}

fn token_count(state: &GatewayHttpState) -> axum::routing::MethodRouter {
    let state = state.clone();
    post(move |request| {
        let state = state.clone();
        async move { count_tokens::handle(state, request).await }
    })
}

fn inference(
    state: &GatewayHttpState,
    inbound: Arc<dyn InboundAdapter>,
) -> axum::routing::MethodRouter {
    let state = state.clone();
    post(move |request| {
        let state = state.clone();
        let inbound = Arc::clone(&inbound);
        async move { messages::handle(inbound, state, request).await }
    })
}

#[cfg(feature = "otel")]
fn otel_routes(router: Router, state: &GatewayHttpState) -> Router {
    use systemprompt_models::ApiPaths;
    router
        .route(ApiPaths::OTEL, otel_ingest(state))
        .route(ApiPaths::OTEL_REST_ROUTE, otel_ingest(state))
}

#[cfg(feature = "otel")]
fn otel_ingest(state: &GatewayHttpState) -> axum::routing::MethodRouter {
    let state = state.clone();
    post(move |request| {
        let state = state.clone();
        async move { otel::handle(state, request).await }
    })
}
