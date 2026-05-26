//! Unit tests for agent services
//!
//! Tests cover:
//! - Shared utilities (slug generation, config, error, resilience)
//! - Agent orchestration (events, event_bus, status, validation)

mod a2a_server;
mod auth_validation;
mod agent_orchestration;
mod mcp;
mod registry;
mod shared;
mod skills;
mod strategies;
mod webhook_service;
