//! HTTP request/response models for the agent management API.
//!
//! Wraps the A2A [`AgentCard`](crate::models::a2a::AgentCard) in the
//! create/update request shapes accepted over the web boundary
//! ([`CreateAgentRequest`], [`UpdateAgentRequest`]), the agent discovery
//! responses ([`AgentDiscoveryResponse`]), list-query parameters, and the
//! shared card-input parsing and validation helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod card_input;
mod create_agent;
mod discovery;
mod query;
mod update_agent;
mod validation;

pub use card_input::AgentCardInput;
pub use create_agent::{CreateAgentRequest, CreateAgentRequestRaw};
pub use discovery::{AgentCounts, AgentDiscoveryEntry, AgentDiscoveryResponse};
pub use query::ListAgentsQuery;
pub use update_agent::{UpdateAgentRequest, UpdateAgentRequestRaw};
pub use validation::{extract_port_from_url, is_valid_version, list_available_mcp_servers};
