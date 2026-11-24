//! Tests for `systemprompt-mcp-entry`: the MCP registry handler and its
//! route set, and the `plugins mcp` commands driven through a member host
//! holding the `caller-session` grant over a kernel that holds only what the
//! `mcp` member provides.

#[cfg(test)]
mod common;
#[cfg(test)]
mod mcp_call_client;
#[cfg(test)]
mod mcp_call_credential;
#[cfg(test)]
mod mcp_internal_status_lifecycle;
#[cfg(test)]
mod mcp_list_summaries;
#[cfg(test)]
mod mcp_logs_db;
#[cfg(test)]
mod mcp_logs_disk;
#[cfg(test)]
mod mcp_logs_follow_lifecycle;
#[cfg(test)]
mod mcp_prompts;
#[cfg(test)]
mod mcp_validate_outputs;
#[cfg(test)]
mod plugins_mcp_configured_db;
#[cfg(test)]
mod probe_flows;
#[cfg(test)]
mod proxy_audit;
#[cfg(test)]
mod proxy_fixed_arguments;
#[cfg(test)]
mod registry_handler;
#[cfg(test)]
mod registry_listing;
