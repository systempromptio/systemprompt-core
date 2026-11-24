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
//! when it does not). The API mount wraps what it
//! returns in the gateway layering; nothing here limits, confines or logs
//! access.
//!
//! The gateway repositories are built per mount from the route-scoped state
//! ([`gateway_repositories`]), with the at-rest material conferred on
//! `gateway-http`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod count_tokens;
pub mod messages;
#[cfg(feature = "otel")]
pub mod otel;
pub mod provider_credentials;
pub mod state;

use std::sync::Arc;

use axum::Router;
use axum::routing::post;
use systemprompt_database::DbPool;
use systemprompt_gateway::protocol::inbound::InboundAdapter;
use systemprompt_gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_gateway::protocol::inbound::openai_chat::OpenAiChatInbound;
use systemprompt_gateway::protocol::inbound::openai_responses::OpenAiResponsesInbound;
use systemprompt_gateway::surface::repositories::gateway_repositories;
use systemprompt_gateway::{
    GatewayEngines, GatewayRouteState, GatewaySurface, GatewaySurfaceError,
};
use systemprompt_traits::DynGatewaySessionMint;

use self::state::{GatewayHttpRepositories, GatewayHttpState};

/// The `gateway-http` member's [`GatewaySurface`].
#[derive(Debug, Clone, Copy, Default)]
pub struct GatewayHttpSurface;

impl GatewaySurface for GatewayHttpSurface {
    fn build(&self, state: GatewayRouteState<'_>) -> Result<Router, GatewaySurfaceError> {
        let GatewayRouteState { resolved, host } = state;
        let own = resolved.get::<Arc<GatewayHttpRepositories>>()?;
        let db = resolved.get::<DbPool>()?;
        let repos = gateway_repositories(&resolved, &host, Arc::clone(&own.contexts), &db)?;
        let credentials = resolved.optional::<Arc<systemprompt_ai::PersonalProviderCredentials>>();
        let state = GatewayHttpState {
            authenticator: resolved.get()?,
            api_keys: own.api_keys.clone(),
            session_mint: resolved.get::<DynGatewaySessionMint>()?,
            repos: Arc::new(repos),
            engines: resolved.get::<Arc<GatewayEngines>>()?,
            host,
            db,
        };
        let router = routes(&state);
        Ok(match credentials {
            Some(credentials) => router.merge(provider_credentials::routes(
                &provider_credentials::ProviderCredentialsState {
                    gateway: state,
                    credentials,
                },
            )),
            None => router,
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
