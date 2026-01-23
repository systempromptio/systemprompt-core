//! Unit tests for agent models
//!
//! Tests cover:
//! - A2A protocol models (JsonRpc, protocol types)
//! - Context models (ContextMessage, ContextStateEvent)
//! - Runtime models (AgentRuntimeInfo)
//! - Web models (ListAgentsQuery, AgentDiscovery)
//! - Agent info (AgentInfo builder methods)
//! - Skill models (Skill, SkillMetadata)
//! - External integration models (TokenInfo, WebhookEndpoint, etc.)
//! - Protocol event types (TaskStatusUpdateEvent, etc.)

mod a2a;
mod agent_info;
mod context;
mod external_integrations;
mod protocol_events;
mod runtime;
mod skill;
mod web;
