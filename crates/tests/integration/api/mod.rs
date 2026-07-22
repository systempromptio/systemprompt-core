//! Integration tests for `systemprompt-api` HTTP route handlers.

#[cfg(test)]
#[path = "routes_marketplace.rs"]
mod routes_marketplace;

#[cfg(test)]
#[path = "routes_marketplace_success.rs"]
mod routes_marketplace_success;

#[cfg(test)]
#[path = "routes_gateway_auth.rs"]
mod routes_gateway_auth;

#[cfg(test)]
#[path = "routes_sync_archive_guards.rs"]
mod routes_sync_archive_guards;

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
#[path = "server_extension_mount.rs"]
mod server_extension_mount;

#[cfg(test)]
#[path = "server_early_bind.rs"]
mod server_early_bind;

#[cfg(test)]
#[path = "server_run_loop.rs"]
mod server_run_loop;

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
#[path = "routes_slack_more.rs"]
mod routes_slack_more;

#[cfg(test)]
#[path = "routes_teams.rs"]
mod routes_teams;

#[cfg(test)]
#[path = "routes_oauth_consent.rs"]
mod routes_oauth_consent;

#[cfg(test)]
#[path = "routes_oauth_authorize.rs"]
mod routes_oauth_authorize;

#[cfg(test)]
#[path = "routes_oauth_logout.rs"]
mod routes_oauth_logout;

#[cfg(test)]
#[path = "middleware_session_lifecycle.rs"]
mod middleware_session_lifecycle;

#[cfg(test)]
#[path = "middleware_analytics_detection.rs"]
mod middleware_analytics_detection;

#[cfg(test)]
#[path = "middleware_behavioral_detection.rs"]
mod middleware_behavioral_detection;

#[cfg(test)]
#[path = "middleware_jwt_context.rs"]
mod middleware_jwt_context;

#[cfg(test)]
#[path = "health_monitor.rs"]
mod health_monitor;

#[cfg(test)]
#[path = "static_files_serving.rs"]
mod static_files_serving;

#[cfg(test)]
#[path = "gateway_pipeline.rs"]
mod gateway_pipeline;

#[cfg(test)]
#[path = "gateway_upstream_error_map.rs"]
mod gateway_upstream_error_map;

#[cfg(test)]
#[path = "routes_content_blog.rs"]
mod routes_content_blog;

#[cfg(test)]
#[path = "routes_content_negotiation.rs"]
mod routes_content_negotiation;

#[cfg(test)]
#[path = "routes_bridge_plugin_file.rs"]
mod routes_bridge_plugin_file;

#[cfg(test)]
#[path = "routes_bridge_manifest_catalog.rs"]
mod routes_bridge_manifest_catalog;

#[cfg(test)]
#[path = "routes_bridge_manifest_config_error.rs"]
mod routes_bridge_manifest_config_error;

#[cfg(test)]
#[path = "routes_slack_edges.rs"]
mod routes_slack_edges;

#[cfg(test)]
#[path = "routes_proxy_mcp_executions.rs"]
mod routes_proxy_mcp_executions;

#[cfg(test)]
#[path = "routes_agent_cards.rs"]
mod routes_agent_cards;

#[cfg(test)]
#[path = "proxy_auth_access.rs"]
mod proxy_auth_access;

#[cfg(test)]
#[path = "proxy_mcp_session_cache.rs"]
mod proxy_mcp_session_cache;

#[cfg(test)]
#[path = "proxy_audit_tap.rs"]
mod proxy_audit_tap;

#[cfg(test)]
#[path = "routes_oauth_token_grants_happy.rs"]
mod routes_oauth_token_grants_happy;

#[cfg(test)]
#[path = "routes_oauth_clients.rs"]
mod routes_oauth_clients;

#[cfg(test)]
#[path = "routes_oauth_introspect_revoke.rs"]
mod routes_oauth_introspect_revoke;

#[cfg(test)]
#[path = "routes_oauth_userinfo.rs"]
mod routes_oauth_userinfo;

#[cfg(test)]
#[path = "routes_oauth_callback_flow.rs"]
mod routes_oauth_callback_flow;

#[cfg(test)]
#[path = "routes_oauth_webauthn.rs"]
mod routes_oauth_webauthn;

#[cfg(test)]
#[path = "routes_oauth_token_exchange_oidc.rs"]
mod routes_oauth_token_exchange_oidc;

#[cfg(test)]
#[path = "routes_agent_notifications.rs"]
mod routes_agent_notifications;

#[cfg(test)]
#[path = "routes_agent_context_events.rs"]
mod routes_agent_context_events;

#[cfg(test)]
#[path = "routes_agent_webhook_broadcast.rs"]
mod routes_agent_webhook_broadcast;

#[cfg(test)]
#[path = "routes_agent_contexts_crud.rs"]
mod routes_agent_contexts_crud;

#[cfg(test)]
#[path = "gateway_messages_more.rs"]
mod gateway_messages_more;

#[cfg(test)]
#[path = "gateway_otel_ingest.rs"]
mod gateway_otel_ingest;

#[cfg(test)]
#[path = "gateway_bridge_models_more.rs"]
mod gateway_bridge_models_more;

#[cfg(test)]
#[path = "gateway_dispatch_more.rs"]
mod gateway_dispatch_more;

#[cfg(test)]
#[path = "server_readiness_metrics.rs"]
mod server_readiness_metrics;

#[cfg(test)]
#[path = "server_health_more.rs"]
mod server_health_more;

#[cfg(test)]
#[path = "static_content_more.rs"]
mod static_content_more;

#[cfg(test)]
#[path = "middleware_more.rs"]
mod middleware_more;

#[cfg(test)]
#[path = "proxy_external_mcp.rs"]
mod proxy_external_mcp;

#[cfg(test)]
#[path = "proxy_more.rs"]
mod proxy_more;

#[cfg(test)]
#[path = "api_error_units.rs"]
mod api_error_units;

#[cfg(test)]
#[path = "routes_content_more.rs"]
mod routes_content_more;

#[cfg(test)]
#[path = "routes_analytics_more.rs"]
mod routes_analytics_more;

#[cfg(test)]
#[path = "routes_analytics_events_success.rs"]
mod routes_analytics_events_success;

#[cfg(test)]
#[path = "routes_admin_cli_subprocess.rs"]
mod routes_admin_cli_subprocess;
