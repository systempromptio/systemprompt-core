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
#[path = "common.rs"]
mod common;
