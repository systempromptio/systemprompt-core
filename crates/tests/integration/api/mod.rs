//! Integration tests for `systemprompt-api` HTTP route handlers.

#[cfg(test)]
#[path = "routes_marketplace.rs"]
mod routes_marketplace;

#[cfg(test)]
#[path = "routes_mcp_registry.rs"]
mod routes_mcp_registry;

#[cfg(test)]
#[path = "routes_wellknown.rs"]
mod routes_wellknown;

#[cfg(test)]
#[path = "routes_agent_registry.rs"]
mod routes_agent_registry;

#[cfg(test)]
#[path = "routes_agent_tasks.rs"]
mod routes_agent_tasks;

#[cfg(test)]
#[path = "routes_admin_keys.rs"]
mod routes_admin_keys;

#[cfg(test)]
#[path = "routes_content_links.rs"]
mod routes_content_links;

#[cfg(test)]
#[path = "routes_users_sessions.rs"]
mod routes_users_sessions;

#[cfg(test)]
#[path = "routes_misc.rs"]
mod routes_misc;

#[cfg(test)]
#[path = "routes_gateway.rs"]
mod routes_gateway;

#[cfg(test)]
#[path = "routes_gateway_authed.rs"]
mod routes_gateway_authed;

#[cfg(test)]
#[path = "routes_gateway_manifest_skills.rs"]
mod routes_gateway_manifest_skills;

#[cfg(test)]
#[path = "routes_oauth_discovery.rs"]
mod routes_oauth_discovery;

#[cfg(test)]
#[path = "routes_stream.rs"]
mod routes_stream;

#[cfg(test)]
#[path = "routes_analytics.rs"]
mod routes_analytics;

#[cfg(test)]
#[path = "routes_engagement.rs"]
mod routes_engagement;

#[cfg(test)]
#[path = "routes_sync.rs"]
mod routes_sync;

#[cfg(test)]
#[path = "routes_proxy.rs"]
mod routes_proxy;

#[cfg(test)]
#[path = "routes_health_and_oauth_wellknown.rs"]
mod routes_health_and_oauth_wellknown;

#[cfg(test)]
#[path = "routes_oauth_public.rs"]
mod routes_oauth_public;

#[cfg(test)]
#[path = "routes_admin_cli.rs"]
mod routes_admin_cli;

#[cfg(test)]
#[path = "routes_agent_contexts.rs"]
mod routes_agent_contexts;

#[cfg(test)]
#[path = "server_boot.rs"]
mod server_boot;

#[cfg(test)]
#[path = "services_quota_policy.rs"]
mod services_quota_policy;

#[cfg(test)]
#[path = "routes_bridge_data.rs"]
mod routes_bridge_data;

#[cfg(test)]
#[path = "routes_gateway_bridge_jwt.rs"]
mod routes_gateway_bridge_jwt;

#[cfg(test)]
#[path = "routes_oauth_pkce_flow.rs"]
mod routes_oauth_pkce_flow;

#[cfg(test)]
#[path = "middleware_edges.rs"]
mod middleware_edges;

#[cfg(test)]
#[path = "gateway_extract_units.rs"]
mod gateway_extract_units;

#[cfg(test)]
#[path = "stream_tap_accumulator.rs"]
mod stream_tap_accumulator;

#[cfg(test)]
#[path = "protocol_matrix.rs"]
mod protocol_matrix;

#[cfg(test)]
#[path = "routes_oauth_token.rs"]
mod routes_oauth_token;

#[cfg(test)]
#[path = "common.rs"]
mod common;

#[cfg(test)]
#[path = "oauth_register_owner_fk_present.rs"]
mod oauth_register_owner_fk_present;

#[cfg(test)]
#[path = "session_middleware_persists_anon.rs"]
mod session_middleware_persists_anon;

#[cfg(test)]
#[path = "routes_mcp_unauth_challenge.rs"]
mod routes_mcp_unauth_challenge;

#[cfg(test)]
#[path = "routes_oauth_callback.rs"]
mod routes_oauth_callback;

#[cfg(test)]
#[path = "routes_oauth_webauthn_complete.rs"]
mod routes_oauth_webauthn_complete;

#[cfg(test)]
#[path = "routes_oauth_token_exchange.rs"]
mod routes_oauth_token_exchange;

#[cfg(test)]
#[path = "routes_health_discovery.rs"]
mod routes_health_discovery;

#[cfg(test)]
#[path = "routes_agent_webhook.rs"]
mod routes_agent_webhook;

#[cfg(test)]
#[path = "routes_proxy_forward.rs"]
mod routes_proxy_forward;

#[cfg(test)]
#[path = "routes_sync_files.rs"]
mod routes_sync_files;

#[cfg(test)]
#[path = "routes_agent_artifacts.rs"]
mod routes_agent_artifacts;

#[cfg(test)]
#[path = "messaging_dispatch.rs"]
mod messaging_dispatch;

#[cfg(test)]
#[path = "routes_slack.rs"]
mod routes_slack;

#[cfg(test)]
#[path = "routes_teams.rs"]
mod routes_teams;
