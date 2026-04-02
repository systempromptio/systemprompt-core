//! Unit tests for agent services
//!
//! Tests cover:
//! - Shared utilities (slug generation, config, error, resilience)
//! - Agent orchestration (events, event_bus, status, validation)

mod shared;
mod agent_orchestration;
mod a2a_server;
mod mcp;
mod skills;
