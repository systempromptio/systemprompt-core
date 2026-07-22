//! Unit tests for agent services
//!
//! Tests cover:
//! - Shared utilities (slug generation, config, error, resilience)
//! - Agent orchestration (events, event_bus, status, validation)

mod a2a_server;
mod agent_orchestration;
mod agent_token_validation;
mod artifact_publishing;
mod auth_validation;
mod config_authoring;
mod context_history;
mod context_provider;
mod execution_tracking_service;
mod mcp;
mod message_service;
mod monitor;
mod oauth_validation;
mod plan_executor;
mod registry;
mod registry_provider;
mod shared;
mod skills;
mod strategies;
mod webhook_client;
mod webhook_config;
mod webhook_delivery;
mod webhook_service;
