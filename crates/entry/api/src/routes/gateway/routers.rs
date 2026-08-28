//! Sub-router builders for the gateway surface: inference proxying, bridge
//! credential exchange, bridge profile and session routes, and release
//! downloads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use axum::routing::{get, post};
use std::sync::Arc;
use systemprompt_runtime::AppContext;

use crate::services::gateway::protocol::inbound::InboundAdapter;
use crate::services::gateway::protocol::inbound::anthropic_messages::AnthropicMessagesInbound;
use crate::services::gateway::protocol::inbound::openai_chat::OpenAiChatInbound;
use crate::services::gateway::protocol::inbound::openai_responses::OpenAiResponsesInbound;
use crate::services::middleware::JwtContextExtractor;

use super::{
    auth, bridge, bridge_decisions, bridge_heartbeat, bridge_manifest, bridge_plugin_file,
    bridge_profile_usage, bridge_release, bridge_stream, bridge_whoami, messages,
};

pub(super) fn inference_routes(
    ctx: &AppContext,
    jwt_extractor: &Arc<JwtContextExtractor>,
    repos: &Arc<crate::services::gateway::GatewayRepositories>,
) -> Router {
    let ctx_messages = ctx.clone();
    let ctx_responses = ctx.clone();
    let ctx_chat = ctx.clone();
    let repos_messages = Arc::clone(repos);
    let repos_responses = Arc::clone(repos);
    let repos_chat = Arc::clone(repos);
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
                let repos = Arc::clone(&repos_messages);
                let inbound = Arc::clone(&anthropic_inbound);
                async move { messages::handle(inbound, extractor, context, repos, request).await }
            }),
        )
        .route(
            "/responses",
            post(move |request| {
                let extractor = Arc::clone(&jwt_responses);
                let context = ctx_responses.clone();
                let repos = Arc::clone(&repos_responses);
                let inbound = Arc::clone(&responses_inbound);
                async move { messages::handle(inbound, extractor, context, repos, request).await }
            }),
        )
        .route(
            "/chat/completions",
            post(move |request| {
                let extractor = Arc::clone(&jwt_chat);
                let context = ctx_chat.clone();
                let repos = Arc::clone(&repos_chat);
                let inbound = Arc::clone(&chat_inbound);
                async move { messages::handle(inbound, extractor, context, repos, request).await }
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
    let ctx_mtls = ctx.clone();
    let ctx_oauth_client = ctx.clone();
    let jwt_oauth_client = Arc::clone(jwt_extractor);

    Router::new()
        .route(
            "/auth/bridge/pat",
            post(move |request| {
                let context = ctx_pat.clone();
                async move { auth::pat(context, request).await }
            }),
        )
        .route(
            "/auth/bridge/session",
            post(move |caller_ip, headers, body| {
                let context = ctx_session.clone();
                async move { auth::session(context, caller_ip, headers, body).await }
            }),
        )
        .route(
            "/auth/bridge/session-pat",
            post(move |body| {
                let context = ctx_session_pat.clone();
                async move { auth::session_pat(context, body).await }
            }),
        )
        .route(
            "/auth/bridge/mtls",
            post(move |caller_ip, headers, body| {
                let context = ctx_mtls.clone();
                async move { auth::mtls(context, caller_ip, headers, body).await }
            }),
        )
        .route(
            "/auth/bridge/oauth-client",
            post(move |request| {
                let extractor = Arc::clone(&jwt_oauth_client);
                let context = ctx_oauth_client.clone();
                async move { auth::provision_oauth_client(extractor, context, request).await }
            }),
        )
        .route("/auth/bridge/capabilities", get(auth::capabilities))
}

pub(super) fn bridge_release_routes(jwt_extractor: &Arc<JwtContextExtractor>) -> Router {
    let jwt_latest = Arc::clone(jwt_extractor);
    let jwt_download = Arc::clone(jwt_extractor);

    Router::new()
        .route(
            "/bridge/latest",
            get(move |headers, query| {
                let extractor = Arc::clone(&jwt_latest);
                async move { bridge_release::latest(extractor, headers, query).await }
            }),
        )
        .route(
            "/bridge/download/{platform}",
            get(move |headers, path| {
                let extractor = Arc::clone(&jwt_download);
                async move { bridge_release::download(extractor, headers, path).await }
            }),
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
    let ctx_plugin_file = ctx.clone();
    let jwt_plugin_file = Arc::clone(jwt_extractor);
    let jwt_whoami = Arc::clone(jwt_extractor);
    let jwt_manifest = Arc::clone(jwt_extractor);
    let jwt_enabled_hosts = Arc::clone(jwt_extractor);
    let jwt_host_model_filter = Arc::clone(jwt_extractor);
    Router::new()
        .route("/bridge/pubkey", get(bridge::pubkey))
        .route("/bridge/profile", get(bridge::profile))
        .route(
            "/bridge/whoami",
            get(move |headers| {
                let extractor = Arc::clone(&jwt_whoami);
                let context = ctx_whoami.clone();
                async move { bridge_whoami::handle(extractor, context, headers).await }
            }),
        )
        .route(
            "/bridge/manifest",
            get(move |headers| {
                let extractor = Arc::clone(&jwt_manifest);
                let context = ctx_manifest.clone();
                async move { bridge_manifest::manifest(extractor, context, headers).await }
            }),
        )
        .route(
            "/bridge/plugins/{plugin_id}/{*path}",
            get(move |headers, path| {
                let extractor = Arc::clone(&jwt_plugin_file);
                let context = ctx_plugin_file.clone();
                async move { bridge_plugin_file::handle(extractor, context, headers, path).await }
            }),
        )
        .route(
            "/bridge/profile/enabled_hosts",
            post(move |headers, body| {
                let extractor = Arc::clone(&jwt_enabled_hosts);
                let context = ctx_enabled_hosts.clone();
                async move { bridge::set_enabled_host(extractor, context, headers, body).await }
            }),
        )
        .route(
            "/bridge/profile/host-model-filter",
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
    let ctx_decisions = ctx.clone();
    let ctx_heartbeat = ctx.clone();
    let jwt_profile_usage = Arc::clone(jwt_extractor);
    let jwt_decisions = Arc::clone(jwt_extractor);
    let jwt_heartbeat = Arc::clone(jwt_extractor);
    let jwt_stream = Arc::clone(jwt_extractor);
    Router::new()
        .route(
            "/bridge/profile/usage",
            get(move |headers| {
                let extractor = Arc::clone(&jwt_profile_usage);
                let context = ctx_profile_usage.clone();
                async move { bridge_profile_usage::handle(extractor, context, headers).await }
            }),
        )
        .route(
            "/bridge/decisions",
            get(move |headers, query| {
                let extractor = Arc::clone(&jwt_decisions);
                let context = ctx_decisions.clone();
                async move { bridge_decisions::handle(extractor, context, headers, query).await }
            }),
        )
        .route(
            "/bridge/heartbeat",
            post(move |headers, body| {
                let extractor = Arc::clone(&jwt_heartbeat);
                let context = ctx_heartbeat.clone();
                async move { bridge_heartbeat::handle(extractor, context, headers, body).await }
            }),
        )
        .route(
            "/bridge/stream",
            get(move |headers| {
                let extractor = Arc::clone(&jwt_stream);
                async move { bridge_stream::handle(extractor, headers).await }
            }),
        )
}
