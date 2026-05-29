//! Unit tests for agent models
//!
//! Tests cover:
//! - A2A protocol models (JsonRpc, protocol types)
//! - Context models (ContextMessage, ContextStateEvent)
//! - Runtime models (AgentRuntimeInfo)
//! - Web models (ListAgentsQuery, AgentDiscovery)
//! - Agent info (AgentInfo builder methods)
//! - External integration models (TokenInfo, WebhookEndpoint, etc.)
//! - Protocol event types (TaskStatusUpdateEvent, etc.)

mod a2a;
mod agent_info;
mod agent_info_extended;
mod agent_runtime;
mod context;
mod context_events_extended;
mod create_update_agent;
mod database_rows;
mod external_integrations;
mod protocol_events;
mod protocol_requests;
mod push_notification_extended;
mod runtime;
mod service_status;
mod validation;
mod web;
mod web_agent_requests;
