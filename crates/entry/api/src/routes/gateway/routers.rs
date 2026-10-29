//! Sub-router builders for the gateway surface: inference proxying, bridge
//! credential exchange, bridge profile and session routes, and release
//! downloads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use axum::routing::{get, post};
use std::sync::Arc;
use systemprompt_gateway::GatewayEngines;
use systemprompt_runtime::AppContext;

use crate::services::middleware::JwtContextExtractor;
use systemprompt_gateway::protocol::inbound::InboundAdapter;
use systemprompt_gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_gateway::protocol::inbound::openai_chat::OpenAiChatInbound;
use systemprompt_gateway::protocol::inbound::openai_responses::OpenAiResponsesInbound;
use systemprompt_models::ApiPaths;

use super::{
    auth, bridge, bridge_device, bridge_heartbeat, bridge_manifest, bridge_plugin_file,
    bridge_profile_usage, bridge_stream, bridge_whoami, count_tokens, messages, otel,
};

pub(super) fn otel_routes(ctx: &AppContext, jwt_extractor: &Arc<JwtContextExtractor>) -> Router {
    let ctx_root = ctx.clone();
    let ctx_rest = ctx.clone();
    let jwt_root = Arc::clone(jwt_extractor);
    let jwt_rest = Arc::clone(jwt_extractor);

    Router::new()
        .route(
            ApiPaths::OTEL,
            post(move |request| {
                let extractor = Arc::clone(&jwt_root);
                let context = ctx_root.clone();
                async move { otel::handle(extractor, context, request).await }
            }),
        )
        .route(
            ApiPaths::OTEL_REST_ROUTE,
            post(move |request| {
                let extractor = Arc::clone(&jwt_rest);
                let context = ctx_rest.clone();
                async move { otel::handle(extractor, context, request).await }
            }),
        )
}

pub(super) fn inference_routes(
    ctx: &AppContext,
    jwt_extractor: &Arc<JwtContextExtractor>,
    repos: &Arc<systemprompt_gateway::GatewayRepositories>,
    engines: &Arc<GatewayEngines>,
) -> Router {
    let handles = messages::GatewayHandles {
        repos: Arc::clone(repos),
        engines: Arc::clone(engines),
    };
    let handles_messages = handles.clone();
    let handles_responses = handles.clone();
    let handles_chat = handles.clone();
    let handles_count = handles;
    let ctx_count = ctx.clone();
    let jwt_count = Arc::clone(jwt_extractor);
    let ctx_messages = ctx.clone();
    let ctx_responses = ctx.clone();
    let ctx_chat = ctx.clone();
    let jwt_messages = Arc::clone(jwt_extractor);
    let jwt_responses = Arc::clone(jwt_extractor);
    let jwt_chat = Arc::clone(jwt_extractor);
    let anthropic_inbound: Arc<dyn InboundAdapter> = Arc::new(AnthropicMessagesInbound);
    let responses_inbound: Arc<dyn InboundAdapter> = Arc::new(OpenAiResponsesInbound);
    let chat_inbound: Arc<dyn InboundAdapter> = Arc::new(OpenAiChatInbound);

    Router::new()
        .route(
            "/messages",
            post(move |request| {
                let extractor = Arc::clone(&jwt_messages);
                let context = ctx_messages.clone();
                let handles = handles_messages.clone();
                let inbound = Arc::clone(&anthropic_inbound);
                async move { messages::handle(inbound, extractor, context, handles, request).await }
            }),
        )
        .route(
            "/messages/count_tokens",
            post(move |request| {
                let extractor = Arc::clone(&jwt_count);
                let context = ctx_count.clone();
                let handles = handles_count.clone();
                async move { count_tokens::handle(extractor, context, handles, request).await }
            }),
        )
        .route(
            "/responses",
            post(move |request| {
                let extractor = Arc::clone(&jwt_responses);
                let context = ctx_responses.clone();
                let handles = handles_responses.clone();
                let inbound = Arc::clone(&responses_inbound);
                async move { messages::handle(inbound, extractor, context, handles, request).await }
            }),
        )
        .route(
            "/chat/completions",
            post(move |request| {
                let extractor = Arc::clone(&jwt_chat);
                let context = ctx_chat.clone();
                let handles = handles_chat.clone();
                let inbound = Arc::clone(&chat_inbound);
                async move { messages::handle(inbound, extractor, context, handles, request).await }
            }),
        )
}

pub(super) fn bridge_auth_routes(
    ctx: &AppContext,
    jwt_extractor: &Arc<JwtContextExtractor>,
) -> Router {
    let ctx_pat = ctx.clone();
    let ctx_session = ctx.clone();
    let ctx_session_pat = ctx.clone();
    let ctx_oauth_client = ctx.clone();
    let jwt_oauth_client = Arc::clone(jwt_extractor);

    Router::new()
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_AUTH_PAT),
            post(move |request| {
                let context = ctx_pat.clone();
                async move { auth::pat(context, request).await }
            }),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_AUTH_SESSION),
            post(move |caller_ip, headers, body| {
                let context = ctx_session.clone();
                async move { auth::session(context, caller_ip, headers, body).await }
            }),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_AUTH_SESSION_PAT),
            post(move |body| {
                let context = ctx_session_pat.clone();
                async move { auth::session_pat(context, body).await }
            }),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_AUTH_OAUTH_CLIENT),
            post(move |request| {
                let extractor = Arc::clone(&jwt_oauth_client);
                let context = ctx_oauth_client.clone();
                async move { auth::provision_oauth_client(extractor, context, request).await }
            }),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_AUTH_CAPABILITIES),
            get(auth::capabilities),
        )
}

pub(super) fn bridge_profile_routes(
    ctx: &AppContext,
    jwt_extractor: &Arc<JwtContextExtractor>,
) -> Router {
    let ctx_whoami = ctx.clone();
    let ctx_manifest = ctx.clone();
    let ctx_enabled_hosts = ctx.clone();
    let ctx_host_model_filter = ctx.clone();
    let ctx_device = ctx.clone();
    let jwt_device = Arc::clone(jwt_extractor);
    let jwt_whoami = Arc::clone(jwt_extractor);
    let jwt_manifest = Arc::clone(jwt_extractor);
    let jwt_enabled_hosts = Arc::clone(jwt_extractor);
    let jwt_host_model_filter = Arc::clone(jwt_extractor);
    let ctx_pubkey = ctx.clone();
    let ctx_profile = ctx.clone();
    let jwt_profile = Arc::clone(jwt_extractor);
    Router::new()
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_PUBKEY),
            get(move || {
                let context = ctx_pubkey.clone();
                async move { bridge::pubkey(&context) }
            }),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_PROFILE),
            get(move |headers| {
                let extractor = Arc::clone(&jwt_profile);
                let context = ctx_profile.clone();
                async move { bridge::profile(extractor, context, headers).await }
            }),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_WHOAMI),
            get(move |headers| {
                let extractor = Arc::clone(&jwt_whoami);
                let context = ctx_whoami.clone();
                async move { bridge_whoami::handle(extractor, context, headers).await }
            }),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_MANIFEST),
            get(move |headers| {
                let extractor = Arc::clone(&jwt_manifest);
                let context = ctx_manifest.clone();
                Box::pin(bridge_manifest::manifest(extractor, context, headers))
            }),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_PLUGIN_FILE_ROUTE),
            bridge_plugin_file::route(ctx, jwt_extractor),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_DEVICE),
            post(move |headers, body| {
                let extractor = Arc::clone(&jwt_device);
                let context = ctx_device.clone();
                async move { bridge_device::enroll_self(extractor, context, headers, body).await }
            }),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_PROFILE_ENABLED_HOSTS),
            post(move |headers, body| {
                let extractor = Arc::clone(&jwt_enabled_hosts);
                let context = ctx_enabled_hosts.clone();
                async move { bridge::set_enabled_host(extractor, context, headers, body).await }
            }),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_PROFILE_HOST_MODEL_FILTER),
            post(move |headers, body| {
                let extractor = Arc::clone(&jwt_host_model_filter);
                let context = ctx_host_model_filter.clone();
                async move {
                    bridge::set_host_model_filter(extractor, context, headers, body).await
                }
            }),
        )
}

pub(super) fn bridge_session_routes(
    ctx: &AppContext,
    jwt_extractor: &Arc<JwtContextExtractor>,
) -> Router {
    let ctx_profile_usage = ctx.clone();
    let ctx_heartbeat = ctx.clone();
    let jwt_profile_usage = Arc::clone(jwt_extractor);
    let jwt_heartbeat = Arc::clone(jwt_extractor);
    let jwt_stream = Arc::clone(jwt_extractor);
    let broadcasters_stream = Arc::clone(ctx.event_broadcasters());
    Router::new()
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_PROFILE_USAGE),
            get(move |headers| {
                let extractor = Arc::clone(&jwt_profile_usage);
                let context = ctx_profile_usage.clone();
                async move { bridge_profile_usage::handle(extractor, context, headers).await }
            }),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_HEARTBEAT),
            post(move |headers, body| {
                let extractor = Arc::clone(&jwt_heartbeat);
                let context = ctx_heartbeat.clone();
                async move { bridge_heartbeat::handle(extractor, context, headers, body).await }
            }),
        )
        .route(
            ApiPaths::gateway_relative(ApiPaths::BRIDGE_STREAM),
            get(move |headers| {
                let extractor = Arc::clone(&jwt_stream);
                let broadcasters = Arc::clone(&broadcasters_stream);
                async move { bridge_stream::handle(extractor, broadcasters, headers).await }
            }),
        )
}
