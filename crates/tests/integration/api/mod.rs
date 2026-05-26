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
#[path = "common.rs"]
mod common;
